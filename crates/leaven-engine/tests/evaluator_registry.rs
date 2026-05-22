mod support;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use futures::executor::block_on;
use leaven_core::{
    Assessment, AssessmentGranularity, AssessmentTarget, EvaluationPurpose, EvaluationRequest,
    EvaluationSet, PairOrder, PartitionId, ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_engine::{
    CachePolicy, CaseSet, EvaluationContext, EvaluationError, Evaluator, Optimizer, OptimizerError,
    RunContext, RunContextError, RunEvent, StepStatus, TrustPolicy, optimize,
};
use leaven_kernel::{
    Budget, CandidateId, CaseId, Cost, ErrorKind, EvaluationSetId, EvaluatorId, Fingerprint,
    MetadataBag, Metered,
};
use leaven_store::EvidenceStore;
use leaven_store_inline::InlineEvidenceStore;
use support::{TestEvidence, TestProblem, TextArtifact};

#[test]
fn engine_dispatches_registered_evaluator_through_run_context() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let cases = CaseSet::new(vec!["case"]);
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .evaluator(RegisteredEvaluator)
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let mut optimizer = RegistryOptimizer {
            seed,
            best: None,
            done: false,
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
        assert_eq!(engine.view().evaluation_request_count(), 1);
        assert_eq!(engine.view().assessment_count(), 1);
        let evidence = store
            .get(
                engine
                    .view()
                    .assessment(
                        engine
                            .view()
                            .assessments(seed)
                            .iter()
                            .next()
                            .expect("seed should have an assessment")
                            .id(),
                    )
                    .expect("assessment should be visible")
                    .evidence_ref(),
            )
            .unwrap();
        assert!((evidence.score - 4.0).abs() < f64::EPSILON);
    });
}

#[test]
fn registry_evaluation_refuses_unknown_evaluator_without_mutation() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let cases = CaseSet::new(vec!["case"]);
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let mut optimizer = MissingEvaluatorOptimizer {
            seed,
            saw_unknown: false,
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
        assert!(optimizer.saw_unknown);
        assert_eq!(engine.view().evaluation_request_count(), 0);
        assert_eq!(engine.view().assessment_count(), 0);
        assert!(engine.view().events().any(|event| matches!(
            event,
            RunEvent::Error { error, .. } if error.kind == ErrorKind::Evaluation
        )));
    });
}

#[test]
fn ordered_pairwise_registry_cache_keeps_reversed_pairs_distinct() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let cases = CaseSet::new(vec!["case"]);
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .evaluator(OrderedPairwiseEvaluator)
            .build();
        let left = engine
            .insert_seed(TextArtifact("left".to_owned()), 0)
            .unwrap();
        let right = engine
            .insert_seed(TextArtifact("right".to_owned()), 1)
            .unwrap();
        let mut optimizer = ReversedOrderedPairCacheOptimizer { left, right };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(right));
        assert_eq!(engine.view().evaluation_request_count(), 2);
        assert_eq!(engine.view().assessment_count(), 2);
        assert_eq!(
            engine
                .view()
                .pairwise_assessments(left, right)
                .iter()
                .count(),
            1
        );
        assert_eq!(
            engine
                .view()
                .pairwise_assessments(right, left)
                .iter()
                .count(),
            1
        );
    });
}

#[test]
fn registered_deterministic_evaluator_reuses_cache_for_identical_request() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let cases = CaseSet::new(vec!["case"]);
        let calls = Arc::new(AtomicUsize::new(0));
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .evaluator(CountingRegisteredEvaluator {
                calls: calls.clone(),
                cache_policy: CachePolicy::Deterministic,
            })
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let mut optimizer = RepeatRegisteredEvaluation { seed };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(engine.view().evaluation_request_count(), 2);
        assert_eq!(engine.view().assessment_count(), 1);
    });
}

#[test]
fn registered_casewise_evaluation_batches_misses_and_reuses_single_case_cache() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let cases = CaseSet::new(vec!["case 0", "case 1", "case 2"]);
        let calls = Arc::new(AtomicUsize::new(0));
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .evaluator(CasewiseRegisteredEvaluator {
                calls: calls.clone(),
                behavior: CasewiseBehavior::Good,
                cache_policy: CachePolicy::Deterministic,
            })
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let mut optimizer = RepeatCasewiseEvaluation { seed };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(engine.view().evaluation_request_count(), 4);
        assert_eq!(engine.view().assessment_count(), 3);
    });
}

#[test]
fn registered_casewise_unknown_evaluator_records_error_without_mutation() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let cases = CaseSet::new(vec!["case"]);
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let mut optimizer = CasewiseErrorOptimizer {
            seed,
            set: EvaluationSet::All,
            expected: CasewiseExpectedError::Unknown,
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
        assert_eq!(engine.view().evaluation_request_count(), 0);
        assert_eq!(engine.view().assessment_count(), 0);
        assert!(engine.view().events().any(|event| matches!(
            event,
            RunEvent::Error { error, .. } if error.kind == ErrorKind::Evaluation
        )));
    });
}

#[test]
fn registered_casewise_hidden_partition_is_refused_before_request_recording() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let secret = PartitionId::from("secret");
        let cases = CaseSet::new(vec!["case"]);
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .trust_policy(TrustPolicy::default().hide_from_optimizers([secret.clone()]))
            .evaluator(CasewiseRegisteredEvaluator::good())
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let mut optimizer = CasewiseErrorOptimizer {
            seed,
            set: EvaluationSet::Partition(secret),
            expected: CasewiseExpectedError::Trust,
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
        assert_eq!(engine.view().evaluation_request_count(), 0);
        assert!(engine.view().events().any(|event| matches!(
            event,
            RunEvent::Error { error, .. } if error.kind == ErrorKind::Trust
        )));
    });
}

#[test]
fn registered_casewise_evaluator_error_records_dyn_stage_error() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let cases = CaseSet::new(vec!["case"]);
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .evaluator(CasewiseRegisteredEvaluator {
                behavior: CasewiseBehavior::Fails,
                ..CasewiseRegisteredEvaluator::good()
            })
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let mut optimizer = CasewiseErrorOptimizer {
            seed,
            set: EvaluationSet::All,
            expected: CasewiseExpectedError::Evaluation("casewise evaluation failed"),
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
        assert!(engine.view().events().any(|event| matches!(
            event,
            RunEvent::Error {
                error,
                ..
            } if error.kind == ErrorKind::Evaluation
                && error.source_chain == vec!["casewise metric backend offline"]
        )));
    });
}

#[test]
fn registered_casewise_batch_requires_case_targets() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let cases = CaseSet::new(vec!["case"]);
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .evaluator(CasewiseRegisteredEvaluator {
                behavior: CasewiseBehavior::UnscopedTarget,
                ..CasewiseRegisteredEvaluator::good()
            })
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let mut optimizer = CasewiseErrorOptimizer {
            seed,
            set: EvaluationSet::All,
            expected: CasewiseExpectedError::Evaluation(
                "casewise batch expected case-targeted assessments",
            ),
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
    });
}

#[test]
fn registered_casewise_batch_requires_every_requested_case() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let cases = CaseSet::new(vec!["case 0", "case 1"]);
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .evaluator(CasewiseRegisteredEvaluator {
                behavior: CasewiseBehavior::MissingLastCase,
                ..CasewiseRegisteredEvaluator::good()
            })
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let mut optimizer = CasewiseErrorOptimizer {
            seed,
            set: EvaluationSet::All,
            expected: CasewiseExpectedError::Evaluation("casewise batch did not return case"),
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
    });
}

#[test]
fn registered_casewise_batch_rejects_rows_outside_requested_cases() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let cases = CaseSet::new(vec!["case 0", "case 1"]);
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .evaluator(CasewiseRegisteredEvaluator {
                behavior: CasewiseBehavior::ExtraCaseRows,
                ..CasewiseRegisteredEvaluator::good()
            })
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let mut optimizer = CasewiseErrorOptimizer {
            seed,
            set: EvaluationSet::All,
            expected: CasewiseExpectedError::Evaluation(
                "casewise batch returned rows outside requested cases",
            ),
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
    });
}

#[test]
fn registered_casewise_batch_rejects_duplicate_rows_for_requested_cases() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let cases = CaseSet::new(vec!["case 0", "case 1"]);
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .evaluator(CasewiseRegisteredEvaluator {
                behavior: CasewiseBehavior::DuplicateFirstCase,
                ..CasewiseRegisteredEvaluator::good()
            })
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let mut optimizer = CasewiseErrorOptimizer {
            seed,
            set: EvaluationSet::All,
            expected: CasewiseExpectedError::Evaluation(
                "casewise batch returned duplicate rows for requested cases",
            ),
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
    });
}

#[test]
fn registered_evaluator_error_records_dyn_stage_error() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let cases = CaseSet::new(vec!["case"]);
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .evaluator(FailingRegisteredEvaluator)
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let mut optimizer = FailingRegisteredEvaluation { seed };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
        assert!(engine.view().events().any(|event| matches!(
            event,
            RunEvent::Error {
                error,
                ..
            } if error.kind == ErrorKind::Evaluation
                && error.source_chain == vec!["registered metric backend offline"]
        )));
    });
}

#[test]
fn registered_evaluation_hidden_partition_is_refused_before_request_recording() {
    block_on(async {
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let secret = PartitionId::from("secret");
        let cases = CaseSet::new(vec!["case"]);
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(10))
            .trust_policy(TrustPolicy::default().hide_from_optimizers([secret.clone()]))
            .evaluator(RegisteredEvaluator)
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let mut optimizer = HiddenPartitionRegistryEvaluation {
            seed,
            secret,
            saw_refusal: false,
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
        assert!(optimizer.saw_refusal);
        assert_eq!(engine.view().evaluation_request_count(), 0);
    });
}

struct RegistryOptimizer {
    seed: CandidateId,
    best: Option<CandidateId>,
    done: bool,
}

impl Optimizer<TestProblem> for RegistryOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        if self.done {
            return Ok(StepStatus::Done);
        }
        let report = ctx
            .evaluate(
                EvaluatorId::PRIMARY,
                EvaluationRequest::Independent {
                    candidates: vec![self.seed],
                    set: leaven_core::EvaluationSet::All,
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        assert_eq!(report.assessment_ids.len(), 1);
        self.best = Some(self.seed);
        self.done = true;
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Option<CandidateId> {
        self.best
    }
}

struct RepeatRegisteredEvaluation {
    seed: CandidateId,
}

impl Optimizer<TestProblem> for RepeatRegisteredEvaluation {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let first = ctx
            .evaluate(EvaluatorId::PRIMARY, independent_request(self.seed))
            .await
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let second = ctx
            .evaluate(EvaluatorId::PRIMARY, independent_request(self.seed))
            .await
            .map_err(|err| OptimizerError::Message(err.to_string()))?;

        assert_eq!(first.assessment_ids, second.assessment_ids);
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Option<CandidateId> {
        Some(self.seed)
    }
}

struct RepeatCasewiseEvaluation {
    seed: CandidateId,
}

impl Optimizer<TestProblem> for RepeatCasewiseEvaluation {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let first = ctx
            .evaluate_independent_casewise_cached(
                EvaluatorId::PRIMARY,
                self.seed,
                EvaluationSet::All,
                EvaluationPurpose::Search,
            )
            .await
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let second = ctx
            .evaluate_independent_casewise_cached(
                EvaluatorId::PRIMARY,
                self.seed,
                EvaluationSet::All,
                EvaluationPurpose::Search,
            )
            .await
            .map_err(|err| OptimizerError::Message(err.to_string()))?;

        assert_eq!(first.cache_hits, 0);
        assert_eq!(first.cache_misses, 3);
        assert_eq!(second.cache_hits, 3);
        assert_eq!(second.cache_misses, 0);
        assert_eq!(first.assessment_ids, second.assessment_ids);
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Option<CandidateId> {
        Some(self.seed)
    }
}

struct CasewiseErrorOptimizer {
    seed: CandidateId,
    set: EvaluationSet,
    expected: CasewiseExpectedError,
}

impl Optimizer<TestProblem> for CasewiseErrorOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let error = ctx
            .evaluate_independent_casewise_cached(
                EvaluatorId::PRIMARY,
                self.seed,
                self.set.clone(),
                EvaluationPurpose::Search,
            )
            .await
            .expect_err("casewise evaluation should surface expected error");
        self.expected.assert_matches(&error);
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Option<CandidateId> {
        Some(self.seed)
    }
}

enum CasewiseExpectedError {
    Unknown,
    Trust,
    Evaluation(&'static str),
}

impl CasewiseExpectedError {
    fn assert_matches(&self, error: &RunContextError) {
        match self {
            Self::Unknown => assert!(matches!(
                error,
                RunContextError::UnknownEvaluator(id) if *id == EvaluatorId::PRIMARY
            )),
            Self::Trust => assert!(matches!(error, RunContextError::TrustViolation(_))),
            Self::Evaluation(needle) => {
                assert!(matches!(error, RunContextError::Evaluation(_)));
                assert!(
                    error.to_string().contains(needle),
                    "expected `{error}` to contain `{needle}`"
                );
            }
        }
    }
}

struct FailingRegisteredEvaluation {
    seed: CandidateId,
}

impl Optimizer<TestProblem> for FailingRegisteredEvaluation {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let error = ctx
            .evaluate(EvaluatorId::PRIMARY, independent_request(self.seed))
            .await
            .expect_err("registered evaluator failure should reach optimizer");
        assert!(matches!(error, RunContextError::Evaluation(_)));
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Option<CandidateId> {
        Some(self.seed)
    }
}

struct HiddenPartitionRegistryEvaluation {
    seed: CandidateId,
    secret: PartitionId,
    saw_refusal: bool,
}

impl Optimizer<TestProblem> for HiddenPartitionRegistryEvaluation {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let error = ctx
            .evaluate(
                EvaluatorId::PRIMARY,
                EvaluationRequest::Independent {
                    candidates: vec![self.seed],
                    set: EvaluationSet::Partition(self.secret.clone()),
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .expect_err("hidden partition should be refused before evaluator dispatch");
        assert!(matches!(error, RunContextError::TrustViolation(_)));
        self.saw_refusal = true;
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Option<CandidateId> {
        Some(self.seed)
    }
}

struct MissingEvaluatorOptimizer {
    seed: CandidateId,
    saw_unknown: bool,
}

impl Optimizer<TestProblem> for MissingEvaluatorOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let error = ctx
            .evaluate(
                EvaluatorId::PRIMARY,
                EvaluationRequest::Independent {
                    candidates: vec![self.seed],
                    set: leaven_core::EvaluationSet::All,
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .expect_err("unregistered evaluator should be refused");
        assert!(matches!(
            error,
            RunContextError::UnknownEvaluator(id) if id == EvaluatorId::PRIMARY
        ));
        self.saw_unknown = true;
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Option<CandidateId> {
        Some(self.seed)
    }
}

struct ReversedOrderedPairCacheOptimizer {
    left: CandidateId,
    right: CandidateId,
}

impl Optimizer<TestProblem> for ReversedOrderedPairCacheOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let first = ctx
            .evaluate(
                EvaluatorId::PAIRWISE_JUDGE,
                ordered_pairwise_request(self.left, self.right),
            )
            .await
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let second = ctx
            .evaluate(
                EvaluatorId::PAIRWISE_JUDGE,
                ordered_pairwise_request(self.right, self.left),
            )
            .await
            .map_err(|err| OptimizerError::Message(err.to_string()))?;

        assert_eq!(first.assessment_ids.len(), 1);
        assert_eq!(second.assessment_ids.len(), 1);
        assert_ne!(first.assessment_ids, second.assessment_ids);
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Option<CandidateId> {
        Some(self.right)
    }
}

struct CountingRegisteredEvaluator {
    calls: Arc<AtomicUsize>,
    cache_policy: CachePolicy,
}

impl Evaluator<TestProblem> for CountingRegisteredEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([6; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        self.cache_policy.clone()
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        Ok(Metered::new(
            vec![Assessment::Independent {
                candidate: candidates[0],
                target: AssessmentTarget::Unscoped,
                evidence: TestEvidence { score: 5.0 },
                cost: Cost::metric_calls(1),
                metadata: MetadataBag::new(),
            }],
            Cost::metric_calls(1),
        ))
    }
}

#[derive(Clone, Copy)]
enum CasewiseBehavior {
    Good,
    Fails,
    UnscopedTarget,
    MissingLastCase,
    ExtraCaseRows,
    DuplicateFirstCase,
}

struct CasewiseRegisteredEvaluator {
    calls: Arc<AtomicUsize>,
    behavior: CasewiseBehavior,
    cache_policy: CachePolicy,
}

impl CasewiseRegisteredEvaluator {
    fn good() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            behavior: CasewiseBehavior::Good,
            cache_policy: CachePolicy::Deterministic,
        }
    }
}

impl Evaluator<TestProblem> for CasewiseRegisteredEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([8; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        self.cache_policy.clone()
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if matches!(self.behavior, CasewiseBehavior::Fails) {
            return Err(EvaluationError::with_source(
                "casewise evaluation failed",
                StaticTestError("casewise metric backend offline"),
            ));
        }
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        let mut cases = request.set.case_ids;
        if matches!(self.behavior, CasewiseBehavior::MissingLastCase) {
            cases.pop();
        }
        if matches!(self.behavior, CasewiseBehavior::ExtraCaseRows) {
            cases.push(CaseId::new(999));
        }
        if matches!(self.behavior, CasewiseBehavior::DuplicateFirstCase)
            && let Some(first) = cases.first().copied()
        {
            cases.push(first);
        }
        let set = EvaluationSetId::from_uuid(request.set.id.as_uuid());
        let assessments = cases
            .into_iter()
            .map(|case| Assessment::Independent {
                candidate: candidates[0],
                target: self.casewise_target(set, case),
                evidence: TestEvidence { score: 6.0 },
                cost: Cost::metric_calls(1),
                metadata: MetadataBag::new(),
            })
            .collect();
        Ok(Metered::new(assessments, Cost::metric_calls(1)))
    }
}

impl CasewiseRegisteredEvaluator {
    fn casewise_target(&self, set: EvaluationSetId, case: CaseId) -> AssessmentTarget {
        if matches!(self.behavior, CasewiseBehavior::UnscopedTarget) {
            AssessmentTarget::Unscoped
        } else {
            AssessmentTarget::Case { set, case }
        }
    }
}

struct FailingRegisteredEvaluator;

impl Evaluator<TestProblem> for FailingRegisteredEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([7; 32])
    }

    async fn evaluate(
        &self,
        _request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        Err(EvaluationError::with_source(
            "registered evaluation failed",
            StaticTestError("registered metric backend offline"),
        ))
    }
}

struct RegisteredEvaluator;

impl Evaluator<TestProblem> for RegisteredEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([4; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        Ok(Metered::new(
            vec![Assessment::Independent {
                candidate: candidates[0],
                target: AssessmentTarget::Unscoped,
                evidence: TestEvidence { score: 4.0 },
                cost: Cost::metric_calls(1),
                metadata: MetadataBag::new(),
            }],
            Cost::metric_calls(1),
        ))
    }
}

struct OrderedPairwiseEvaluator;

impl Evaluator<TestProblem> for OrderedPairwiseEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PAIRWISE_JUDGE
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([5; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Deterministic
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        let ResolvedRequestKind::Pairwise { left, right, order } = request.kind else {
            return Err(EvaluationError::Message(
                "expected pairwise request".to_owned(),
            ));
        };
        assert_eq!(order, PairOrder::Ordered);
        Ok(Metered::new(
            vec![Assessment::Pairwise {
                left,
                right,
                target: AssessmentTarget::Unscoped,
                evidence: TestEvidence { score: 1.0 },
                cost: Cost::metric_calls(1),
                metadata: MetadataBag::new(),
            }],
            Cost::metric_calls(1),
        ))
    }
}

fn ordered_pairwise_request(left: CandidateId, right: CandidateId) -> EvaluationRequest {
    EvaluationRequest::Pairwise {
        left,
        right,
        set: leaven_core::EvaluationSet::All,
        granularity: AssessmentGranularity::Aggregate,
        purpose: EvaluationPurpose::Selection,
        order: PairOrder::Ordered,
    }
}

fn independent_request(candidate: CandidateId) -> EvaluationRequest {
    EvaluationRequest::Independent {
        candidates: vec![candidate],
        set: EvaluationSet::All,
        granularity: AssessmentGranularity::Aggregate,
        purpose: EvaluationPurpose::Search,
    }
}

#[derive(Debug)]
struct StaticTestError(&'static str);

impl std::fmt::Display for StaticTestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for StaticTestError {}
