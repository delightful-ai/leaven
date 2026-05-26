use std::collections::{BTreeMap, BTreeSet, VecDeque};

use leaven_core::{
    AssessmentGranularity, AssessmentTarget, EvaluationPurpose, EvaluationRequest, EvaluationSet,
    OptimizationProblem, ResolvedEvaluationRequest,
};
use leaven_kernel::{AssessmentId, CandidateId, CaseId, Cost, ErrorKind, EvaluatorId, StageId};

use crate::{
    CacheBypassReason, CacheStatus, CasewiseEvaluationReport, DynEvaluator, ErrorPolicy,
    EvaluationCacheKey, EvaluationError, EvaluationReport, RunEvent,
    graph::storage::AssessmentRecordTarget,
};

use super::support::{candidate_count, request_granularity, request_purpose, resolved_kind};
use super::{RunContext, RunContextError};

struct CasewiseCacheMiss {
    index: usize,
    case: CaseId,
    cache_key: Result<EvaluationCacheKey, CacheBypassReason>,
}

struct CasewiseCacheRows {
    rows: Vec<(usize, Vec<AssessmentId>)>,
    missing: Vec<CasewiseCacheMiss>,
    cache_hits: usize,
}

impl<P: OptimizationProblem> RunContext<'_, P> {
    /// Evaluate one candidate casewise while preserving per-case cache keys.
    ///
    /// Cache hits are returned immediately from their single-case cache entries.
    /// Cache misses are batched into one evaluator call so evaluators can use
    /// their own internal parallelism, then each returned case row is written
    /// back under the single-case cache key.
    pub async fn evaluate_independent_casewise_cached(
        &mut self,
        evaluator_id: EvaluatorId,
        candidate: CandidateId,
        set: EvaluationSet,
        purpose: EvaluationPurpose,
    ) -> Result<CasewiseEvaluationReport, RunContextError> {
        let Some(evaluator) = self
            .evaluators
            .and_then(|evaluators| evaluators.get(&evaluator_id))
            .cloned()
        else {
            let error = RunContextError::UnknownEvaluator(evaluator_id);
            self.emit(RunEvent::Error {
                stage: Some(StageId::custom("optimizer")),
                error: leaven_kernel::ErrorRecord::from_error(ErrorKind::Evaluation, &error),
                policy: ErrorPolicy::Continued,
            });
            return Err(error);
        };
        self.evaluate_independent_casewise_cached_dyn(evaluator.as_ref(), candidate, set, purpose)
            .await
    }

    async fn evaluate_independent_casewise_cached_dyn(
        &mut self,
        evaluator: &dyn DynEvaluator<P>,
        candidate: CandidateId,
        set: EvaluationSet,
        purpose: EvaluationPurpose,
    ) -> Result<CasewiseEvaluationReport, RunContextError> {
        let evaluator_id = evaluator.id();
        let full_request = EvaluationRequest::Independent {
            candidates: vec![candidate],
            set,
            granularity: AssessmentGranularity::PerCase,
            purpose: purpose.clone(),
        };
        if let Err(error) = self
            .trust
            .check_evaluation_request(&crate::Actor::Optimizer, &full_request)
        {
            self.emit(RunEvent::Error {
                stage: Some(StageId::custom("optimizer")),
                error: leaven_kernel::ErrorRecord::from_error(ErrorKind::Trust, &error),
                policy: ErrorPolicy::Continued,
            });
            return Err(RunContextError::TrustViolation(error));
        }
        let case_ids = self.resolve_evaluation_request(&full_request)?.case_ids;
        let CasewiseCacheRows {
            mut rows,
            missing,
            cache_hits,
        } = self.collect_casewise_cache_rows(
            evaluator,
            &evaluator_id,
            candidate,
            case_ids,
            &purpose,
        )?;

        let cache_misses = missing.len();
        let cost = if missing.is_empty() {
            Cost::zero()
        } else {
            self.evaluate_casewise_cache_misses(
                evaluator,
                &evaluator_id,
                candidate,
                purpose,
                missing,
                &mut rows,
            )
            .await?
        };

        rows.sort_by_key(|(index, _)| *index);
        let assessment_ids = rows
            .into_iter()
            .flat_map(|(_, assessments)| assessments)
            .collect();
        Ok(CasewiseEvaluationReport {
            assessment_ids,
            cost,
            cache_hits,
            cache_misses,
        })
    }

    fn collect_casewise_cache_rows(
        &mut self,
        evaluator: &dyn DynEvaluator<P>,
        evaluator_id: &EvaluatorId,
        candidate: CandidateId,
        case_ids: Vec<CaseId>,
        purpose: &EvaluationPurpose,
    ) -> Result<CasewiseCacheRows, RunContextError> {
        let mut rows = Vec::with_capacity(case_ids.len());
        let mut missing = Vec::new();
        let mut cache_hits = 0_usize;
        for (index, case) in case_ids.into_iter().enumerate() {
            let request = single_case_request(candidate, case, purpose.clone());
            let resolved_set = self.resolve_evaluation_request(&request)?;
            let resolved_request = ResolvedEvaluationRequest {
                kind: resolved_kind(&request),
                set: resolved_set.clone(),
                granularity: request_granularity(&request),
                purpose: request_purpose(&request),
            };
            let evaluator_fingerprint = evaluator.fingerprint();
            let policy = evaluator.cache_policy(&resolved_request);
            let cache_key =
                self.evaluation_cache_key(evaluator_fingerprint, policy.clone(), &resolved_request);
            if let Some(assessment_ids) =
                self.cached_assessment_ids(&policy, cache_key.as_ref().ok())
            {
                let request_id = self.record_evaluation_request(
                    evaluator_id,
                    evaluator_fingerprint,
                    request,
                    resolved_set,
                    candidate_count(&resolved_request),
                );
                let assessment_ids = self.materialize_casewise_cache_hit(
                    evaluator_id,
                    request_id,
                    candidate,
                    case,
                    assessment_ids,
                )?;
                let report = EvaluationReport {
                    request_id,
                    resolved_set: resolved_request.set.id,
                    assessment_ids,
                    cost: Cost::zero(),
                    cache: CacheStatus::Hit,
                };
                self.emit_evaluation_completed(evaluator_id, &report);
                cache_hits = cache_hits.saturating_add(1);
                rows.push((index, report.assessment_ids));
            } else {
                missing.push(CasewiseCacheMiss {
                    index,
                    case,
                    cache_key,
                });
            }
        }
        Ok(CasewiseCacheRows {
            rows,
            missing,
            cache_hits,
        })
    }

    async fn evaluate_casewise_cache_misses(
        &mut self,
        evaluator: &dyn DynEvaluator<P>,
        evaluator_id: &EvaluatorId,
        candidate: CandidateId,
        purpose: EvaluationPurpose,
        missing: Vec<CasewiseCacheMiss>,
        rows: &mut Vec<(usize, Vec<AssessmentId>)>,
    ) -> Result<Cost, RunContextError> {
        let missing_cases = missing.iter().map(|miss| miss.case).collect::<Vec<_>>();
        let request = EvaluationRequest::Independent {
            candidates: vec![candidate],
            set: EvaluationSet::Cases(missing_cases),
            granularity: AssessmentGranularity::PerCase,
            purpose,
        };
        let resolved_set = self.resolve_evaluation_request(&request)?;
        let resolved_request = ResolvedEvaluationRequest {
            kind: resolved_kind(&request),
            set: resolved_set.clone(),
            granularity: request_granularity(&request),
            purpose: request_purpose(&request),
        };
        let evaluator_fingerprint = evaluator.fingerprint();
        let batch_policy = evaluator.cache_policy(&resolved_request);
        let batch_cache_key =
            self.evaluation_cache_key(evaluator_fingerprint, batch_policy, &resolved_request);
        let request_id = self.record_evaluation_request(
            evaluator_id,
            evaluator_fingerprint,
            request,
            resolved_set,
            candidate_count(&resolved_request),
        );
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
        let report = self.complete_evaluation(
            evaluator_id,
            request_id,
            &resolved_request,
            batch_cache_key,
            metered,
        )?;
        let cost = report.cost.clone();
        let mut by_case = self.casewise_batch_rows_by_case(&report.assessment_ids)?;
        // Reject batches with rows for cases that were not requested *before*
        // touching the cache: those rows are pure noise from a misbehaving
        // evaluator, and caching the in-set siblings of a poisoned batch would
        // silently legitimize the evaluator's output.
        let requested_cases: BTreeSet<CaseId> = missing.iter().map(|miss| miss.case).collect();
        if by_case.keys().any(|case| !requested_cases.contains(case)) {
            return Err(RunContextError::Evaluation(EvaluationError::Message(
                "casewise batch returned rows outside requested cases".to_owned(),
            )));
        }
        for miss in missing {
            let assessment = pop_casewise_batch_assessment(&mut by_case, miss.case)?;
            if let Ok(cache_key) = miss.cache_key
                && let Some(cache) = self.cache.as_mut()
            {
                cache.insert(cache_key, vec![assessment]);
            }
            rows.push((miss.index, vec![assessment]));
        }
        // Duplicate detection: any leftover rows after popping one per `miss`
        // indicate the evaluator returned more rows than requested for a case.
        // The in-set rows already in cache are still legitimate evaluations,
        // but the evaluator misbehaved — surface it.
        if by_case.values().any(|assessments| !assessments.is_empty()) {
            return Err(RunContextError::Evaluation(EvaluationError::Message(
                "casewise batch returned duplicate rows for requested cases".to_owned(),
            )));
        }
        if self.cache.is_some() {
            self.checkpoint()?;
        }
        Ok(cost)
    }

    fn casewise_batch_rows_by_case(
        &self,
        assessments: &[AssessmentId],
    ) -> Result<BTreeMap<CaseId, VecDeque<AssessmentId>>, RunContextError> {
        let mut by_case = BTreeMap::<CaseId, VecDeque<AssessmentId>>::new();
        for assessment in assessments {
            let assessment_view = self.graph().assessment(*assessment).ok_or_else(|| {
                RunContextError::Evaluation(EvaluationError::Message(format!(
                    "casewise batch assessment `{assessment}` is missing from graph"
                )))
            })?;
            let AssessmentTarget::Case { case, .. } = assessment_view.target() else {
                return Err(RunContextError::Evaluation(EvaluationError::Message(
                    "casewise batch expected case-targeted assessments".to_owned(),
                )));
            };
            by_case.entry(*case).or_default().push_back(*assessment);
        }
        Ok(by_case)
    }
}

fn pop_casewise_batch_assessment(
    by_case: &mut BTreeMap<CaseId, VecDeque<AssessmentId>>,
    case: CaseId,
) -> Result<AssessmentId, RunContextError> {
    by_case
        .get_mut(&case)
        .and_then(VecDeque::pop_front)
        .ok_or_else(|| {
            RunContextError::Evaluation(EvaluationError::Message(format!(
                "casewise batch did not return case `{case}`"
            )))
        })
}

impl<P: OptimizationProblem> RunContext<'_, P> {
    fn materialize_casewise_cache_hit(
        &mut self,
        evaluator_id: &EvaluatorId,
        request_id: leaven_kernel::EvaluationRequestId,
        candidate: CandidateId,
        case: CaseId,
        assessment_ids: Vec<AssessmentId>,
    ) -> Result<Vec<AssessmentId>, RunContextError> {
        let mut remapped = Vec::with_capacity(assessment_ids.len());
        let mut recorded_alias_rows = false;
        for assessment in assessment_ids {
            let Some(view) = self.graph().assessment(assessment) else {
                return Err(RunContextError::Evaluation(EvaluationError::Message(
                    format!("casewise cache hit referenced missing assessment `{assessment}`"),
                )));
            };
            let AssessmentTarget::Case { case: row_case, .. } = view.target() else {
                return Err(RunContextError::Evaluation(EvaluationError::Message(
                    "casewise cache hit expected case-targeted assessment".to_owned(),
                )));
            };
            if *row_case != case {
                return Err(RunContextError::Evaluation(EvaluationError::Message(
                    "casewise cache hit returned assessment for the wrong case".to_owned(),
                )));
            }
            if view.independent_candidate() == Some(candidate) {
                remapped.push(assessment);
                continue;
            }
            let target = view.target().clone();
            let metadata = view.metadata().clone();
            let evidence = view.evidence_ref().clone();
            let id = self.graph.record_assessment(
                request_id,
                evaluator_id.clone(),
                AssessmentRecordTarget::Independent { candidate, target },
                metadata,
                evidence,
            );
            recorded_alias_rows = true;
            remapped.push(id);
        }
        if recorded_alias_rows {
            self.checkpoint()?;
        }
        Ok(remapped)
    }
}

fn single_case_request(
    candidate: CandidateId,
    case: CaseId,
    purpose: EvaluationPurpose,
) -> EvaluationRequest {
    EvaluationRequest::Independent {
        candidates: vec![candidate],
        set: EvaluationSet::Cases(vec![case]),
        granularity: AssessmentGranularity::PerCase,
        purpose,
    }
}
