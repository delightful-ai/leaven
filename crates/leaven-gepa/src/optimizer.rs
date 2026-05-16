//! Reusable GEPA optimizer loop.

use std::collections::BTreeSet;

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
    async fn initialize(&mut self, _ctx: &mut RunContext<'_, P>) -> Result<(), OptimizerError> {
        Ok(())
    }

    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, P>,
    ) -> Result<leaven_engine::StepStatus, OptimizerError> {
        if self.completed_iterations >= self.max_iterations {
            self.best = self.population.best();
            return Ok(leaven_engine::StepStatus::Done);
        }

        let seed = self
            .select_candidate(ctx.graph())
            .or_else(|| ctx.graph().candidate_tree().roots().first().copied())
            .ok_or_else(|| {
                OptimizerError::Message("GEPA requires at least one seed candidate".to_owned())
            })?;

        let evaluation_set = self.batch_sampler.sample_train(&self.train_partition);
        let parent_baseline = self
            .evaluate_casewise(
                ctx,
                seed,
                evaluation_set.clone(),
                EvaluationPurpose::SeedBaseline,
            )
            .await?;
        if self.observed.insert(seed) {
            self.candidate_history
                .push(parent_baseline.history_entry(seed));
            let events = self.population.observe_gepa(
                Some(&self.train_partition),
                seed,
                &parent_baseline.assessments,
                &parent_baseline.scalar_evidence,
            );
            ctx.emit(leaven_engine::RunEvent::PopulationUpdated {
                population_id: self.population.id(),
                events,
            });
        }
        if self.validation_best.is_none() {
            self.validate_candidate(ctx, seed).await?;
        }

        let parent = self.select_candidate(ctx.graph()).unwrap_or(seed);
        let parent_screening = if parent == seed {
            parent_baseline
        } else {
            self.evaluate_casewise(
                ctx,
                parent,
                evaluation_set.clone(),
                EvaluationPurpose::SeedBaseline,
            )
            .await?
        };
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
            if self
                .gate
                .decide(parent_screening.average_score, screened.average_score)
                .is_accept()
            {
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
                self.validate_candidate(ctx, candidate).await?;
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
        self.train_partition = state.train_partition;
        self.max_iterations = state.max_iterations;
        self.proposal_count = state.proposal_count;
        self.completed_iterations = state.completed_iterations;
        self.best = state.best;
        self.observed = state.observed;
        self.candidate_history = state.candidate_history;
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
        self.reflector
            .reflect_candidate(ctx, &self.surface, request)
            .await
    }

    async fn validate_candidate<P>(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        candidate: CandidateId,
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
                    purpose,
                },
            )
            .await
            .map_err(|source| OptimizerError::with_source("GEPA evaluation failed", source))?;
        if report.assessment_ids.is_empty() {
            return Err(OptimizerError::Message(
                "GEPA expected at least one case assessment row".to_owned(),
            ));
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
        })
    }
}

struct GepaAssessment {
    assessments: Vec<AssessmentId>,
    scalar_evidence: CasewiseEvidence<ScalarEvidence>,
    average_score: f64,
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
