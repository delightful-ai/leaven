//! `RunContext` mutation surface.

use std::{collections::BTreeMap, sync::Arc};

use leaven_core::{OptimizationProblem, ProposalBatch};
use leaven_kernel::{
    BudgetExceeded, BudgetSnapshot, CandidateId, Cost, ErrorKind, ErrorRecord, EvaluatorId,
    IterationId, ProposalBatchId, ProposalId, StageCallId, StageId,
};
use leaven_store::EvidenceStore;

use crate::graph::storage::ApplyAttemptOutcome;
use crate::{
    ApplyOneReport, ApplyOutcome, ApplyReport, BudgetHandle, BudgetLedger, CaseSet, DynCallback,
    DynEvaluator, ErrorPolicy, EvaluationCache, EvaluationContext, OptimizerStateWrite,
    ProposalBatchReport, ProposalContext, Proposer, ReadScope, RenderContext, RunCheckpointRequest,
    RunEvent, RunGraph, RunGraphView, RunPersistence, TrustPolicy,
};

use super::proposal_context::StageAttemptEventSink;

mod casewise;
mod evaluation;
mod support;

pub use support::RunContextError;

pub struct RunContext<'a, P: OptimizationProblem> {
    graph: &'a mut RunGraph<P>,
    budget: &'a mut BudgetLedger,
    iteration: Option<IterationId>,
    read_scope: ReadScope,
    trust: TrustPolicy,
    case_set: Option<&'a CaseSet<P::Case>>,
    cache: Option<&'a mut EvaluationCache>,
    evidence_store: Option<&'a dyn EvidenceStore<P::Evidence>>,
    evaluators: Option<&'a BTreeMap<EvaluatorId, Arc<dyn DynEvaluator<P>>>>,
    callbacks: Option<&'a mut [Box<dyn DynCallback<P>>]>,
    persistence: Option<&'a dyn RunPersistence<P>>,
}

impl<'a, P: OptimizationProblem> RunContext<'a, P> {
    pub fn new(graph: &'a mut RunGraph<P>, budget: &'a mut BudgetLedger) -> Self {
        Self {
            graph,
            budget,
            iteration: None,
            read_scope: ReadScope::default(),
            trust: TrustPolicy::default(),
            case_set: None,
            cache: None,
            evidence_store: None,
            evaluators: None,
            callbacks: None,
            persistence: None,
        }
    }

    #[must_use]
    pub fn with_case_set(mut self, case_set: &'a CaseSet<P::Case>) -> Self {
        self.case_set = Some(case_set);
        self
    }

    #[must_use]
    pub fn with_cache(mut self, cache: &'a mut EvaluationCache) -> Self {
        self.cache = Some(cache);
        self
    }

    #[must_use]
    pub fn with_evidence_store(mut self, store: &'a dyn EvidenceStore<P::Evidence>) -> Self {
        self.evidence_store = Some(store);
        self
    }

    #[must_use]
    pub fn with_evaluators(
        mut self,
        evaluators: &'a BTreeMap<EvaluatorId, Arc<dyn DynEvaluator<P>>>,
    ) -> Self {
        self.evaluators = Some(evaluators);
        self
    }

    #[must_use]
    pub fn with_callbacks(mut self, callbacks: &'a mut [Box<dyn DynCallback<P>>]) -> Self {
        self.callbacks = Some(callbacks);
        self
    }

    #[must_use]
    pub fn with_persistence(mut self, persistence: Option<&'a dyn RunPersistence<P>>) -> Self {
        self.persistence = persistence;
        self
    }

    #[must_use]
    pub fn with_trust_policy(mut self, trust: TrustPolicy) -> Self {
        self.read_scope = trust.optimizer_read_scope();
        self.trust = trust;
        self
    }

    #[must_use]
    pub fn graph(&self) -> RunGraphView<'_, P> {
        self.graph.view(self.read_scope.clone())
    }

    #[must_use]
    pub const fn iteration(&self) -> Option<IterationId> {
        self.iteration
    }

    #[must_use]
    pub(crate) fn with_iteration(mut self, iteration: IterationId) -> Self {
        self.iteration = Some(iteration);
        self
    }

    #[must_use]
    pub fn budget(&self) -> BudgetSnapshot {
        self.budget.snapshot()
    }

    /// Looks up one resolved case from the installed case set.
    ///
    /// Returns `None` when no case set is installed or the case is unknown.
    /// This is a read-only observation; it does not mutate the run graph.
    #[must_use]
    pub fn case(&self, case: leaven_kernel::CaseId) -> Option<&P::Case> {
        self.case_set.and_then(|case_set| case_set.get(case))
    }

    /// Persist a clean checkpoint with explicit optimizer/private state.
    ///
    /// Graph truth, budget, and cache state still come from the live context.
    /// The extra state is only the optimizer-owned continuation data that
    /// cannot be derived from graph events alone.
    pub fn checkpoint_with_optimizer_state(
        &self,
        state: OptimizerStateWrite,
    ) -> Result<(), RunContextError> {
        if let Some(persistence) = self.persistence {
            let request =
                RunCheckpointRequest::new(&*self.graph, &*self.budget, self.cache.as_deref())
                    .with_optimizer_state(state)
                    .advance_latest();
            persistence.checkpoint(request)?;
        }
        Ok(())
    }

    pub fn insert_seed(
        &mut self,
        artifact: P::Artifact,
        seed_index: usize,
    ) -> Result<CandidateId, RunContextError> {
        let candidate = self
            .graph
            .insert_seed(artifact, seed_index)
            .map_err(RunContextError::Graph)?;
        self.checkpoint()?;
        Ok(candidate)
    }

    pub fn record_proposal_batch(
        &mut self,
        stage: StageId,
        batch: ProposalBatch<P>,
        cost: Cost,
    ) -> Result<ProposalBatchReport, RunContextError> {
        self.charge(stage.clone(), cost.clone())?;
        let proposal_count = batch.proposals.len();
        let (batch_id, proposal_ids) =
            self.graph
                .record_proposal_batch(stage.clone(), batch, self.iteration);
        self.emit(RunEvent::ProposalBatchProduced {
            iteration: self.iteration,
            batch_id,
            proposer: stage,
            proposal_count,
        });
        for proposal_id in &proposal_ids {
            let proposal = &self.graph.proposals[proposal_id];
            self.emit(RunEvent::ProposalRecorded {
                proposal_id: *proposal_id,
                batch_id,
                effect: RunGraph::<P>::proposal_effect_kind(&proposal.effect),
                causal: proposal.provenance.causal().clone(),
                informed_by_count: proposal.provenance.informed_by_refs().len(),
            });
        }
        self.checkpoint()?;
        Ok(ProposalBatchReport {
            batch_id,
            proposal_ids,
            cost,
        })
    }

    pub async fn propose<T>(
        &mut self,
        proposer: &T,
        request: T::Request,
    ) -> Result<ProposalBatchReport, RunContextError>
    where
        T: Proposer<P>,
    {
        let stage = StageId::from_proposer(proposer.id());
        let proposal_ctx = self.proposal_context(stage.clone());
        let sink = proposal_ctx.stage_attempt_sink();
        let result = proposer.propose(request, proposal_ctx).await;
        self.drain_stage_attempts(&sink);
        let metered = match result {
            Ok(metered) => metered,
            Err(err) => {
                self.emit_stage_error(Some(stage.clone()), ErrorKind::Proposal, &err);
                return Err(err.into());
            }
        };
        self.record_proposal_batch(stage, metered.value, metered.cost)
    }

    pub fn apply_batch(
        &mut self,
        batch_id: ProposalBatchId,
    ) -> Result<ApplyReport, RunContextError> {
        let proposal_ids = self
            .graph
            .proposal_batches
            .get(&batch_id)
            .ok_or(RunContextError::UnknownBatch(batch_id))?
            .proposal_ids
            .clone();
        let mut outcomes = Vec::with_capacity(proposal_ids.len());
        for proposal_id in proposal_ids {
            outcomes.push(self.apply_proposal(proposal_id)?);
        }
        self.checkpoint()?;
        Ok(ApplyReport { batch_id, outcomes })
    }

    pub fn apply_proposal(
        &mut self,
        proposal_id: ProposalId,
    ) -> Result<ApplyOneReport, RunContextError> {
        let attempt = self.graph.apply_proposal_record(proposal_id);
        let outcome = match &attempt.outcome {
            ApplyAttemptOutcome::Success { candidate_id } => {
                self.emit(RunEvent::ApplySucceeded {
                    proposal_id,
                    candidate_id: *candidate_id,
                });
                ApplyOutcome::Success {
                    candidate_id: *candidate_id,
                }
            }
            ApplyAttemptOutcome::Failure { error } => {
                let error = error.clone();
                self.emit(RunEvent::ApplyFailed {
                    proposal_id,
                    error: error.clone(),
                });
                self.emit(RunEvent::Error {
                    stage: None,
                    error: error.clone(),
                    policy: ErrorPolicy::Continued,
                });
                ApplyOutcome::Failure { error }
            }
        };
        Ok(ApplyOneReport {
            proposal_id,
            attempt_id: attempt.id,
            outcome,
        })
    }

    pub fn charge(&mut self, stage: StageId, cost: Cost) -> Result<BudgetSnapshot, BudgetExceeded> {
        match self.budget.charge(stage.clone(), cost.clone()) {
            Ok(snapshot) => {
                self.emit(RunEvent::BudgetCharged {
                    stage,
                    cost,
                    remaining: snapshot.clone(),
                });
                Ok(snapshot)
            }
            Err(error) => {
                self.emit(RunEvent::Error {
                    stage: Some(stage),
                    error: ErrorRecord::from_error(ErrorKind::Budget, &error),
                    policy: ErrorPolicy::StoppedRun,
                });
                Err(error)
            }
        }
    }

    /// Build the proposer-facing context for a stage.
    ///
    /// The context carries the proposer read scope and a stage-scoped budget
    /// handle. Most callers should prefer [`RunContext::propose`].
    pub fn proposal_context(&mut self, stage: StageId) -> ProposalContext<'_, P> {
        let scope = self.trust.proposer_read_scope();
        ProposalContext::new(
            self.graph.view(scope.clone()),
            BudgetHandle::new(self.budget, stage),
            scope,
            StageCallId::new(),
            StageAttemptEventSink::new(),
        )
    }

    /// Build the evaluator-facing context for a stage.
    ///
    /// This is primarily for object-safe evaluator dispatch and tests of the
    /// evaluator contract. Most callers should prefer [`RunContext::evaluate_with`].
    pub fn evaluation_context(&mut self, stage: StageId) -> EvaluationContext<'_, P> {
        let scope = self.trust.evaluator_read_scope();
        EvaluationContext::new(
            self.graph.view(scope.clone()),
            BudgetHandle::new(self.budget, stage),
            scope,
        )
    }

    /// Build the renderer-facing context for a stage.
    pub fn render_context(&mut self, stage: StageId) -> RenderContext<'_, P> {
        let scope = self.trust.renderer_read_scope();
        RenderContext::new(
            self.graph.view(scope.clone()),
            BudgetHandle::new(self.budget, stage),
            scope,
        )
    }

    /// Build the materializer-facing context for a stage.
    pub fn materialize_context(&self) -> crate::MaterializeContext<'_, P> {
        let scope = self.trust.renderer_read_scope();
        crate::MaterializeContext::new(self.graph.view(scope.clone()), self.budget(), scope)
    }

    pub fn emit(&mut self, event: RunEvent) {
        self.graph.record_event(event);
        if let Some(callbacks) = self.callbacks.as_deref_mut() {
            let read_scope = self.trust.callback_read_scope();
            let event = self
                .graph
                .events
                .last()
                .expect("event was just recorded before callback dispatch");
            for callback in callbacks.iter_mut() {
                callback.on_event_dyn(event, self.graph.view(read_scope.clone()));
            }
        }
    }

    fn drain_stage_attempts(&mut self, sink: &StageAttemptEventSink) {
        for pending in sink.drain() {
            self.emit(RunEvent::StageAttemptRecorded {
                stage_call_id: pending.stage_call_id,
                role: pending.role,
                receipt: pending.receipt,
                outcome: pending.outcome,
            });
        }
    }

    fn checkpoint(&self) -> Result<(), RunContextError> {
        if let Some(persistence) = self.persistence {
            let request = RunCheckpointRequest::new(self.graph, self.budget, self.cache.as_deref());
            persistence.checkpoint(request)?;
        }
        Ok(())
    }

    fn emit_stage_error(
        &mut self,
        stage: Option<StageId>,
        kind: ErrorKind,
        err: &(dyn std::error::Error + 'static),
    ) {
        self.emit(RunEvent::Error {
            stage,
            error: ErrorRecord::from_error(kind, err),
            policy: ErrorPolicy::Continued,
        });
    }
}
