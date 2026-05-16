//! Reusable GEPA optimizer loop.

use std::collections::{BTreeMap, BTreeSet};

use leaven_core::{
    AssessmentGranularity, AssessmentTarget, EvaluationPurpose, EvaluationRequest, EvaluationSet,
    OptimizationProblem, PartitionId,
};
use leaven_engine::{
    CheckpointContext, CheckpointError, CheckpointableOptimizer, Optimizer, OptimizerError,
    OptimizerStateReader, OptimizerStateWrite, PopulationEvent, PrivateStatePolicy, RestoreContext,
    RunContext, RunGraphView, StateFormat, restore_checkpointable_optimizer_state,
};
use leaven_evidence::{CaseAssessmentEvidence, CaseOutcome, CasewiseEvidence, ScalarEvidence};
use leaven_kernel::CaseId;
use leaven_kernel::{AssessmentId, CandidateId, EvaluatorId, Fingerprint, PopulationId};
use leaven_population::{KeepBest, ParetoFrontier};
use leaven_surface::{EditSurface, SurfaceError};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    CandidateSelector, CheckpointCandidateSelector, CheckpointGate, CheckpointPartSelector, Gate,
    GateDecision, GepaReflectiveDataset, GepaReflector, ParetoFrequencyWeighted, PartSelector,
    ReflectRequest, ReflectiveDatasetBuilder, RoundRobinPart, StrictImprovement,
    validation::{
        BatchSampler, CheckpointBatchSampler, CheckpointValidationPolicy, EpochShuffled,
        FullValidation, ValidationPolicy,
    },
};

const GEPA_OPTIMIZER_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([8; 32]);
const DEFAULT_MAX_ITERATIONS: usize = 500;
const GEPA_CHECKPOINT_SCHEMA: Fingerprint = Fingerprint::from_bytes([9; 32]);

/// One assessment-row evidence shape GEPA can compare as a scalar score.
pub trait GepaCaseEvidence: leaven_core::Evidence {
    /// Project the comparable scalar score for this case row.
    fn scalar_score(&self) -> Option<ScalarEvidence>;
}

impl GepaCaseEvidence for ScalarEvidence {
    fn scalar_score(&self) -> Option<ScalarEvidence> {
        Some(*self)
    }
}

impl GepaCaseEvidence for CaseAssessmentEvidence {
    fn scalar_score(&self) -> Option<ScalarEvidence> {
        Some(self.score())
    }
}

/// Population behavior the reusable GEPA loop needs.
pub trait GepaPopulation {
    /// Population identifier for events.
    fn id(&self) -> PopulationId;
    /// Current best candidate.
    fn best(&self) -> Option<CandidateId>;
    /// Observe casewise scalar evidence.
    fn observe_gepa(
        &mut self,
        partition: Option<&PartitionId>,
        candidate: CandidateId,
        assessments: &[AssessmentId],
        evidence: &CasewiseEvidence<ScalarEvidence>,
    ) -> Vec<PopulationEvent>;
}

/// Private population state that must survive GEPA checkpoint/restore.
pub trait CheckpointPopulation {
    /// Serializable state shape.
    type State: Serialize + DeserializeOwned;

    /// Capture population state.
    fn checkpoint_state(&self) -> Self::State;

    /// Restore population state.
    fn restore_state(&mut self, state: Self::State);
}

impl GepaPopulation for ParetoFrontier {
    fn id(&self) -> PopulationId {
        self.id()
    }

    fn best(&self) -> Option<CandidateId> {
        self.best()
    }

    fn observe_gepa(
        &mut self,
        partition: Option<&PartitionId>,
        candidate: CandidateId,
        assessments: &[AssessmentId],
        evidence: &CasewiseEvidence<ScalarEvidence>,
    ) -> Vec<PopulationEvent> {
        let Some(assessment) = assessments.first().copied() else {
            return vec![PopulationEvent::Ignored {
                population: self.id(),
                candidate,
                reason: "casewise evidence had no assessment source rows".to_owned(),
            }];
        };
        match partition {
            Some(partition) => {
                self.observe_partitioned_casewise_scalar(partition, candidate, assessment, evidence)
            }
            None => self.observe_casewise_scalar(candidate, assessment, evidence),
        }
    }
}

impl CheckpointPopulation for ParetoFrontier {
    type State = Self;

    fn checkpoint_state(&self) -> Self::State {
        self.clone()
    }

    fn restore_state(&mut self, state: Self::State) {
        *self = state;
    }
}

impl GepaPopulation for KeepBest {
    fn id(&self) -> PopulationId {
        self.id()
    }

    fn best(&self) -> Option<CandidateId> {
        self.best()
    }

    fn observe_gepa(
        &mut self,
        _partition: Option<&PartitionId>,
        candidate: CandidateId,
        assessments: &[AssessmentId],
        evidence: &CasewiseEvidence<ScalarEvidence>,
    ) -> Vec<PopulationEvent> {
        let Some(assessment) = assessments.first().copied() else {
            return vec![PopulationEvent::Ignored {
                population: self.id(),
                candidate,
                reason: "casewise evidence had no assessment source rows".to_owned(),
            }];
        };
        let Some(score) = average_scalar(evidence) else {
            return vec![PopulationEvent::Ignored {
                population: self.id(),
                candidate,
                reason: "casewise evidence had no comparable score".to_owned(),
            }];
        };
        self.observe(
            candidate,
            assessment,
            ScalarEvidence::new(score).expect("finite average"),
        )
    }
}

impl CheckpointPopulation for KeepBest {
    type State = Self;

    fn checkpoint_state(&self) -> Self::State {
        self.clone()
    }

    fn restore_state(&mut self, state: Self::State) {
        *self = state;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct GepaValidationBest {
    candidate: CandidateId,
    assessments: Vec<AssessmentId>,
    score: f64,
}

/// Stable GEPA candidate index in discovery order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct GepaCandidateIndex(u32);

impl GepaCandidateIndex {
    /// Build a GEPA candidate index from a discovery-order value.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Raw discovery-order index. The seed candidate is always `0`.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One accepted candidate in GEPA reference state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GepaCandidateRecord {
    index: GepaCandidateIndex,
    candidate: CandidateId,
    parents: Vec<GepaCandidateIndex>,
    discovery_metric_calls: u64,
    validation_score: Option<f64>,
    validation_rows: Vec<AssessmentId>,
}

impl GepaCandidateRecord {
    /// GEPA discovery-order index.
    #[must_use]
    pub const fn index(&self) -> GepaCandidateIndex {
        self.index
    }

    /// Candidate id in graph truth.
    #[must_use]
    pub const fn candidate(&self) -> CandidateId {
        self.candidate
    }

    /// Parent GEPA indices.
    #[must_use]
    pub fn parents(&self) -> &[GepaCandidateIndex] {
        &self.parents
    }

    /// Metric calls spent when this candidate was admitted.
    #[must_use]
    pub const fn discovery_metric_calls(&self) -> u64 {
        self.discovery_metric_calls
    }

    /// Aggregate validation score.
    #[must_use]
    pub const fn validation_score(&self) -> Option<f64> {
        self.validation_score
    }

    /// Validation assessment rows backing this candidate.
    #[must_use]
    pub fn validation_rows(&self) -> &[AssessmentId] {
        &self.validation_rows
    }
}

/// GEPA-owned reference state used for validation-frontier selection and reports.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GepaReferenceState {
    records: Vec<GepaCandidateRecord>,
    candidate_to_index: BTreeMap<CandidateId, GepaCandidateIndex>,
    validation_subscores: Vec<BTreeMap<CaseId, ScalarEvidence>>,
    validation_frontier_scores: BTreeMap<CaseId, ScalarEvidence>,
    validation_frontier_candidates: BTreeMap<CaseId, BTreeSet<GepaCandidateIndex>>,
    total_metric_calls: u64,
    full_validation_evals: u64,
}

impl GepaReferenceState {
    /// Accepted candidate records in GEPA discovery order.
    #[must_use]
    pub fn records(&self) -> &[GepaCandidateRecord] {
        &self.records
    }

    /// Per-validation-case frontier membership.
    #[must_use]
    pub const fn validation_frontier(&self) -> &BTreeMap<CaseId, BTreeSet<GepaCandidateIndex>> {
        &self.validation_frontier_candidates
    }

    /// Total new evaluator metric calls charged to GEPA search.
    #[must_use]
    pub const fn total_metric_calls(&self) -> u64 {
        self.total_metric_calls
    }

    /// Number of full validation evaluations GEPA has run.
    #[must_use]
    pub const fn full_validation_evals(&self) -> u64 {
        self.full_validation_evals
    }

    fn index_of(&self, candidate: CandidateId) -> Option<GepaCandidateIndex> {
        self.candidate_to_index.get(&candidate).copied()
    }

    fn best_candidate(&self) -> Option<CandidateId> {
        self.records
            .iter()
            .filter_map(|record| {
                record
                    .validation_score
                    .map(|score| (score, record.candidate))
            })
            .max_by(|left, right| left.0.total_cmp(&right.0))
            .map(|(_, candidate)| candidate)
    }

    fn select_by_validation_frontier_frequency(&self) -> Option<(GepaCandidateIndex, CandidateId)> {
        let mut frequencies = BTreeMap::<GepaCandidateIndex, usize>::new();
        for candidates in self.validation_frontier_candidates.values() {
            for candidate in candidates {
                *frequencies.entry(*candidate).or_default() += 1;
            }
        }
        let (index, _) = frequencies.into_iter().max_by(|left, right| {
            let left_score = self
                .record(left.0)
                .and_then(GepaCandidateRecord::validation_score)
                .unwrap_or(f64::NEG_INFINITY);
            let right_score = self
                .record(right.0)
                .and_then(GepaCandidateRecord::validation_score)
                .unwrap_or(f64::NEG_INFINITY);
            left.1
                .cmp(&right.1)
                .then_with(|| left_score.total_cmp(&right_score))
                .then_with(|| right.0.cmp(&left.0))
        })?;
        Some((index, self.record(index)?.candidate()))
    }

    fn record(&self, index: GepaCandidateIndex) -> Option<&GepaCandidateRecord> {
        self.records.get(usize::try_from(index.get()).ok()?)
    }

    fn add_validated_candidate(
        &mut self,
        candidate: CandidateId,
        parents: Vec<GepaCandidateIndex>,
        discovery_metric_calls: u64,
        validation: &GepaAssessment,
    ) -> GepaCandidateIndex {
        if let Some(index) = self.index_of(candidate) {
            return index;
        }
        let index = GepaCandidateIndex::new(
            u32::try_from(self.records.len()).expect("GEPA candidate count fits u32"),
        );
        let mut subscores = BTreeMap::new();
        for outcome in validation.scalar_evidence.outcomes() {
            let case = outcome.case();
            let score = *outcome.evidence();
            subscores.insert(case, score);
            match self.validation_frontier_scores.get(&case).copied() {
                None => {
                    self.validation_frontier_scores.insert(case, score);
                    self.validation_frontier_candidates
                        .insert(case, BTreeSet::from([index]));
                }
                Some(best) if score.score() > best.score() => {
                    self.validation_frontier_scores.insert(case, score);
                    self.validation_frontier_candidates
                        .insert(case, BTreeSet::from([index]));
                }
                Some(best) if (score.score() - best.score()).abs() < f64::EPSILON => {
                    self.validation_frontier_candidates
                        .entry(case)
                        .or_default()
                        .insert(index);
                }
                Some(_) => {}
            }
        }
        self.candidate_to_index.insert(candidate, index);
        self.validation_subscores.push(subscores);
        self.records.push(GepaCandidateRecord {
            index,
            candidate,
            parents,
            discovery_metric_calls,
            validation_score: Some(validation.average_score),
            validation_rows: validation.assessments.clone(),
        });
        index
    }

    fn add_unvalidated_candidate(
        &mut self,
        candidate: CandidateId,
        parents: Vec<GepaCandidateIndex>,
    ) -> GepaCandidateIndex {
        if let Some(index) = self.index_of(candidate) {
            return index;
        }
        let index = GepaCandidateIndex::new(
            u32::try_from(self.records.len()).expect("GEPA candidate count fits u32"),
        );
        self.candidate_to_index.insert(candidate, index);
        self.validation_subscores.push(BTreeMap::new());
        self.records.push(GepaCandidateRecord {
            index,
            candidate,
            parents,
            discovery_metric_calls: self.total_metric_calls,
            validation_score: None,
            validation_rows: Vec::new(),
        });
        index
    }

    fn add_metric_calls(&mut self, calls: u64) {
        self.total_metric_calls = self.total_metric_calls.saturating_add(calls);
    }

    fn note_full_validation(&mut self) {
        self.full_validation_evals = self.full_validation_evals.saturating_add(1);
    }
}

/// Non-fatal GEPA proposal skip reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum GepaSkipReason {
    /// The reflective dataset builder produced no examples.
    NoReflectiveExamples,
    /// All selected parent rows were already perfect.
    AllScoresPerfect,
}

/// Structured GEPA phase event summary for reports/tests.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GepaEventSummary {
    /// Profile was resolved.
    ProfileResolved,
    /// Seed validation started.
    SeedValidationStarted { candidate: CandidateId },
    /// Seed validation completed.
    SeedValidationCompleted {
        candidate_index: GepaCandidateIndex,
        score: String,
    },
    /// One GEPA iteration started.
    IterationStarted { iteration: usize },
    /// Parent was selected for mutation.
    ParentSelected { candidate_index: GepaCandidateIndex },
    /// Train minibatch was sampled.
    TrainMinibatchSampled,
    /// Parent evaluation completed.
    ParentEvaluated { metric_calls_delta: u64 },
    /// Proposal was skipped before provider work.
    ProposalSkipped { reason: GepaSkipReason },
    /// Reflective examples were built.
    ReflectiveDatasetBuilt { records: usize },
    /// Child candidate was built.
    ChildBuilt { candidate: CandidateId },
    /// Child evaluation completed.
    ChildEvaluated { metric_calls_delta: u64 },
    /// Proposal was accepted by the train-screening policy.
    ProposalAccepted { child: CandidateId },
    /// Proposal was rejected by the train-screening policy.
    ProposalRejected,
    /// Accepted candidate validation completed.
    AcceptedValidationCompleted { candidate_index: GepaCandidateIndex },
    /// Validation frontier was updated.
    FrontierUpdated,
    /// GEPA reached the end of optimizer execution.
    OptimizationEnded { best: Option<GepaCandidateIndex> },
}

/// One candidate observation tracked by GEPA's private history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GepaCandidateHistoryEntry {
    candidate: CandidateId,
    assessments: Vec<AssessmentId>,
    score: f64,
}

impl GepaCandidateHistoryEntry {
    /// Candidate observed by GEPA.
    #[must_use]
    pub const fn candidate(&self) -> CandidateId {
        self.candidate
    }

    /// Assessment rows that justified the observation.
    #[must_use]
    pub fn assessments(&self) -> &[AssessmentId] {
        &self.assessments
    }

    /// Comparable average score GEPA used for screening.
    #[must_use]
    pub const fn score(&self) -> f64 {
        self.score
    }
}

/// Reusable GEPA optimizer over an explicit edit surface.
#[derive(Clone, Debug)]
pub struct Gepa<
    S,
    Pop = ParetoFrontier,
    Reflect = crate::FixedSurfaceEdit<<S as EditSurfacePlaceholder>::Edit>,
    CandidateSel = ParetoFrequencyWeighted,
    PartSel = RoundRobinPart,
    GatePol = StrictImprovement,
    Batch = EpochShuffled,
    Validate = FullValidation,
    Dataset = GepaReflectiveDataset,
> {
    surface: S,
    population: Pop,
    reflector: Reflect,
    candidate_selector: CandidateSel,
    part_selector: PartSel,
    gate: GatePol,
    batch_sampler: Batch,
    validation_policy: Validate,
    dataset: Dataset,
    train_partition: PartitionId,
    max_iterations: usize,
    proposal_count: usize,
    completed_iterations: usize,
    best: Option<CandidateId>,
    validation_best: Option<GepaValidationBest>,
    observed: BTreeSet<CandidateId>,
    candidate_history: Vec<GepaCandidateHistoryEntry>,
    reference_state: GepaReferenceState,
    events: Vec<GepaEventSummary>,
}

/// Serializable GEPA private state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GepaCheckpointState<
    PopulationState,
    CandidateSelectorState,
    PartSelectorState,
    GateState,
    BatchSamplerState,
    ValidationPolicyState,
> {
    train_partition: PartitionId,
    max_iterations: usize,
    proposal_count: usize,
    completed_iterations: usize,
    best: Option<CandidateId>,
    validation_best: Option<GepaValidationBest>,
    observed: BTreeSet<CandidateId>,
    candidate_history: Vec<GepaCandidateHistoryEntry>,
    reference_state: GepaReferenceState,
    events: Vec<GepaEventSummary>,
    population: PopulationState,
    candidate_selector: CandidateSelectorState,
    part_selector: PartSelectorState,
    gate: GateState,
    batch_sampler: BatchSamplerState,
    validation_policy: ValidationPolicyState,
}

/// Internal helper trait used only to give `Gepa` a default generic slot.
/// Hidden from docs: it is required `pub` solely so the public `Gepa` default
/// type parameter resolves, never an intended import.
#[doc(hidden)]
pub trait EditSurfacePlaceholder {
    /// Edit type placeholder.
    type Edit;
}

impl<T> EditSurfacePlaceholder for T {
    type Edit = ();
}

impl<S, Pop, Reflect> Gepa<S, Pop, Reflect> {
    /// Build GEPA with deterministic default strategies.
    #[must_use]
    pub fn new(surface: S, population: Pop, reflector: Reflect) -> Self {
        Self::with_strategies(
            surface,
            population,
            reflector,
            ParetoFrequencyWeighted,
            RoundRobinPart::new(),
            StrictImprovement,
        )
    }
}

impl<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
    Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
{
    /// Build GEPA with explicit strategy values.
    #[must_use]
    pub fn with_strategies(
        surface: S,
        population: Pop,
        reflector: Reflect,
        candidate_selector: CandidateSel,
        part_selector: PartSel,
        gate: GatePol,
    ) -> Self
    where
        Batch: Default,
        Validate: Default,
        Dataset: Default,
    {
        Self {
            surface,
            population,
            reflector,
            candidate_selector,
            part_selector,
            gate,
            batch_sampler: Batch::default(),
            validation_policy: Validate::default(),
            dataset: Dataset::default(),
            train_partition: PartitionId::from("TRAIN"),
            max_iterations: DEFAULT_MAX_ITERATIONS,
            proposal_count: 1,
            completed_iterations: 0,
            best: None,
            validation_best: None,
            observed: BTreeSet::new(),
            candidate_history: Vec::new(),
            reference_state: GepaReferenceState::default(),
            events: Vec::new(),
        }
    }

    /// Surface owned by this optimizer.
    #[must_use]
    pub const fn surface(&self) -> &S {
        &self.surface
    }

    /// Population state owned by this optimizer.
    #[must_use]
    pub const fn population(&self) -> &Pop {
        &self.population
    }

    /// Mutable population state owned by this optimizer.
    #[must_use]
    pub const fn population_mut(&mut self) -> &mut Pop {
        &mut self.population
    }

    /// Mutable gate policy.
    #[must_use]
    pub const fn gate_mut(&mut self) -> &mut GatePol {
        &mut self.gate
    }

    /// Set the train minibatch sampler used for parent and child screening.
    #[must_use]
    pub fn batch_sampler<NextBatch>(
        self,
        batch_sampler: NextBatch,
    ) -> Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, NextBatch, Validate, Dataset> {
        Gepa {
            surface: self.surface,
            population: self.population,
            reflector: self.reflector,
            candidate_selector: self.candidate_selector,
            part_selector: self.part_selector,
            gate: self.gate,
            batch_sampler,
            validation_policy: self.validation_policy,
            dataset: self.dataset,
            train_partition: self.train_partition,
            max_iterations: self.max_iterations,
            proposal_count: self.proposal_count,
            completed_iterations: self.completed_iterations,
            best: self.best,
            validation_best: self.validation_best,
            observed: self.observed,
            candidate_history: self.candidate_history,
            reference_state: self.reference_state,
            events: self.events,
        }
    }

    /// Set the validation policy used after accepted candidates.
    #[must_use]
    pub fn validation_policy<NextValidate>(
        self,
        validation_policy: NextValidate,
    ) -> Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, NextValidate, Dataset> {
        Gepa {
            surface: self.surface,
            population: self.population,
            reflector: self.reflector,
            candidate_selector: self.candidate_selector,
            part_selector: self.part_selector,
            gate: self.gate,
            batch_sampler: self.batch_sampler,
            validation_policy,
            dataset: self.dataset,
            train_partition: self.train_partition,
            max_iterations: self.max_iterations,
            proposal_count: self.proposal_count,
            completed_iterations: self.completed_iterations,
            best: self.best,
            validation_best: self.validation_best,
            observed: self.observed,
            candidate_history: self.candidate_history,
            reference_state: self.reference_state,
            events: self.events,
        }
    }

    /// Swap the reflective-dataset builder used before each reflection step.
    ///
    /// The builder is the "what data does reflection see" seam. The default is
    /// [`GepaReflectiveDataset`], a GEPA-parity per-case projection. A plain
    /// closure can be passed here via the closure blanket impl.
    #[must_use]
    pub fn reflective_dataset<NextDataset>(
        self,
        dataset: NextDataset,
    ) -> Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, NextDataset> {
        Gepa {
            surface: self.surface,
            population: self.population,
            reflector: self.reflector,
            candidate_selector: self.candidate_selector,
            part_selector: self.part_selector,
            gate: self.gate,
            batch_sampler: self.batch_sampler,
            validation_policy: self.validation_policy,
            dataset,
            train_partition: self.train_partition,
            max_iterations: self.max_iterations,
            proposal_count: self.proposal_count,
            completed_iterations: self.completed_iterations,
            best: self.best,
            validation_best: self.validation_best,
            observed: self.observed,
            candidate_history: self.candidate_history,
            reference_state: self.reference_state,
            events: self.events,
        }
    }

    /// Set maximum fixed-surface-edit iterations.
    #[must_use]
    pub const fn max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set how many proposal attempts to run for the selected candidate in each iteration.
    #[must_use]
    pub const fn proposal_count(mut self, proposal_count: usize) -> Self {
        self.proposal_count = proposal_count;
        self
    }

    /// Candidate observations tracked by GEPA's private state.
    #[must_use]
    pub fn candidate_history(&self) -> &[GepaCandidateHistoryEntry] {
        &self.candidate_history
    }

    /// GEPA reference state used for candidate indices and validation frontier reports.
    #[must_use]
    pub const fn reference_state(&self) -> &GepaReferenceState {
        &self.reference_state
    }

    /// Structured GEPA phase events emitted by this optimizer.
    #[must_use]
    pub fn events(&self) -> &[GepaEventSummary] {
        &self.events
    }

    /// Select the next candidate to mutate.
    pub fn select_candidate<P>(&mut self, graph: RunGraphView<'_, P>) -> Option<CandidateId>
    where
        P: OptimizationProblem,
        CandidateSel: CandidateSelector<P, Pop, Selection = Option<CandidateId>>,
    {
        self.candidate_selector.select(&self.population, graph)
    }

    /// Select the next surface part to mutate.
    pub fn select_part<A>(&mut self, artifact: &A) -> Result<S::PartId, SurfaceError>
    where
        A: leaven_core::Artifact,
        S: EditSurface<A>,
        PartSel: PartSelector<A, S>,
    {
        self.part_selector.select_part(artifact, &self.surface)
    }

    /// Lower a surface-native edit into an artifact-native change.
    pub fn change_part<A>(
        &self,
        artifact: &A,
        part: S::PartId,
        edit: S::Edit,
    ) -> Result<<A as leaven_core::Artifact>::Change, SurfaceError>
    where
        A: leaven_core::Artifact,
        S: EditSurface<A>,
    {
        self.surface.change_part(artifact, part, edit)
    }

    /// Apply the configured gate to two scalar screening scores.
    pub fn decide(&mut self, parent_score: f64, candidate_score: f64) -> GateDecision
    where
        GatePol: Gate,
    {
        self.gate.decide(parent_score, candidate_score)
    }
}

impl<P, S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset> Optimizer<P>
    for Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
where
    P: OptimizationProblem,
    P::Evidence: GepaCaseEvidence,
    P::ProposalAnnotations: Default,
    S: EditSurface<P::Artifact> + Send + Sync,
    S::PartId: std::fmt::Debug,
    Pop: CheckpointPopulation + GepaPopulation + Send + Sync,
    Reflect: GepaReflector<P, S> + Send + Sync,
    CandidateSel: CandidateSelector<P, Pop, Selection = Option<CandidateId>>
        + CheckpointCandidateSelector
        + Send
        + Sync,
    PartSel: PartSelector<P::Artifact, S> + CheckpointPartSelector + Send + Sync,
    GatePol: CheckpointGate + Gate + Send + Sync,
    Batch: BatchSampler + CheckpointBatchSampler + Send + Sync,
    Validate: ValidationPolicy + CheckpointValidationPolicy + Send + Sync,
    Dataset: ReflectiveDatasetBuilder<P, S>,
{
    async fn initialize(&mut self, ctx: &mut RunContext<'_, P>) -> Result<(), OptimizerError> {
        self.events.push(GepaEventSummary::ProfileResolved);
        let seed = ctx
            .graph()
            .candidate_tree()
            .roots()
            .first()
            .copied()
            .ok_or_else(|| {
                OptimizerError::Message("GEPA requires at least one seed candidate".to_owned())
            })?;
        if self.reference_state.index_of(seed).is_none() {
            self.events
                .push(GepaEventSummary::SeedValidationStarted { candidate: seed });
            self.validate_candidate(ctx, seed, Vec::new(), true).await?;
            if self.reference_state.index_of(seed).is_none() {
                self.reference_state
                    .add_unvalidated_candidate(seed, Vec::new());
            }
        }
        Ok(())
    }

    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, P>,
    ) -> Result<leaven_engine::StepStatus, OptimizerError> {
        if self.completed_iterations >= self.max_iterations {
            self.best = self
                .reference_state
                .best_candidate()
                .or_else(|| self.population.best());
            let best = self
                .best
                .and_then(|candidate| self.reference_state.index_of(candidate));
            self.events
                .push(GepaEventSummary::OptimizationEnded { best });
            return Ok(leaven_engine::StepStatus::Done);
        }

        let seed = ctx
            .graph()
            .candidate_tree()
            .roots()
            .first()
            .copied()
            .ok_or_else(|| {
                OptimizerError::Message("GEPA requires at least one seed candidate".to_owned())
            })?;

        self.events.push(GepaEventSummary::IterationStarted {
            iteration: self.completed_iterations + 1,
        });
        let evaluation_set = self.batch_sampler.sample_train(&self.train_partition);
        self.events.push(GepaEventSummary::TrainMinibatchSampled);
        let (parent_index, parent) = self
            .reference_state
            .select_by_validation_frontier_frequency()
            .or_else(|| {
                let parent = self.select_candidate(ctx.graph()).unwrap_or(seed);
                self.reference_state
                    .index_of(parent)
                    .map(|index| (index, parent))
            })
            .ok_or_else(|| {
                OptimizerError::Message("GEPA selected a parent outside reference state".to_owned())
            })?;
        self.events.push(GepaEventSummary::ParentSelected {
            candidate_index: parent_index,
        });
        let parent_screening = self
            .evaluate_casewise(
                ctx,
                parent,
                evaluation_set.clone(),
                EvaluationPurpose::SeedBaseline,
            )
            .await?;
        self.reference_state
            .add_metric_calls(parent_screening.metric_calls_new);
        self.events.push(GepaEventSummary::ParentEvaluated {
            metric_calls_delta: parent_screening.metric_calls_new,
        });
        if self.observed.insert(parent) {
            self.candidate_history
                .push(parent_screening.history_entry(parent));
            let events = self.population.observe_gepa(
                Some(&self.train_partition),
                parent,
                &parent_screening.assessments,
                &parent_screening.scalar_evidence,
            );
            ctx.emit(leaven_engine::RunEvent::PopulationUpdated {
                population_id: self.population.id(),
                events,
            });
        }
        for _ in 0..self.proposal_count {
            let Some(candidate) = self
                .propose_candidate(ctx, parent, &parent_screening.assessments)
                .await?
            else {
                continue;
            };

            let screened = self
                .evaluate_casewise(
                    ctx,
                    candidate,
                    evaluation_set.clone(),
                    EvaluationPurpose::Search,
                )
                .await?;
            self.reference_state
                .add_metric_calls(screened.metric_calls_new);
            self.events.push(GepaEventSummary::ChildEvaluated {
                metric_calls_delta: screened.metric_calls_new,
            });
            if self
                .gate
                .decide(parent_screening.average_score, screened.average_score)
                .is_accept()
            {
                self.events
                    .push(GepaEventSummary::ProposalAccepted { child: candidate });
                self.candidate_history
                    .push(screened.history_entry(candidate));
                let events = self.population.observe_gepa(
                    Some(&self.train_partition),
                    candidate,
                    &screened.assessments,
                    &screened.scalar_evidence,
                );
                ctx.emit(leaven_engine::RunEvent::PopulationUpdated {
                    population_id: self.population.id(),
                    events,
                });
                self.best = self.population.best();
                self.validate_candidate(ctx, candidate, vec![parent_index], false)
                    .await?;
                if self.reference_state.index_of(candidate).is_none() {
                    self.reference_state
                        .add_unvalidated_candidate(candidate, vec![parent_index]);
                }
            } else {
                self.events.push(GepaEventSummary::ProposalRejected);
            }
        }
        self.completed_iterations += 1;

        if self.completed_iterations >= self.max_iterations {
            Ok(leaven_engine::StepStatus::Done)
        } else {
            Ok(leaven_engine::StepStatus::Continue)
        }
    }

    fn best_candidate(&self, _graph: RunGraphView<'_, P>) -> Option<CandidateId> {
        self.validation_best
            .as_ref()
            .map(|best| best.candidate)
            .or(self.best)
            .or_else(|| self.population.best())
    }

    fn checkpoint_state_write(
        &self,
        ctx: CheckpointContext<'_, P>,
    ) -> Result<Option<OptimizerStateWrite>, OptimizerError> {
        <Self as CheckpointableOptimizer<P>>::checkpoint_state_write(self, ctx)
    }

    fn restore_checkpoint_state<R>(
        &mut self,
        checkpoint: &leaven_engine::RunCheckpoint,
        reader: &R,
        ctx: RestoreContext<'_, P>,
    ) -> Result<(), OptimizerError>
    where
        R: OptimizerStateReader,
    {
        restore_checkpointable_optimizer_state(self, checkpoint, reader, ctx)
    }
}

impl<P, S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
    CheckpointableOptimizer<P>
    for Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
where
    P: OptimizationProblem,
    P::Evidence: GepaCaseEvidence,
    P::ProposalAnnotations: Default,
    S: EditSurface<P::Artifact> + Send + Sync,
    S::PartId: std::fmt::Debug,
    Pop: CheckpointPopulation + GepaPopulation + Send + Sync,
    Reflect: GepaReflector<P, S> + Send + Sync,
    CandidateSel: CandidateSelector<P, Pop, Selection = Option<CandidateId>>
        + CheckpointCandidateSelector
        + Send
        + Sync,
    PartSel: PartSelector<P::Artifact, S> + CheckpointPartSelector + Send + Sync,
    GatePol: CheckpointGate + Gate + Send + Sync,
    Batch: BatchSampler + CheckpointBatchSampler + Send + Sync,
    Validate: ValidationPolicy + CheckpointValidationPolicy + Send + Sync,
    Dataset: ReflectiveDatasetBuilder<P, S>,
{
    type State = GepaCheckpointState<
        Pop::State,
        CandidateSel::State,
        PartSel::State,
        GatePol::State,
        Batch::State,
        Validate::State,
    >;

    fn optimizer_fingerprint(&self) -> Fingerprint {
        GEPA_OPTIMIZER_FINGERPRINT
    }

    fn private_state_policy(&self) -> PrivateStatePolicy {
        PrivateStatePolicy::ExplicitSnapshot {
            schema: GEPA_CHECKPOINT_SCHEMA,
            format: StateFormat::Json,
        }
    }

    fn checkpoint_state(
        &self,
        ctx: CheckpointContext<'_, P>,
    ) -> Result<Self::State, CheckpointError> {
        let graph = ctx.graph();
        if let Some(best) = self.best {
            ensure_checkpoint_candidate(&graph, best, "best candidate")?;
        }
        if let Some(validation_best) = &self.validation_best {
            ensure_checkpoint_candidate(
                &graph,
                validation_best.candidate,
                "validation best candidate",
            )?;
            for assessment in &validation_best.assessments {
                ensure_checkpoint_assessment(
                    &graph,
                    *assessment,
                    "validation best assessment row",
                )?;
            }
        }
        for observed in &self.observed {
            ensure_checkpoint_candidate(&graph, *observed, "observed candidate")?;
        }
        for entry in &self.candidate_history {
            ensure_checkpoint_candidate(&graph, entry.candidate, "candidate history candidate")?;
            for assessment in entry.assessments() {
                ensure_checkpoint_assessment(
                    &graph,
                    *assessment,
                    "candidate history assessment row",
                )?;
            }
        }
        for record in self.reference_state.records() {
            ensure_checkpoint_candidate(&graph, record.candidate(), "GEPA reference candidate")?;
            for assessment in record.validation_rows() {
                ensure_checkpoint_assessment(&graph, *assessment, "GEPA reference validation row")?;
            }
        }
        if let Some(population_best) = self.population.best() {
            ensure_checkpoint_candidate(&graph, population_best, "population best candidate")?;
        }
        Ok(GepaCheckpointState {
            train_partition: self.train_partition.clone(),
            max_iterations: self.max_iterations,
            proposal_count: self.proposal_count,
            completed_iterations: self.completed_iterations,
            best: self.best,
            validation_best: self.validation_best.clone(),
            observed: self.observed.clone(),
            candidate_history: self.candidate_history.clone(),
            reference_state: self.reference_state.clone(),
            events: self.events.clone(),
            population: CheckpointPopulation::checkpoint_state(&self.population),
            candidate_selector: CheckpointCandidateSelector::checkpoint_state(
                &self.candidate_selector,
            ),
            part_selector: CheckpointPartSelector::checkpoint_state(&self.part_selector),
            gate: CheckpointGate::checkpoint_state(&self.gate),
            batch_sampler: CheckpointBatchSampler::checkpoint_state(&self.batch_sampler),
            validation_policy: CheckpointValidationPolicy::checkpoint_state(
                &self.validation_policy,
            ),
        })
    }

    fn restore_state(
        &mut self,
        state: Self::State,
        ctx: RestoreContext<'_, P>,
    ) -> Result<(), CheckpointError> {
        let graph = ctx.graph();
        if let Some(best) = state.best {
            ensure_checkpoint_candidate(&graph, best, "best candidate")?;
        }
        if let Some(validation_best) = &state.validation_best {
            ensure_checkpoint_candidate(
                &graph,
                validation_best.candidate,
                "validation best candidate",
            )?;
            for assessment in &validation_best.assessments {
                ensure_checkpoint_assessment(
                    &graph,
                    *assessment,
                    "validation best assessment row",
                )?;
            }
        }
        for observed in &state.observed {
            ensure_checkpoint_candidate(&graph, *observed, "observed candidate")?;
        }
        for entry in &state.candidate_history {
            ensure_checkpoint_candidate(&graph, entry.candidate, "candidate history candidate")?;
            for assessment in entry.assessments() {
                ensure_checkpoint_assessment(
                    &graph,
                    *assessment,
                    "candidate history assessment row",
                )?;
            }
        }
        for record in state.reference_state.records() {
            ensure_checkpoint_candidate(&graph, record.candidate(), "GEPA reference candidate")?;
            for assessment in record.validation_rows() {
                ensure_checkpoint_assessment(&graph, *assessment, "GEPA reference validation row")?;
            }
        }
        self.train_partition = state.train_partition;
        self.max_iterations = state.max_iterations;
        self.proposal_count = state.proposal_count;
        self.completed_iterations = state.completed_iterations;
        self.best = state.best;
        self.observed = state.observed;
        self.candidate_history = state.candidate_history;
        self.reference_state = state.reference_state;
        self.events = state.events;
        self.population.restore_state(state.population);
        self.candidate_selector
            .restore_state(state.candidate_selector);
        self.part_selector.restore_state(state.part_selector);
        self.gate.restore_state(state.gate);
        self.batch_sampler.restore_state(state.batch_sampler);
        self.validation_policy
            .restore_state(state.validation_policy);
        self.validation_best = state.validation_best;
        if let Some(population_best) = self.population.best() {
            ensure_checkpoint_candidate(&graph, population_best, "population best candidate")?;
        }
        Ok(())
    }
}

fn ensure_checkpoint_candidate<P>(
    graph: &RunGraphView<'_, P>,
    candidate: CandidateId,
    role: &str,
) -> Result<(), CheckpointError>
where
    P: OptimizationProblem,
{
    if graph.candidate(candidate).is_none() {
        return Err(CheckpointError::MissingGraphTruth {
            reason: format!("{role} `{candidate}` is not in the graph"),
        });
    }
    Ok(())
}

fn ensure_checkpoint_assessment<P>(
    graph: &RunGraphView<'_, P>,
    assessment: AssessmentId,
    role: &str,
) -> Result<(), CheckpointError>
where
    P: OptimizationProblem,
{
    if graph.assessment(assessment).is_none() {
        return Err(CheckpointError::MissingGraphTruth {
            reason: format!("{role} `{assessment}` is not in the graph"),
        });
    }
    Ok(())
}

impl<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
    Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
{
    async fn propose_candidate<P>(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        parent: CandidateId,
        parent_assessments: &[AssessmentId],
    ) -> Result<Option<CandidateId>, OptimizerError>
    where
        P: OptimizationProblem,
        P::ProposalAnnotations: Default,
        S: EditSurface<P::Artifact>,
        S::PartId: std::fmt::Debug,
        Reflect: GepaReflector<P, S>,
        PartSel: PartSelector<P::Artifact, S>,
        Dataset: ReflectiveDatasetBuilder<P, S>,
    {
        let artifact = ctx
            .graph()
            .artifact(parent)
            .ok_or_else(|| {
                OptimizerError::Message(format!(
                    "selected candidate {parent} is missing from graph"
                ))
            })?
            .clone();
        let part = self
            .part_selector
            .select_part(&artifact, &self.surface)
            .map_err(|source| OptimizerError::with_source("GEPA part selection failed", source))?;
        let part_label = format!("{part:?}");
        let examples = self
            .dataset
            .build(ctx, parent, parent_assessments, &part)
            .await
            .map_err(|source| {
                OptimizerError::with_source("GEPA reflective-dataset build failed", source)
            })?;
        if examples.is_empty() {
            self.events.push(GepaEventSummary::ProposalSkipped {
                reason: GepaSkipReason::NoReflectiveExamples,
            });
            return Ok(None);
        }
        self.events.push(GepaEventSummary::ReflectiveDatasetBuilt {
            records: examples.len(),
        });
        let source_refs = std::iter::once(leaven_core::InfoRef::Candidate(parent))
            .chain(
                parent_assessments
                    .iter()
                    .copied()
                    .map(leaven_core::InfoRef::Assessment),
            )
            .collect();
        let request = ReflectRequest {
            parent,
            part,
            part_label,
            examples,
            source_refs,
        };
        let candidate = self
            .reflector
            .reflect_candidate(ctx, &self.surface, request)
            .await?;
        if let Some(candidate) = candidate {
            self.events.push(GepaEventSummary::ChildBuilt { candidate });
        }
        Ok(candidate)
    }

    async fn validate_candidate<P>(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        candidate: CandidateId,
        parents: Vec<GepaCandidateIndex>,
        seed_validation: bool,
    ) -> Result<(), OptimizerError>
    where
        P: OptimizationProblem,
        P::Evidence: GepaCaseEvidence,
        Validate: ValidationPolicy + Sync,
        S: Sync,
        Pop: Sync,
        Reflect: Sync,
        CandidateSel: Sync,
        PartSel: Sync,
        GatePol: Sync,
        Batch: Sync,
        Dataset: Sync,
    {
        let Some(set) = self.validation_policy.validation_set(candidate) else {
            return Ok(());
        };
        let assessment = self
            .evaluate_casewise(ctx, candidate, set, EvaluationPurpose::Validation)
            .await?;
        self.reference_state
            .add_metric_calls(assessment.metric_calls_new);
        self.reference_state.note_full_validation();
        let index = self.reference_state.add_validated_candidate(
            candidate,
            parents,
            self.reference_state.total_metric_calls(),
            &assessment,
        );
        if self
            .validation_best
            .as_ref()
            .is_none_or(|best| assessment.average_score > best.score)
        {
            self.validation_best = Some(GepaValidationBest {
                candidate,
                assessments: assessment.assessments.clone(),
                score: assessment.average_score,
            });
        }
        if seed_validation {
            self.events.push(GepaEventSummary::SeedValidationCompleted {
                candidate_index: index,
                score: assessment.average_score.to_string(),
            });
        } else {
            self.events
                .push(GepaEventSummary::AcceptedValidationCompleted {
                    candidate_index: index,
                });
        }
        self.events.push(GepaEventSummary::FrontierUpdated);
        Ok(())
    }

    async fn evaluate_casewise<P>(
        &self,
        ctx: &mut RunContext<'_, P>,
        candidate: CandidateId,
        set: EvaluationSet,
        purpose: EvaluationPurpose,
    ) -> Result<GepaAssessment, OptimizerError>
    where
        P: OptimizationProblem,
        P::Evidence: GepaCaseEvidence,
        S: Sync,
        Pop: Sync,
        Reflect: Sync,
        CandidateSel: Sync,
        PartSel: Sync,
        GatePol: Sync,
        Batch: Sync,
        Validate: Sync,
        Dataset: Sync,
    {
        let report = ctx
            .evaluate(
                EvaluatorId::PRIMARY,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: purpose.clone(),
                },
            )
            .await
            .map_err(|source| OptimizerError::with_source("GEPA evaluation failed", source))?;
        if report.assessment_ids.is_empty() {
            return Err(OptimizerError::Message(format!(
                "GEPA {purpose:?} expected at least one case assessment row"
            )));
        }
        let mut outcomes = Vec::with_capacity(report.assessment_ids.len());
        for assessment in &report.assessment_ids {
            let assessment_view = ctx.graph().assessment(*assessment).ok_or_else(|| {
                OptimizerError::Message(format!(
                    "GEPA assessment row `{assessment}` is missing from graph"
                ))
            })?;
            let row_candidate = assessment_view.independent_candidate().ok_or_else(|| {
                OptimizerError::Message("GEPA expected independent assessment rows".to_owned())
            })?;
            if row_candidate != candidate {
                return Err(OptimizerError::Message(
                    "GEPA evaluation returned a row for the wrong candidate".to_owned(),
                ));
            }
            let case = match assessment_view.target() {
                AssessmentTarget::Case { case, .. } => *case,
                AssessmentTarget::Unscoped | AssessmentTarget::EvaluationSet(_) => {
                    return Err(OptimizerError::Message(
                        "GEPA expected case-targeted assessment rows".to_owned(),
                    ));
                }
            };
            let evidence = ctx.assessment_evidence(*assessment).map_err(|source| {
                OptimizerError::with_source("GEPA evidence lookup failed", source)
            })?;
            let score = evidence.scalar_score().ok_or_else(|| {
                OptimizerError::Message("GEPA expected comparable case scores".to_owned())
            })?;
            outcomes.push(CaseOutcome::new(case, score));
        }
        let scalar_evidence = CasewiseEvidence::new(outcomes);
        let average_score = average_scalar(&scalar_evidence).ok_or_else(|| {
            OptimizerError::Message("GEPA expected comparable case scores".to_owned())
        })?;
        Ok(GepaAssessment {
            assessments: report.assessment_ids,
            scalar_evidence,
            average_score,
            metric_calls_new: report.cost.metric_calls,
        })
    }
}

struct GepaAssessment {
    assessments: Vec<AssessmentId>,
    scalar_evidence: CasewiseEvidence<ScalarEvidence>,
    average_score: f64,
    metric_calls_new: u64,
}

impl GepaAssessment {
    fn history_entry(&self, candidate: CandidateId) -> GepaCandidateHistoryEntry {
        GepaCandidateHistoryEntry {
            candidate,
            assessments: self.assessments.clone(),
            score: self.average_score,
        }
    }
}

fn average_scalar(evidence: &CasewiseEvidence<ScalarEvidence>) -> Option<f64> {
    if evidence.outcomes().is_empty() {
        return None;
    }
    let total: f64 = evidence
        .outcomes()
        .iter()
        .map(|outcome| outcome.evidence().score())
        .sum();
    let count = u32::try_from(evidence.outcomes().len()).expect("case count fits into u32");
    Some(total / f64::from(count))
}
