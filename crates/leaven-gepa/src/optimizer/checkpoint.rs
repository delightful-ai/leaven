use std::collections::BTreeSet;

use leaven_core::{OptimizationProblem, PartitionId};
use leaven_engine::{
    CheckpointContext, CheckpointError, CheckpointableOptimizer, PrivateStatePolicy,
    RestoreContext, RunGraphView, StateFormat,
};
use leaven_kernel::{AssessmentId, CandidateId, Fingerprint};
use leaven_surface::EditSurface;
use serde::{Deserialize, Serialize};

use crate::{
    CandidateSelector, CheckpointCandidateSelector, CheckpointGate, CheckpointPartSelector,
    CheckpointPopulation, Gate, GepaCaseEvidence, GepaEventSummary, GepaPopulation,
    GepaProposalAttempt, GepaReferenceState, GepaReflector, PartSelector, ReflectiveDatasetBuilder,
    ValidationPolicy,
    validation::{BatchSampler, CheckpointBatchSampler, CheckpointValidationPolicy, GepaRandom},
};

use super::{
    GEPA_CHECKPOINT_SCHEMA, GEPA_OPTIMIZER_FINGERPRINT, Gepa, GepaCandidateHistoryEntry,
    GepaValidationBest,
};

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
    population: PopulationState,
    candidate_selector: CandidateSelectorState,
    part_selector: PartSelectorState,
    gate: GateState,
    batch_sampler: BatchSamplerState,
    validation_policy: ValidationPolicyState,
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
        for attempt in &self.proposal_attempts {
            ensure_checkpoint_candidate(&graph, attempt.parent, "proposal attempt parent")?;
            for assessment in &attempt.parent_assessments {
                ensure_checkpoint_assessment(
                    &graph,
                    *assessment,
                    "proposal attempt parent assessment row",
                )?;
            }
            if let Some(child) = attempt.child {
                ensure_checkpoint_candidate(&graph, child, "proposal attempt child")?;
            }
            for assessment in &attempt.child_assessments {
                ensure_checkpoint_assessment(
                    &graph,
                    *assessment,
                    "proposal attempt child assessment row",
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
            skip_perfect_score: self.skip_perfect_score,
            perfect_score: self.perfect_score,
            completed_iterations: self.completed_iterations,
            best: self.best,
            validation_best: self.validation_best.clone(),
            observed: self.observed.clone(),
            candidate_history: self.candidate_history.clone(),
            proposal_attempts: self.proposal_attempts.clone(),
            reference_state: self.reference_state.clone(),
            rng: self.rng.clone(),
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
        for attempt in &state.proposal_attempts {
            ensure_checkpoint_candidate(&graph, attempt.parent, "proposal attempt parent")?;
            for assessment in &attempt.parent_assessments {
                ensure_checkpoint_assessment(
                    &graph,
                    *assessment,
                    "proposal attempt parent assessment row",
                )?;
            }
            if let Some(child) = attempt.child {
                ensure_checkpoint_candidate(&graph, child, "proposal attempt child")?;
            }
            for assessment in &attempt.child_assessments {
                ensure_checkpoint_assessment(
                    &graph,
                    *assessment,
                    "proposal attempt child assessment row",
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
        self.skip_perfect_score = state.skip_perfect_score;
        self.perfect_score = state.perfect_score;
        self.completed_iterations = state.completed_iterations;
        self.best = state.best;
        self.observed = state.observed;
        self.candidate_history = state.candidate_history;
        self.proposal_attempts = state.proposal_attempts;
        self.reference_state = state.reference_state;
        self.rng = state.rng;
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
