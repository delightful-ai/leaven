//! Reusable GEPA optimizer loop.

mod assessment;
mod checkpoint;
mod config;
mod step;

pub use checkpoint::GepaCheckpointState;
pub use config::GepaProfile;

use assessment::GepaAssessment;
use config::{GepaEventSink, GepaReportSink};

use std::collections::BTreeSet;

use leaven_core::{EvaluationPurpose, EvaluationSet, OptimizationProblem, PartitionId};
use leaven_engine::{
    CheckpointContext, CheckpointableOptimizer, Optimizer, OptimizerCompatibility, OptimizerError,
    OptimizerReportPayload, OptimizerStateReader, OptimizerStateWrite, PrivateStatePolicy,
    RestoreContext, RunContext, RunGraphView, StateFormat, StopReason,
    restore_checkpointable_optimizer_state,
};
use leaven_kernel::{AssessmentId, CandidateId, CaseId, Fingerprint, FingerprintBuilder};
use leaven_population::ParetoFrontier;
use leaven_surface::EditSurface;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    CandidateSelector, CheckpointCandidateSelector, CheckpointGate, CheckpointPartSelector, Gate,
    GepaCandidateIndex, GepaCaseEvidence, GepaEventSummary, GepaPopulation, GepaReferenceState,
    GepaReflectiveDataset, GepaReflector, GepaSkipReason, PartSelector, PopulationBestFallback,
    ReflectRequest, ReflectiveDatasetBuilder, RoundRobinPart, StrictImprovement,
    population::CheckpointPopulation,
    report::GepaReportProfile,
    validation::{
        BatchSampler, CheckpointBatchSampler, CheckpointValidationPolicy, EpochShuffled,
        FullValidation, GepaRandom, ValidationPolicy,
    },
};

const GEPA_OPTIMIZER_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([8; 32]);
const GEPA_CHECKPOINT_SCHEMA: Fingerprint = Fingerprint::from_bytes([13; 32]);

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
    profile: GepaReportProfile,
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
    rng: GepaRandom,
    events: Vec<GepaEventSummary>,
    event_sink: Option<GepaEventSink>,
    report_sink: Option<GepaReportSink>,
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
        update_checkpoint_state(&mut fingerprint, b"profile", &self.profile);
        fingerprint.update(self.proposal_count.to_le_bytes());
        fingerprint.update([u8::from(self.skip_perfect_score)]);
        fingerprint.update(self.perfect_score.to_le_bytes());
        fingerprint.finish()
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
        self.record_event(GepaEventSummary::ProfileResolved {
            profile: self.profile.clone(),
        });
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
        attempt_index: usize,
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
        let reflective_cases: Vec<CaseId> = examples
            .iter()
            .filter_map(|example| example.case_id)
            .collect();
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
            attempt_index: None,
        }
        .with_attempt_index(attempt_index);
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
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use leaven_engine::{BudgetLedger, OptimizerError};
    use leaven_kernel::{Budget, BudgetDimension, BudgetExceeded, Cost, StageId};

    use crate::{GepaEventSummary, GepaReport};

    use super::{
        config::{GepaEventSink, GepaReportSink},
        optimizer_error_contains_budget_exceeded,
    };

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
