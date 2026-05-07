mod support;

use futures::executor::block_on;
use leaven_core::{
    Assessment, AssessmentGranularity, AssessmentTarget, EvaluationPurpose, EvaluationRequest,
    ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_engine::{
    CachePolicy, CaseSet, EvaluationContext, EvaluationError, Evaluator, Optimizer, OptimizerError,
    RunContext, StepStatus, optimize,
};
use leaven_kernel::{Budget, CandidateId, Cost, EvaluatorId, Fingerprint, MetadataBag, Metered};
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
