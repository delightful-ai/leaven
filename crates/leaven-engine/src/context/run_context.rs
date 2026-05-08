//! `RunContext` mutation surface.

use std::{collections::BTreeMap, sync::Arc};

use leaven_core::{
    Artifact, Assessment, CacheIdentity, EvaluationRequest, OptimizationProblem, ProposalBatch,
    ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_kernel::{
    AssessmentId, BudgetExceeded, BudgetSnapshot, CandidateId, Cost, ErrorKind, ErrorRecord,
    EvaluationRequestId, EvaluatorId, IterationId, ProposalBatchId, ProposalId, StageId,
};
use leaven_store::{EvidenceStore, StoreError};
use thiserror::Error;

use crate::graph::storage::AssessmentRecordTarget;
use crate::graph::storage::{ApplyAttemptOutcome, ApplyProposalError};
use crate::{
    ApplyOneReport, ApplyOutcome, ApplyReport, BudgetHandle, BudgetLedger, CachePolicy,
    CacheStatus, CaseSet, DynCallback, DynEvaluator, ErrorPolicy, EvaluationCache,
    EvaluationCacheKey, EvaluationContext, EvaluationError, EvaluationReport,
    EvaluationResolveError, Evaluator, ProposalBatchReport, ProposalContext, ProposalError,
    Proposer, ReadScope, RenderContext, RunEvent, RunGraph, RunGraphView, RunPersistence,
    TrustPolicy, TrustViolation,
};

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
                causal: proposal.provenance.causal.clone(),
                informed_by_count: proposal.provenance.informed_by.len(),
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
        let metered = proposer
            .propose(request, proposal_ctx)
            .await
            .inspect_err(|err| {
                self.emit_stage_error(Some(stage.clone()), ErrorKind::Proposal, err);
            })?;
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

    /// Resolve the evaluation set inside an evaluation request.
    pub fn resolve_evaluation_request(
        &self,
        request: &EvaluationRequest,
    ) -> Result<leaven_core::ResolvedEvaluationSet, RunContextError> {
        let set = match request {
            EvaluationRequest::Independent { set, .. }
            | EvaluationRequest::Pairwise { set, .. }
            | EvaluationRequest::Listwise { set, .. } => set,
        };
        self.case_set
            .ok_or(RunContextError::MissingCaseSet)?
            .resolve(set)
            .map_err(Into::into)
    }

    /// Evaluate a request, store assessment evidence, and record durable events.
    pub async fn evaluate_with<T>(
        &mut self,
        evaluator: &T,
        request: EvaluationRequest,
    ) -> Result<EvaluationReport, RunContextError>
    where
        T: Evaluator<P>,
    {
        self.evaluate_static(evaluator, request).await
    }

    /// Evaluate through the engine-owned evaluator registry.
    pub async fn evaluate(
        &mut self,
        evaluator_id: EvaluatorId,
        request: EvaluationRequest,
    ) -> Result<EvaluationReport, RunContextError> {
        let Some(evaluator) = self
            .evaluators
            .and_then(|evaluators| evaluators.get(&evaluator_id))
            .cloned()
        else {
            let error = RunContextError::UnknownEvaluator(evaluator_id);
            self.emit(RunEvent::Error {
                stage: Some(StageId::custom("optimizer")),
                error: ErrorRecord::from_error(ErrorKind::Evaluation, &error),
                policy: ErrorPolicy::Continued,
            });
            return Err(error);
        };
        self.evaluate_dyn(evaluator.as_ref(), request).await
    }

    async fn evaluate_static<T>(
        &mut self,
        evaluator: &T,
        request: EvaluationRequest,
    ) -> Result<EvaluationReport, RunContextError>
    where
        T: Evaluator<P>,
    {
        let evaluator_id = evaluator.id();
        if let Err(error) = self
            .trust
            .check_evaluation_request(&crate::Actor::Optimizer, &request)
        {
            self.emit(RunEvent::Error {
                stage: Some(StageId::custom("optimizer")),
                error: ErrorRecord::from_error(ErrorKind::Trust, &error),
                policy: ErrorPolicy::Continued,
            });
            return Err(RunContextError::TrustViolation(error));
        }
        let resolved_set = self.resolve_evaluation_request(&request)?;
        let resolved_request = ResolvedEvaluationRequest {
            kind: resolved_kind(&request),
            set: resolved_set.clone(),
            granularity: request_granularity(&request),
            purpose: request_purpose(&request),
        };
        let policy = evaluator.cache_policy(&resolved_request);
        let cache_key =
            self.evaluation_cache_key(evaluator.fingerprint(), policy.clone(), &resolved_request);
        let request_id = self.record_evaluation_request(
            &evaluator_id,
            request,
            resolved_set.clone(),
            candidate_count(&resolved_request),
        );
        if let Some(report) = self.cached_evaluation_report(
            &evaluator_id,
            request_id,
            &resolved_request,
            &policy,
            cache_key.as_ref(),
        ) {
            return Ok(report);
        }

        let stage = StageId::from_evaluator(evaluator_id.clone());
        let eval_ctx = self.evaluation_context(stage.clone());
        let metered = evaluator
            .evaluate(resolved_request.clone(), eval_ctx)
            .await
            .inspect_err(|err| {
                self.emit_stage_error(Some(stage.clone()), ErrorKind::Evaluation, err);
            })
            .map_err(RunContextError::Evaluation)?;
        self.complete_evaluation(
            &evaluator_id,
            request_id,
            &resolved_request,
            &policy,
            cache_key,
            metered,
        )
    }

    async fn evaluate_dyn(
        &mut self,
        evaluator: &dyn DynEvaluator<P>,
        request: EvaluationRequest,
    ) -> Result<EvaluationReport, RunContextError> {
        let evaluator_id = evaluator.id();
        if let Err(error) = self
            .trust
            .check_evaluation_request(&crate::Actor::Optimizer, &request)
        {
            self.emit(RunEvent::Error {
                stage: Some(StageId::custom("optimizer")),
                error: ErrorRecord::from_error(ErrorKind::Trust, &error),
                policy: ErrorPolicy::Continued,
            });
            return Err(RunContextError::TrustViolation(error));
        }
        let resolved_set = self.resolve_evaluation_request(&request)?;
        let resolved_request = ResolvedEvaluationRequest {
            kind: resolved_kind(&request),
            set: resolved_set.clone(),
            granularity: request_granularity(&request),
            purpose: request_purpose(&request),
        };
        let policy = evaluator.cache_policy(&resolved_request);
        let cache_key =
            self.evaluation_cache_key(evaluator.fingerprint(), policy.clone(), &resolved_request);
        let request_id = self.record_evaluation_request(
            &evaluator_id,
            request,
            resolved_set.clone(),
            candidate_count(&resolved_request),
        );
        if let Some(report) = self.cached_evaluation_report(
            &evaluator_id,
            request_id,
            &resolved_request,
            &policy,
            cache_key.as_ref(),
        ) {
            return Ok(report);
        }

        let stage = StageId::from_evaluator(evaluator_id.clone());
        let eval_ctx = self.evaluation_context(stage.clone());
        let metered = evaluator
            .evaluate_boxed(resolved_request.clone(), eval_ctx)
            .await
            .inspect_err(|err| {
                self.emit_stage_error(Some(stage.clone()), ErrorKind::Evaluation, err);
            })
            .map_err(RunContextError::Evaluation)?;
        self.complete_evaluation(
            &evaluator_id,
            request_id,
            &resolved_request,
            &policy,
            cache_key,
            metered,
        )
    }

    fn complete_evaluation(
        &mut self,
        evaluator_id: &EvaluatorId,
        request_id: EvaluationRequestId,
        resolved_request: &ResolvedEvaluationRequest,
        policy: &CachePolicy,
        cache_key: Option<EvaluationCacheKey>,
        metered: leaven_kernel::Metered<Vec<Assessment<P>>>,
    ) -> Result<EvaluationReport, RunContextError> {
        let stage = StageId::from_evaluator(evaluator_id.clone());
        self.charge(stage, metered.cost.clone())?;
        let assessment_ids = self.record_assessments(request_id, evaluator_id, metered.value)?;
        let cache = if matches!(policy, &CachePolicy::Never) || cache_key.is_none() {
            CacheStatus::Bypassed
        } else {
            if let (Some(cache), Some(cache_key)) = (self.cache.as_mut(), cache_key) {
                cache.insert(cache_key, assessment_ids.clone());
                self.checkpoint()?;
            }
            CacheStatus::Miss
        };
        let report = EvaluationReport {
            request_id,
            resolved_set: resolved_request.set.id,
            assessment_ids,
            cost: metered.cost,
            cache,
        };
        self.emit_evaluation_completed(evaluator_id, &report);
        Ok(report)
    }

    fn cached_evaluation_report(
        &mut self,
        evaluator: &EvaluatorId,
        request_id: EvaluationRequestId,
        resolved_request: &ResolvedEvaluationRequest,
        policy: &CachePolicy,
        cache_key: Option<&EvaluationCacheKey>,
    ) -> Option<EvaluationReport> {
        if matches!(policy, CachePolicy::Never) {
            return None;
        }
        let cache_key = cache_key?;
        let assessment_ids = self
            .cache
            .as_ref()
            .and_then(|cache| cache.get(cache_key))
            .cloned()?;
        let report = EvaluationReport {
            request_id,
            resolved_set: resolved_request.set.id,
            assessment_ids,
            cost: Cost::zero(),
            cache: CacheStatus::Hit,
        };
        self.emit_evaluation_completed(evaluator, &report);
        Some(report)
    }

    fn evaluation_cache_key(
        &self,
        evaluator: leaven_kernel::Fingerprint,
        policy: CachePolicy,
        request: &ResolvedEvaluationRequest,
    ) -> Option<EvaluationCacheKey> {
        evaluation_cache_key(evaluator, policy, request, self.graph())
    }

    fn record_evaluation_request(
        &mut self,
        evaluator: &EvaluatorId,
        request: EvaluationRequest,
        resolved_set: leaven_core::ResolvedEvaluationSet,
        candidate_count: usize,
    ) -> EvaluationRequestId {
        let request_id =
            self.graph
                .record_evaluation_request(evaluator.clone(), request, resolved_set);
        self.emit(RunEvent::EvaluationRequested {
            request_id,
            evaluator: evaluator.clone(),
            request: crate::EvaluationRequestSummary { candidate_count },
        });
        request_id
    }

    fn emit_evaluation_completed(&mut self, evaluator: &EvaluatorId, report: &EvaluationReport) {
        self.emit(RunEvent::EvaluationCompleted {
            request_id: report.request_id,
            evaluator: evaluator.clone(),
            assessment_ids: report.assessment_ids.clone(),
            cost: report.cost.clone(),
            cache: report.cache,
        });
    }

    fn record_assessments(
        &mut self,
        request_id: EvaluationRequestId,
        evaluator: &EvaluatorId,
        assessments: Vec<Assessment<P>>,
    ) -> Result<Vec<AssessmentId>, RunContextError> {
        let store = self
            .evidence_store
            .ok_or(RunContextError::MissingEvidenceStore)?;
        let mut ids = Vec::with_capacity(assessments.len());
        for assessment in assessments {
            let (target, evidence, metadata) = assessment_parts(assessment);
            let reference = store.put(evidence).inspect_err(|err| {
                self.emit_stage_error(
                    Some(StageId::from_evaluator(evaluator.clone())),
                    ErrorKind::Store,
                    err,
                );
            })?;
            ids.push(self.graph.record_assessment(
                request_id,
                evaluator.clone(),
                target,
                metadata,
                reference,
            ));
        }
        self.checkpoint()?;
        Ok(ids)
    }

    pub fn assessment_evidence(
        &self,
        assessment_id: AssessmentId,
    ) -> Result<P::Evidence, RunContextError> {
        let reference = self
            .graph()
            .assessment(assessment_id)
            .map(|assessment| assessment.evidence_ref().clone())
            .ok_or(RunContextError::UnknownAssessment(assessment_id))?;
        self.evidence_store
            .ok_or(RunContextError::MissingEvidenceStore)?
            .get(&reference)
            .map_err(RunContextError::Store)
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

    fn checkpoint(&self) -> Result<(), RunContextError> {
        if let Some(persistence) = self.persistence {
            persistence.checkpoint(self.graph)?;
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

/// Failures from the public run-context mutation surface.
#[derive(Debug, Error)]
pub enum RunContextError {
    /// Graph insertion or proposal-application refusal.
    #[error(transparent)]
    Graph(#[from] ApplyProposalError),
    /// Evaluation-set resolution refused the request.
    #[error(transparent)]
    EvaluationResolve(#[from] EvaluationResolveError),
    /// The requested proposal batch is not present in the graph.
    #[error("unknown proposal batch: {0}")]
    UnknownBatch(ProposalBatchId),
    /// The requested evaluator is not registered in the engine.
    #[error("unknown evaluator: {0}")]
    UnknownEvaluator(EvaluatorId),
    /// The requested assessment is not visible or not present in the graph.
    #[error("unknown assessment: {0}")]
    UnknownAssessment(AssessmentId),
    /// A budget ledger refused a charge.
    #[error(transparent)]
    Budget(#[from] BudgetExceeded),
    /// A proposer refused its request.
    #[error(transparent)]
    Proposal(#[from] ProposalError),
    /// An evaluator refused its request.
    #[error(transparent)]
    Evaluation(#[from] EvaluationError),
    /// Evidence or checkpoint storage refused an operation.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Run persistence refused a checkpoint.
    #[error(transparent)]
    Persistence(#[from] crate::RunPersistenceError),
    /// Trust policy refused a request.
    #[error(transparent)]
    TrustViolation(#[from] TrustViolation),
    /// Evaluation was requested without a case set.
    #[error("case set is required")]
    MissingCaseSet,
    /// Assessment evidence storage or retrieval was requested without a store.
    #[error("evidence store is required")]
    MissingEvidenceStore,
}

fn resolved_kind(request: &EvaluationRequest) -> ResolvedRequestKind {
    match request {
        EvaluationRequest::Independent { candidates, .. } => ResolvedRequestKind::Independent {
            candidates: candidates.clone(),
        },
        EvaluationRequest::Pairwise {
            left, right, order, ..
        } => ResolvedRequestKind::Pairwise {
            left: *left,
            right: *right,
            order: *order,
        },
        EvaluationRequest::Listwise { candidates, .. } => ResolvedRequestKind::Listwise {
            candidates: candidates.clone(),
        },
    }
}

fn request_granularity(request: &EvaluationRequest) -> leaven_core::AssessmentGranularity {
    match request {
        EvaluationRequest::Independent { granularity, .. }
        | EvaluationRequest::Pairwise { granularity, .. }
        | EvaluationRequest::Listwise { granularity, .. } => *granularity,
    }
}

fn request_purpose(request: &EvaluationRequest) -> leaven_core::EvaluationPurpose {
    match request {
        EvaluationRequest::Independent { purpose, .. }
        | EvaluationRequest::Pairwise { purpose, .. }
        | EvaluationRequest::Listwise { purpose, .. } => purpose.clone(),
    }
}

fn candidate_count(request: &ResolvedEvaluationRequest) -> usize {
    match &request.kind {
        ResolvedRequestKind::Independent { candidates }
        | ResolvedRequestKind::Listwise { candidates } => candidates.len(),
        ResolvedRequestKind::Pairwise { .. } => 2,
    }
}

fn evaluation_cache_key<P: OptimizationProblem>(
    evaluator: leaven_kernel::Fingerprint,
    policy: CachePolicy,
    request: &ResolvedEvaluationRequest,
    graph: RunGraphView<'_, P>,
) -> Option<EvaluationCacheKey> {
    let candidates = request_candidate_cache_identities(&policy, request, graph)?;
    Some(EvaluationCacheKey {
        evaluator,
        policy,
        case_set_version: request.set.case_set_version.clone(),
        case_ids: request.set.case_ids.clone(),
        candidates,
    })
}

fn request_candidate_cache_identities<P: OptimizationProblem>(
    policy: &CachePolicy,
    request: &ResolvedEvaluationRequest,
    graph: RunGraphView<'_, P>,
) -> Option<Vec<CacheIdentity>> {
    match policy {
        CachePolicy::Never => Some(Vec::new()),
        CachePolicy::UserKey(fingerprint) => Some(vec![CacheIdentity::User(*fingerprint)]),
        CachePolicy::Deterministic | CachePolicy::DeterministicWithSeed(_) => {
            request_candidates(request)
                .into_iter()
                .map(|candidate| graph.artifact(candidate)?.cache_identity())
                .collect()
        }
    }
}

fn request_candidates(request: &ResolvedEvaluationRequest) -> Vec<CandidateId> {
    match &request.kind {
        ResolvedRequestKind::Independent { candidates }
        | ResolvedRequestKind::Listwise { candidates } => candidates.clone(),
        ResolvedRequestKind::Pairwise { left, right, .. } => vec![*left, *right],
    }
}

fn assessment_parts<P: OptimizationProblem>(
    assessment: Assessment<P>,
) -> (
    AssessmentRecordTarget,
    P::Evidence,
    leaven_kernel::MetadataBag,
) {
    match assessment {
        Assessment::Independent {
            candidate,
            target,
            evidence,
            metadata,
            ..
        } => (
            AssessmentRecordTarget::Independent { candidate, target },
            evidence,
            metadata,
        ),
        Assessment::Pairwise {
            left,
            right,
            target,
            evidence,
            metadata,
            ..
        } => (
            AssessmentRecordTarget::Pairwise {
                left,
                right,
                target,
            },
            evidence,
            metadata,
        ),
        Assessment::Listwise {
            candidates,
            target,
            evidence,
            metadata,
            ..
        } => (
            AssessmentRecordTarget::Listwise { candidates, target },
            evidence,
            metadata,
        ),
    }
}
