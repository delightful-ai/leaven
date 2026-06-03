//! Evaluation finalization through `RunContext`.

use leaven_core::{
    Assessment, AssessmentGranularity, EvaluationPurpose, EvaluationRequest, EvaluationSet,
    OptimizationProblem, ResolvedEvaluationRequest,
};
use leaven_kernel::{
    AssessmentId, Cost, ErrorKind, ErrorRecord, EvaluationRequestId, EvaluatorId, StageId,
};

use super::{RunContext, RunContextError};
use crate::{
    Actor, CacheBypassReason, CachePolicy, CacheStatus, DynEvaluator, ErrorPolicy,
    EvaluationCacheKey, EvaluationError, EvaluationReport, Evaluator, RunEvent,
};

impl<P: OptimizationProblem> RunContext<'_, P> {
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

    /// Resolve an optimizer-visible evaluation set without recording an
    /// evaluation request.
    ///
    /// This is for optimizer control decisions such as GEPA train minibatch
    /// sampling. Hidden validation/test partitions still go through the same
    /// trust check as optimizer-issued evaluations.
    pub fn resolve_optimizer_evaluation_set(
        &self,
        set: &EvaluationSet,
    ) -> Result<leaven_core::ResolvedEvaluationSet, RunContextError> {
        let request = EvaluationRequest::Independent {
            candidates: Vec::new(),
            set: set.clone(),
            granularity: AssessmentGranularity::PerCase,
            purpose: EvaluationPurpose::Probe,
        };
        self.trust
            .check_evaluation_request(&Actor::Optimizer, &request)
            .map_err(RunContextError::TrustViolation)?;
        self.resolve_evaluation_request(&request)
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
            kind: super::support::resolved_kind(&request),
            set: resolved_set.clone(),
            granularity: super::support::request_granularity(&request),
            purpose: super::support::request_purpose(&request),
        };
        let evaluator_fingerprint = evaluator.fingerprint();
        let policy = evaluator.cache_policy(&resolved_request);
        let cache_key =
            self.evaluation_cache_key(evaluator_fingerprint, policy.clone(), &resolved_request);
        let request_id = self.record_evaluation_request(
            &evaluator_id,
            evaluator_fingerprint,
            request,
            resolved_set.clone(),
            super::support::candidate_count(&resolved_request),
        );
        if let Some(report) = self.cached_evaluation_report(
            &evaluator_id,
            request_id,
            &resolved_request,
            &policy,
            cache_key.as_ref().ok(),
        ) {
            return Ok(report);
        }

        let stage = StageId::from_evaluator(evaluator_id.clone());
        let eval_ctx = self.evaluation_context(stage.clone());
        let metered = match evaluator.evaluate(resolved_request.clone(), eval_ctx).await {
            Ok(metered) => metered,
            Err(error) => {
                self.charge_failed_evaluation_cost(&stage, &error)?;
                self.emit_stage_error(Some(stage.clone()), ErrorKind::Evaluation, &error);
                return Err(RunContextError::Evaluation(error));
            }
        };
        self.complete_evaluation(
            &evaluator_id,
            request_id,
            &resolved_request,
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
            kind: super::support::resolved_kind(&request),
            set: resolved_set.clone(),
            granularity: super::support::request_granularity(&request),
            purpose: super::support::request_purpose(&request),
        };
        let evaluator_fingerprint = evaluator.fingerprint();
        let policy = evaluator.cache_policy(&resolved_request);
        let cache_key =
            self.evaluation_cache_key(evaluator_fingerprint, policy.clone(), &resolved_request);
        let request_id = self.record_evaluation_request(
            &evaluator_id,
            evaluator_fingerprint,
            request,
            resolved_set.clone(),
            super::support::candidate_count(&resolved_request),
        );
        if let Some(report) = self.cached_evaluation_report(
            &evaluator_id,
            request_id,
            &resolved_request,
            &policy,
            cache_key.as_ref().ok(),
        ) {
            return Ok(report);
        }

        let stage = StageId::from_evaluator(evaluator_id.clone());
        let eval_ctx = self.evaluation_context(stage.clone());
        let metered = match evaluator
            .evaluate_boxed(resolved_request.clone(), eval_ctx)
            .await
        {
            Ok(metered) => metered,
            Err(error) => {
                self.charge_failed_evaluation_cost(&stage, &error)?;
                self.emit_stage_error(Some(stage.clone()), ErrorKind::Evaluation, &error);
                return Err(RunContextError::Evaluation(error));
            }
        };
        self.complete_evaluation(
            &evaluator_id,
            request_id,
            &resolved_request,
            cache_key,
            metered,
        )
    }

    pub(super) fn charge_failed_evaluation_cost(
        &mut self,
        stage: &StageId,
        error: &EvaluationError,
    ) -> Result<(), RunContextError> {
        let cost = error.cost();
        if cost.is_zero() {
            return Ok(());
        }
        self.charge(stage.clone(), cost)?;
        Ok(())
    }

    pub(super) fn complete_evaluation(
        &mut self,
        evaluator_id: &EvaluatorId,
        request_id: EvaluationRequestId,
        resolved_request: &ResolvedEvaluationRequest,
        cache_key: Result<EvaluationCacheKey, CacheBypassReason>,
        metered: leaven_kernel::Metered<Vec<Assessment<P>>>,
    ) -> Result<EvaluationReport, RunContextError> {
        let stage = StageId::from_evaluator(evaluator_id.clone());
        self.charge(stage, metered.cost.clone())?;
        let assessment_ids = self.record_assessments(request_id, evaluator_id, metered.value)?;
        let cache = match cache_key {
            Ok(cache_key) => {
                if let Some(cache) = self.cache.as_mut() {
                    cache.insert(cache_key, assessment_ids.clone());
                    self.checkpoint()?;
                    CacheStatus::Miss
                } else {
                    CacheStatus::Bypassed(CacheBypassReason::CacheUnavailable)
                }
            }
            Err(reason) => CacheStatus::Bypassed(reason),
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

    /// Submit typed assessment output produced outside the in-process evaluator call.
    ///
    /// External worker seams use this when the evaluation request already
    /// exists in the graph and the worker returns typed problem evidence through
    /// a trusted host-side lowering layer. Evidence is still stored through the
    /// configured evidence store and graph mutation still goes through
    /// `RunContext`.
    pub fn submit_assessments(
        &mut self,
        request_id: EvaluationRequestId,
        metered: leaven_kernel::Metered<Vec<Assessment<P>>>,
    ) -> Result<EvaluationReport, RunContextError> {
        let request = self
            .graph()
            .evaluation_request(request_id)
            .ok_or(RunContextError::UnknownEvaluationRequest(request_id))?;
        let evaluator_id = request.evaluator().clone();
        let resolved_set = request.resolved_set().id;
        let stage = StageId::from_evaluator(evaluator_id.clone());
        self.charge(stage, metered.cost.clone())?;
        let assessment_ids = self.record_assessments(request_id, &evaluator_id, metered.value)?;
        let report = EvaluationReport {
            request_id,
            resolved_set,
            assessment_ids,
            cost: metered.cost,
            cache: CacheStatus::Bypassed(CacheBypassReason::DisabledByPolicy),
        };
        self.emit_evaluation_completed(&evaluator_id, &report);
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
        let assessment_ids = self.cached_assessment_ids(policy, cache_key)?;
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

    pub(super) fn cached_assessment_ids(
        &self,
        policy: &CachePolicy,
        cache_key: Option<&EvaluationCacheKey>,
    ) -> Option<Vec<AssessmentId>> {
        if matches!(policy, CachePolicy::Never) {
            return None;
        }
        let cache_key = cache_key?;
        let assessment_ids = self
            .cache
            .as_ref()
            .and_then(|cache| cache.get(cache_key))
            .cloned()?;
        if assessment_ids
            .iter()
            .any(|assessment_id| !self.graph.assessments.contains_key(assessment_id))
        {
            return None;
        }
        Some(assessment_ids)
    }

    pub(super) fn evaluation_cache_key(
        &self,
        evaluator: leaven_kernel::Fingerprint,
        policy: CachePolicy,
        request: &ResolvedEvaluationRequest,
    ) -> Result<EvaluationCacheKey, CacheBypassReason> {
        let graph = self.graph();
        super::support::evaluation_cache_key(evaluator, policy, request, &graph)
    }

    pub(super) fn record_evaluation_request(
        &mut self,
        evaluator: &EvaluatorId,
        evaluator_fingerprint: leaven_kernel::Fingerprint,
        request: EvaluationRequest,
        resolved_set: leaven_core::ResolvedEvaluationSet,
        candidate_count: usize,
    ) -> EvaluationRequestId {
        let request_id = self.graph.record_evaluation_request(
            evaluator.clone(),
            evaluator_fingerprint,
            request,
            resolved_set,
        );
        self.emit(RunEvent::EvaluationRequested {
            request_id,
            evaluator: evaluator.clone(),
            request: crate::EvaluationRequestSummary { candidate_count },
        });
        request_id
    }

    /// Record an evaluation request produced by an external worker seam.
    ///
    /// This resolves and trust-checks the request exactly like in-process
    /// evaluation, but it does not execute an evaluator. The returned request id
    /// is the durable graph identity that a later `submit_assessments` call must
    /// reference.
    pub fn request_evaluation(
        &mut self,
        evaluator: EvaluatorId,
        evaluator_fingerprint: leaven_kernel::Fingerprint,
        request: EvaluationRequest,
    ) -> Result<EvaluationRequestId, RunContextError> {
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
        let candidate_count = super::support::candidate_count(&ResolvedEvaluationRequest {
            kind: super::support::resolved_kind(&request),
            set: resolved_set.clone(),
            granularity: super::support::request_granularity(&request),
            purpose: super::support::request_purpose(&request),
        });
        Ok(self.record_evaluation_request(
            &evaluator,
            evaluator_fingerprint,
            request,
            resolved_set,
            candidate_count,
        ))
    }

    pub(super) fn emit_evaluation_completed(
        &mut self,
        evaluator: &EvaluatorId,
        report: &EvaluationReport,
    ) {
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
            let (target, evidence, metadata) = super::support::assessment_parts(assessment);
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
}
