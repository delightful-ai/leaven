use std::sync::Arc;

use futures::executor::block_on;
use leaven_core::{
    Artifact, ArtifactIdentity, AssessmentGranularity, CaseSetVersion, EvaluationPurpose,
    EvaluationSet, PairOrder, ResolvedEvaluationRequest, ResolvedEvaluationSet,
    ResolvedRequestKind,
};
use leaven_engine::{BudgetLedger, CachePolicy, Evaluator, RunContext, RunGraph};
use leaven_kernel::{
    CandidateId, CaseId, ContentId, EvaluatorId, ResolvedEvaluationSetId, RunId, StageId, now,
};
use leaven_run::{RunOutput, RunProblem, Score, ScoreContext, ScoringEvaluator};

#[test]
fn scoring_evaluator_rejects_unsupported_request_shapes_and_bad_inputs() {
    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = scoring_evaluator(|ctx| {
            Score::new(
                ctx.output.output.parse::<f64>().unwrap(),
                format!("case {}", ctx.case),
            )
        });

        let aggregate = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Independent {
                        candidates: vec![candidate],
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::Aggregate,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .err()
            .expect("evaluation should fail");
        assert!(aggregate.to_string().contains("per-case granularity"));

        let pairwise = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Pairwise {
                        left: candidate,
                        right: CandidateId::new(),
                        order: PairOrder::Ordered,
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .err()
            .expect("evaluation should fail");
        assert!(pairwise.to_string().contains("independent requests"));

        let missing_candidate = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Independent {
                        candidates: vec![CandidateId::new()],
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .err()
            .expect("evaluation should fail");
        assert!(missing_candidate.to_string().contains("is missing"));

        let missing_case = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Independent {
                        candidates: vec![candidate],
                    },
                    vec![CaseId::new(99)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .err()
            .expect("evaluation should fail");
        assert!(missing_case.to_string().contains("evaluator cases"));
    });
}

#[test]
fn scoring_evaluator_rejects_non_finite_scores() {
    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = scoring_evaluator(|_| Score::new(f64::NAN, "not finite"));

        let error = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Independent {
                        candidates: vec![candidate],
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .err()
            .expect("evaluation should fail");

        assert!(error.to_string().contains("score was not finite"));
    });
}

fn scoring_evaluator(
    scorer: impl for<'a> Fn(ScoreContext<'a, TextArtifact, i32>) -> Score + Send + Sync + 'static,
) -> ScoringEvaluator<TextArtifact, i32> {
    ScoringEvaluator::new(
        Arc::new(vec![2]),
        Arc::new(|artifact, case| {
            RunOutput::new(
                (artifact.0 + case).to_string(),
                vec!["runner trace".to_owned()],
            )
        }),
        Arc::new(scorer),
        "scoring-evaluator-test",
    )
}

fn request(
    kind: ResolvedRequestKind,
    case_ids: Vec<CaseId>,
    granularity: AssessmentGranularity,
) -> ResolvedEvaluationRequest {
    ResolvedEvaluationRequest {
        kind,
        set: ResolvedEvaluationSet {
            id: ResolvedEvaluationSetId::new(),
            expr: EvaluationSet::All,
            case_ids,
            resolved_at: now(),
            case_set_version: CaseSetVersion("test".to_owned()),
        },
        granularity,
        purpose: EvaluationPurpose::Search,
    }
}

fn graph_with_seed() -> (
    RunGraph<RunProblem<TextArtifact, i32>>,
    BudgetLedger,
    CandidateId,
) {
    let mut graph = RunGraph::<RunProblem<TextArtifact, i32>>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let candidate = {
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        ctx.insert_seed(TextArtifact(40), 0).unwrap()
    };
    (graph, budget, candidate)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextArtifact(i32);

#[derive(Debug)]
struct TextArtifactError;

impl std::fmt::Display for TextArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("text artifact error")
    }
}

impl std::error::Error for TextArtifactError {}

impl Artifact for TextArtifact {
    type Change = i32;
    type ApplyError = TextArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::from_bytes([self.0 as u8; ContentId::BYTES]))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(*change))
    }
}

#[test]
fn scoring_evaluator_identity_and_cache_policy_are_stable() {
    let evaluator = scoring_evaluator(|ctx| Score::new(ctx.output.output.parse().unwrap(), "ok"));
    let request = request(
        ResolvedRequestKind::Independent {
            candidates: vec![CandidateId::new()],
        },
        vec![CaseId::new(0)],
        AssessmentGranularity::PerCase,
    );

    assert_eq!(
        Evaluator::<RunProblem<TextArtifact, i32>>::id(&evaluator),
        EvaluatorId::PRIMARY
    );
    assert_eq!(evaluator.cache_policy(&request), CachePolicy::Never);
    assert_eq!(evaluator.fingerprint(), evaluator.fingerprint());
}
