//! Reusable GEPA optimizer loop.

mod checkpoint;
mod step;

pub use checkpoint::GepaCheckpointState;

use std::{collections::BTreeSet, sync::Arc};

use leaven_core::{
    AssessmentGranularity, AssessmentTarget, EvaluationPurpose, EvaluationRequest, EvaluationSet,
    OptimizationProblem, PartitionId,
};
use leaven_engine::{
    CheckpointContext, CheckpointableOptimizer, Optimizer, OptimizerCompatibility, OptimizerError,
    OptimizerReportPayload, OptimizerStateReader, OptimizerStateWrite, PrivateStatePolicy,
    RestoreContext, RunContext, RunGraphView, StateFormat, StopReason,
    restore_checkpointable_optimizer_state,
};
use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence};
use leaven_kernel::{
    AssessmentId, CandidateId, CaseId, EvaluatorId, Fingerprint, FingerprintBuilder,
};
use leaven_population::ParetoFrontier;
use leaven_surface::{EditSurface, SurfaceError};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    CandidateSelector, CheckpointCandidateSelector, CheckpointGate, CheckpointPartSelector, Gate,
    GateDecision, GepaCandidateIndex, GepaCaseEvidence, GepaEventSummary, GepaPopulation,
    GepaReferenceState, GepaReflectiveDataset, GepaReflector, GepaReport, GepaSkipReason,
    PartSelector, PopulationBestFallback, ReflectRequest, ReflectiveDatasetBuilder, RoundRobinPart,
    StrictImprovement,
    population::CheckpointPopulation,
    report::GepaReportInput,
    validation::{
        BatchSampler, CheckpointBatchSampler, CheckpointValidationPolicy, EpochShuffled,
        FullValidation, ValidationPolicy,
    },
};

const GEPA_OPTIMIZER_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([8; 32]);
const DEFAULT_MAX_ITERATIONS: usize = 500;
const GEPA_CHECKPOINT_SCHEMA: Fingerprint = Fingerprint::from_bytes([11; 32]);
const DEFAULT_PERFECT_SCORE: f64 = 1.0;

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
    Reflect = crate::MissingReflector,
    CandidateSel = PopulationBestFallback,
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
    skip_perfect_score: bool,
    perfect_score: f64,
    completed_iterations: usize,
    best: Option<CandidateId>,
    validation_best: Option<GepaValidationBest>,
    observed: BTreeSet<CandidateId>,
    candidate_history: Vec<GepaCandidateHistoryEntry>,
    proposal_attempts: Vec<GepaProposalAttempt>,
    reference_state: GepaReferenceState,
    events: Vec<GepaEventSummary>,
    event_sink: Option<GepaEventSink>,
    report_sink: Option<GepaReportSink>,
}

#[derive(Clone)]
struct GepaEventSink(Arc<dyn Fn(&GepaEventSummary) + Send + Sync>);

impl std::fmt::Debug for GepaEventSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GepaEventSink(..)")
    }
}

#[derive(Clone)]
struct GepaReportSink(Arc<dyn Fn(&GepaReport) + Send + Sync>);

impl std::fmt::Debug for GepaReportSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GepaReportSink(..)")
    }
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
            PopulationBestFallback,
            RoundRobinPart::new(),
            StrictImprovement,
        )
    }
}

/// One GEPA proposal attempt, including skipped and rejected attempts.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GepaProposalAttempt {
    /// Monotonic proposal attempt ordinal, one-based.
    pub attempt_index: usize,
    /// GEPA iteration number, one-based.
    pub iteration: usize,
    /// Selected parent GEPA index.
    pub parent_index: GepaCandidateIndex,
    /// Selected parent candidate.
    pub parent: CandidateId,
    /// Parent train-screening assessment rows.
    pub parent_assessments: Vec<AssessmentId>,
    /// Parent train-screening case IDs.
    pub parent_cases: Vec<CaseId>,
    /// Parent average train-screening score.
    pub parent_score: f64,
    /// Selected surface part label, when part selection happened.
    pub part_label: Option<String>,
    /// Reflective examples supplied to the reflector, when built.
    pub reflective_example_count: Option<usize>,
    /// Child candidate produced by reflection, when any.
    pub child: Option<CandidateId>,
    /// Child train-screening assessment rows.
    pub child_assessments: Vec<AssessmentId>,
    /// Child train-screening case IDs.
    pub child_cases: Vec<CaseId>,
    /// Child average train-screening score.
    pub child_score: Option<f64>,
    /// Train-screening acceptance decision, when a child was screened.
    pub accepted: Option<bool>,
    /// GEPA candidate index assigned after accepted-child validation/admission.
    pub admitted_index: Option<GepaCandidateIndex>,
    /// Skip reason for attempts stopped before child screening.
    pub skip_reason: Option<GepaSkipReason>,
}

struct ProposalOutcome {
    candidate: Option<CandidateId>,
    part_label: Option<String>,
    reflective_example_count: Option<usize>,
    skip_reason: Option<GepaSkipReason>,
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
            skip_perfect_score: true,
            perfect_score: DEFAULT_PERFECT_SCORE,
            completed_iterations: 0,
            best: None,
            validation_best: None,
            observed: BTreeSet::new(),
            candidate_history: Vec::new(),
            proposal_attempts: Vec::new(),
            reference_state: GepaReferenceState::default(),
            events: Vec::new(),
            event_sink: None,
            report_sink: None,
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
            skip_perfect_score: self.skip_perfect_score,
            perfect_score: self.perfect_score,
            completed_iterations: self.completed_iterations,
            best: self.best,
            validation_best: self.validation_best,
            observed: self.observed,
            candidate_history: self.candidate_history,
            proposal_attempts: self.proposal_attempts,
            reference_state: self.reference_state,
            events: self.events,
            event_sink: self.event_sink,
            report_sink: self.report_sink,
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
            skip_perfect_score: self.skip_perfect_score,
            perfect_score: self.perfect_score,
            completed_iterations: self.completed_iterations,
            best: self.best,
            validation_best: self.validation_best,
            observed: self.observed,
            candidate_history: self.candidate_history,
            proposal_attempts: self.proposal_attempts,
            reference_state: self.reference_state,
            events: self.events,
            event_sink: self.event_sink,
            report_sink: self.report_sink,
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
            skip_perfect_score: self.skip_perfect_score,
            perfect_score: self.perfect_score,
            completed_iterations: self.completed_iterations,
            best: self.best,
            validation_best: self.validation_best,
            observed: self.observed,
            candidate_history: self.candidate_history,
            proposal_attempts: self.proposal_attempts,
            reference_state: self.reference_state,
            events: self.events,
            event_sink: self.event_sink,
            report_sink: self.report_sink,
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

    /// Enable or disable upstream GEPA's all-perfect parent-minibatch skip.
    #[must_use]
    pub const fn skip_perfect_score(mut self, skip: bool) -> Self {
        self.skip_perfect_score = skip;
        self
    }

    /// Set the score threshold considered perfect by the skip-perfect policy.
    #[must_use]
    pub const fn perfect_score(mut self, perfect_score: f64) -> Self {
        self.perfect_score = perfect_score;
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

    /// Detailed GEPA report snapshot for accepted candidates and validation frontier state.
    #[must_use]
    pub fn report(&self) -> GepaReport {
        GepaReport::from_reference_state(&GepaReportInput {
            reference_state: &self.reference_state,
            candidate_history: &self.candidate_history,
            proposal_attempts: &self.proposal_attempts,
            events: &self.events,
            best_candidate: self.best,
            validation_best_candidate: self.validation_best.as_ref().map(|best| best.candidate),
            skip_perfect_score: self.skip_perfect_score,
            perfect_score: self.perfect_score,
        })
    }

    /// Register a GEPA phase event observer.
    ///
    /// This observes optimizer-level GEPA phases without requiring callers to
    /// parse generic engine events. The sink is intentionally not checkpointed;
    /// resumed runs install the observer from the fresh builder configuration.
    #[must_use]
    pub fn on_event<F>(mut self, sink: F) -> Self
    where
        F: Fn(&GepaEventSummary) + Send + Sync + 'static,
    {
        self.event_sink = Some(GepaEventSink(Arc::new(sink)));
        self
    }

    /// Register a detailed GEPA report observer.
    ///
    /// The sink is called when GEPA reaches a terminal optimizer status. It is
    /// intentionally not checkpointed; resumed runs install observers from the
    /// fresh builder configuration.
    #[must_use]
    pub fn on_report<F>(mut self, sink: F) -> Self
    where
        F: Fn(&GepaReport) + Send + Sync + 'static,
    {
        self.report_sink = Some(GepaReportSink(Arc::new(sink)));
        self
    }

    pub(crate) fn record_event(&mut self, event: GepaEventSummary) {
        if let Some(sink) = &self.event_sink {
            (sink.0)(&event);
        }
        self.events.push(event);
    }

    pub(crate) fn emit_report(&self) {
        if let Some(sink) = &self.report_sink {
            (sink.0)(&self.report());
        }
    }

    fn optimizer_compatibility_fingerprint(&self) -> Fingerprint
    where
        CandidateSel: CheckpointCandidateSelector,
        PartSel: CheckpointPartSelector,
        GatePol: CheckpointGate,
        Batch: CheckpointBatchSampler,
        Validate: CheckpointValidationPolicy,
    {
        let mut fingerprint = FingerprintBuilder::new();
        fingerprint.update(b"leaven-gepa.optimizer-compatibility.v1");
        fingerprint.update(GEPA_OPTIMIZER_FINGERPRINT.0);
        fingerprint.update(GEPA_CHECKPOINT_SCHEMA.0);
        update_type::<S>(&mut fingerprint);
        update_type::<Pop>(&mut fingerprint);
        update_type::<Reflect>(&mut fingerprint);
        update_type::<CandidateSel>(&mut fingerprint);
        update_type::<PartSel>(&mut fingerprint);
        update_type::<GatePol>(&mut fingerprint);
        update_type::<Batch>(&mut fingerprint);
        update_type::<Validate>(&mut fingerprint);
        update_type::<Dataset>(&mut fingerprint);
        update_checkpoint_state(
            &mut fingerprint,
            b"candidate-selector-state",
            &CheckpointCandidateSelector::checkpoint_state(&self.candidate_selector),
        );
        update_checkpoint_state(
            &mut fingerprint,
            b"part-selector-state",
            &CheckpointPartSelector::checkpoint_state(&self.part_selector),
        );
        update_checkpoint_state(
            &mut fingerprint,
            b"gate-state",
            &CheckpointGate::checkpoint_state(&self.gate),
        );
        update_checkpoint_state(
            &mut fingerprint,
            b"batch-sampler-state",
            &CheckpointBatchSampler::checkpoint_state(&self.batch_sampler),
        );
        update_checkpoint_state(
            &mut fingerprint,
            b"validation-policy-state",
            &CheckpointValidationPolicy::checkpoint_state(&self.validation_policy),
        );
        fingerprint.update(self.train_partition.0.as_str().as_bytes());
        fingerprint.update(self.max_iterations.to_le_bytes());
        fingerprint.update(self.proposal_count.to_le_bytes());
        fingerprint.update([u8::from(self.skip_perfect_score)]);
        fingerprint.update(self.perfect_score.to_le_bytes());
        fingerprint.finish()
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

fn update_type<T>(fingerprint: &mut FingerprintBuilder) {
    fingerprint.update(std::any::type_name::<T>().as_bytes());
}

fn update_checkpoint_state<T>(fingerprint: &mut FingerprintBuilder, label: &[u8], state: &T)
where
    T: Serialize + DeserializeOwned,
{
    fingerprint.update(label);
    match serde_json::to_vec(state) {
        Ok(bytes) => {
            fingerprint.update(b"ok");
            fingerprint.update(bytes);
        }
        Err(error) => {
            fingerprint.update(b"error");
            fingerprint.update(error.to_string().as_bytes());
        }
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
        self.record_event(GepaEventSummary::ProfileResolved);
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
            self.record_event(GepaEventSummary::SeedValidationStarted { candidate: seed });
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
        if let Some(status) = self.finish_if_iteration_limit() {
            return Ok(status);
        }

        let seed = Self::seed_candidate(ctx)?;
        if let Err(error) = self.run_iteration(ctx, seed).await {
            if optimizer_error_contains_budget_exceeded(&error) {
                return Ok(self.finish_for_budget_stop());
            }
            return Err(error);
        }
        self.completed_iterations += 1;

        if self.completed_iterations >= self.max_iterations {
            Ok(self
                .finish_if_iteration_limit()
                .unwrap_or(leaven_engine::StepStatus::Done))
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

    fn optimizer_report(&self) -> Option<OptimizerReportPayload> {
        Some(std::sync::Arc::new(self.report()))
    }

    fn optimizer_compatibility(&self) -> Option<OptimizerCompatibility> {
        Some(OptimizerCompatibility::new(
            self.optimizer_compatibility_fingerprint(),
            PrivateStatePolicy::ExplicitSnapshot {
                schema: GEPA_CHECKPOINT_SCHEMA,
                format: StateFormat::Json,
            },
        ))
    }

    fn on_engine_stop(&mut self, _reason: StopReason) -> Result<(), OptimizerError> {
        self.finish_for_engine_stop();
        Ok(())
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

fn optimizer_error_contains_budget_exceeded(error: &OptimizerError) -> bool {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(source) = current {
        if source.is::<leaven_kernel::BudgetExceeded>() {
            return true;
        }
        if matches!(
            source.downcast_ref::<leaven_engine::RunContextError>(),
            Some(leaven_engine::RunContextError::Budget(_))
        ) {
            return true;
        }
        current = source.source();
    }
    false
}

impl<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
    Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
{
    async fn propose_candidate<P>(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        parent: CandidateId,
        parent_screening: &GepaAssessment,
    ) -> Result<ProposalOutcome, OptimizerError>
    where
        P: OptimizationProblem,
        P::ProposalAnnotations: Default,
        S: EditSurface<P::Artifact>,
        S::PartId: std::fmt::Debug,
        Reflect: GepaReflector<P, S>,
        PartSel: PartSelector<P::Artifact, S>,
        Dataset: ReflectiveDatasetBuilder<P, S>,
    {
        let parent_assessments = &parent_screening.assessments;
        if self.skip_perfect_score && parent_screening.all_scores_at_least(self.perfect_score) {
            self.record_event(GepaEventSummary::ProposalSkipped {
                reason: GepaSkipReason::AllScoresPerfect,
            });
            return Ok(ProposalOutcome {
                candidate: None,
                part_label: None,
                reflective_example_count: None,
                skip_reason: Some(GepaSkipReason::AllScoresPerfect),
            });
        }
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
        let reflective_example_count = examples.len();
        let reflective_cases: Vec<CaseId> =
            examples.iter().filter_map(|example| example.case).collect();
        self.record_event(GepaEventSummary::ReflectiveDatasetBuilt {
            records: reflective_example_count,
            cases: reflective_cases.clone(),
            source_ref_count: parent_assessments.len() + 1,
        });
        if examples.is_empty() {
            self.record_event(GepaEventSummary::ProposalSkipped {
                reason: GepaSkipReason::NoReflectiveExamples,
            });
            return Ok(ProposalOutcome {
                candidate: None,
                part_label: Some(part_label),
                reflective_example_count: Some(0),
                skip_reason: Some(GepaSkipReason::NoReflectiveExamples),
            });
        }
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
            part_label: part_label.clone(),
            examples,
            source_refs,
        };
        self.record_event(GepaEventSummary::ReflectionStarted {
            parent,
            part_label: part_label.clone(),
            records: reflective_example_count,
            cases: reflective_cases,
            source_ref_count: parent_assessments.len() + 1,
        });
        let candidate = self
            .reflector
            .reflect_candidate(ctx, &self.surface, request)
            .await?;
        self.record_event(GepaEventSummary::ReflectionCompleted {
            parent,
            child: candidate,
        });
        if let Some(candidate) = candidate {
            self.record_event(GepaEventSummary::ChildBuilt { candidate });
        }
        Ok(ProposalOutcome {
            candidate,
            part_label: Some(part_label),
            reflective_example_count: Some(reflective_example_count),
            skip_reason: None,
        })
    }

    async fn validate_candidate<P>(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        candidate: CandidateId,
        parents: Vec<GepaCandidateIndex>,
        seed_validation: bool,
    ) -> Result<Option<GepaCandidateIndex>, OptimizerError>
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
            return Ok(None);
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
            assessment.average_score,
            assessment.assessments.clone(),
            &assessment.scalar_evidence,
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
            self.record_event(GepaEventSummary::SeedValidationCompleted {
                candidate_index: index,
                score: assessment.average_score.to_string(),
            });
        } else {
            self.record_event(GepaEventSummary::AcceptedValidationCompleted {
                candidate_index: index,
            });
            self.record_event(GepaEventSummary::CandidateAdmitted {
                candidate,
                candidate_index: index,
            });
        }
        self.record_event(GepaEventSummary::FrontierUpdated);
        Ok(Some(index))
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
        Self::ensure_non_empty_casewise_set(ctx, candidate, &set, &purpose)?;
        let report = ctx
            .evaluate_independent_casewise_cached(
                EvaluatorId::PRIMARY,
                candidate,
                set,
                purpose.clone(),
            )
            .await
            .map_err(|source| OptimizerError::with_source("GEPA evaluation failed", source))?;
        let metric_calls_new = report.cost.metric_calls;
        let assessments = report.assessment_ids;
        if assessments.is_empty() {
            return Err(OptimizerError::Message(format!(
                "GEPA {purpose:?} expected at least one case assessment row"
            )));
        }
        let mut outcomes = Vec::with_capacity(assessments.len());
        for assessment in &assessments {
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
            assessments,
            scalar_evidence,
            average_score,
            metric_calls_new,
        })
    }

    fn ensure_non_empty_casewise_set<P>(
        ctx: &RunContext<'_, P>,
        candidate: CandidateId,
        set: &EvaluationSet,
        purpose: &EvaluationPurpose,
    ) -> Result<(), OptimizerError>
    where
        P: OptimizationProblem,
    {
        let request = EvaluationRequest::Independent {
            candidates: vec![candidate],
            set: set.clone(),
            granularity: AssessmentGranularity::PerCase,
            purpose: purpose.clone(),
        };
        let resolved = match ctx.resolve_evaluation_request(&request) {
            Ok(resolved) => resolved,
            Err(source) if matches!(purpose, EvaluationPurpose::Validation) => {
                return Err(OptimizerError::with_source(
                    reference_validation_required_message(),
                    source,
                ));
            }
            Err(source) => {
                return Err(OptimizerError::with_source(
                    "GEPA could not resolve casewise evaluation set",
                    source,
                ));
            }
        };
        if !resolved.case_ids.is_empty() {
            return Ok(());
        }
        let reason = match purpose {
            EvaluationPurpose::Validation => reference_validation_required_message(),
            _ => "GEPA casewise evaluation requires at least one visible case",
        };
        Err(OptimizerError::Message(reason.to_owned()))
    }
}

fn reference_validation_required_message() -> &'static str {
    "GEPA reference profile requires a non-empty validation set; supply `.validation(...)` or choose an explicit non-reference fallback profile"
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

    fn all_scores_at_least(&self, threshold: f64) -> bool {
        self.scalar_evidence
            .outcomes()
            .iter()
            .all(|outcome| outcome.evidence().score() >= threshold)
    }

    fn cases(&self) -> Vec<CaseId> {
        self.scalar_evidence
            .outcomes()
            .iter()
            .map(CaseOutcome::case)
            .collect()
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use leaven_engine::{BudgetLedger, OptimizerError};
    use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence};
    use leaven_kernel::{
        AssessmentId, Budget, BudgetDimension, BudgetExceeded, CandidateId, CaseId, Cost, StageId,
    };

    use crate::{GepaEventSummary, GepaReport};

    use super::{
        GepaAssessment, GepaCandidateHistoryEntry, GepaEventSink, GepaReportSink, average_scalar,
        optimizer_error_contains_budget_exceeded,
    };

    #[test]
    fn assessment_helpers_preserve_casewise_average_and_history_rows() {
        let evidence = CasewiseEvidence::new(vec![
            CaseOutcome::new(CaseId::new(0), ScalarEvidence::new(0.25).unwrap()),
            CaseOutcome::new(CaseId::new(1), ScalarEvidence::new(0.75).unwrap()),
        ]);
        assert_eq!(average_scalar(&evidence), Some(0.5));
        assert_eq!(
            average_scalar(&CasewiseEvidence::<ScalarEvidence>::new(Vec::new())),
            None
        );

        let candidate = CandidateId::new();
        let rows = vec![AssessmentId::new(), AssessmentId::new()];
        let assessment = GepaAssessment {
            assessments: rows.clone(),
            scalar_evidence: evidence,
            average_score: 0.5,
            metric_calls_new: 2,
        };
        let entry: GepaCandidateHistoryEntry = assessment.history_entry(candidate);

        assert_eq!(entry.candidate(), candidate);
        assert_eq!(entry.assessments(), rows.as_slice());
        assert!((entry.score() - 0.5).abs() < f64::EPSILON);
        assert_eq!(assessment.metric_calls_new, 2);
        assert_eq!(assessment.scalar_evidence.outcomes().len(), 2);
    }

    #[test]
    fn event_and_report_sinks_have_stable_debug_names() {
        let event_sink = GepaEventSink(Arc::new(|_: &GepaEventSummary| {}));
        let report_sink = GepaReportSink(Arc::new(|_: &GepaReport| {}));

        assert_eq!(format!("{event_sink:?}"), "GepaEventSink(..)");
        assert_eq!(format!("{report_sink:?}"), "GepaReportSink(..)");
    }

    #[test]
    fn optimizer_error_budget_detection_walks_source_chain() {
        let plain = OptimizerError::Message("plain failure".to_owned());
        assert!(!optimizer_error_contains_budget_exceeded(&plain));

        let exceeded = BudgetExceeded {
            stage: StageId::custom("test"),
            requested: Box::new(Cost::metric_calls(1)),
            snapshot: Box::new(BudgetLedger::new(Budget::metric_calls(0)).snapshot()),
            dimension: BudgetDimension::MetricCalls,
        };
        let wrapped = OptimizerError::with_source("wrapped budget refusal", exceeded);

        assert!(optimizer_error_contains_budget_exceeded(&wrapped));
    }
}
