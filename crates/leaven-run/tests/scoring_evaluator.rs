use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use futures::{FutureExt, channel::oneshot, executor::block_on};
use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, CaseSetVersion,
    EvaluationPurpose, EvaluationSet, PairOrder, ResolvedEvaluationRequest, ResolvedEvaluationSet,
    ResolvedRequestKind,
};
use leaven_engine::{BudgetLedger, CachePolicy, Evaluator, RunContext, RunGraph};
use leaven_kernel::{
    Budget, CandidateId, CaseId, ContentId, Cost, EvaluatorId, ResolvedEvaluationSetId, RunId,
    StageId, now,
};
use leaven_run::{
    FeedbackAttachment, RunOutput, RunProblem, Score, ScoreContext, ScoreError, ScoringEvaluator,
};

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

#[test]
fn scoring_evaluator_reports_per_candidate_cost_for_independent_batches() {
    block_on(async {
        let (mut graph, mut budget, left) = graph_with_seed();
        let right = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact(50), 1).unwrap()
        };
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![2, 3]),
            Arc::new(|artifact: TextArtifact, case| {
                async move {
                    RunOutput::new(
                        (artifact.0 + case).to_string(),
                        vec!["runner trace".to_owned()],
                    )
                    .with_cost(Cost::llm_calls(u64::try_from(case).unwrap()))
                }
                .boxed()
            }),
            Arc::new(|ctx: ScoreContext<TextArtifact, i32>| {
                async move {
                    Ok(Score::new(ctx.output.output.parse::<f64>().unwrap(), "ok")
                        .with_structured("verdict", "accepted")
                        .with_cost(Cost::llm_calls(1))
                        .with_attachment(FeedbackAttachment::text(
                            "judge-transcript",
                            "judge says ok",
                        )))
                }
                .boxed()
            }),
            "scoring-evaluator-test",
        );
        assert!(evaluator.parallelism().get() > 0);

        let metered = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Independent {
                        candidates: vec![left, right],
                    },
                    vec![CaseId::new(0), CaseId::new(1)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .unwrap();

        assert_eq!(metered.cost.metric_calls, 4);
        assert_eq!(metered.cost.llm_calls, 14);
        let assessment_costs = metered
            .value
            .iter()
            .map(|assessment| match assessment {
                Assessment::Independent { cost, .. } => (cost.metric_calls, cost.llm_calls),
                _ => panic!("expected independent assessment"),
            })
            .collect::<Vec<_>>();
        assert_eq!(assessment_costs, vec![(2, 7), (2, 7)]);
        let [Assessment::Independent { evidence, .. }, ..] = metered.value.as_slice() else {
            panic!("expected independent assessments");
        };
        assert_eq!(
            evidence.outcomes()[0].evidence().attachments()[0].name(),
            "judge-transcript"
        );
        assert!(
            evidence.outcomes()[0]
                .evidence()
                .trace()
                .iter()
                .any(|line| line == "verdict: accepted")
        );
    });
}

#[test]
fn score_error_preserves_source_trace_message_and_cost() {
    let error = ScoreError::with_source("judge failed", JudgeSourceError)
        .with_trace("provider returned malformed rubric")
        .with_cost(Cost::llm_calls(7));

    assert_eq!(error.message(), "judge failed");
    assert_eq!(
        error.trace(),
        &["provider returned malformed rubric".to_owned()]
    );
    assert_eq!(error.cost().llm_calls, 7);
    assert_eq!(error.to_string(), "score failed: judge failed");
    let source = std::error::Error::source(&error).expect("source is preserved");
    assert_eq!(source.to_string(), "judge source");
}

#[test]
fn scoring_evaluator_surfaces_async_scorer_failures_with_metered_cost() {
    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![2]),
            Arc::new(|artifact: TextArtifact, case| {
                async move {
                    RunOutput::new(
                        (artifact.0 + case).to_string(),
                        vec!["runner trace".to_owned()],
                    )
                    .with_cost(Cost::llm_calls(2))
                }
                .boxed()
            }),
            Arc::new(|_ctx: ScoreContext<TextArtifact, i32>| {
                async move {
                    Err(ScoreError::new("judge unavailable")
                        .with_trace("judge call reached provider")
                        .with_cost(Cost::llm_calls(3)))
                }
                .boxed()
            }),
            "scoring-evaluator-failure-test",
        );

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

        assert!(error.to_string().contains("scoring function failed"));
        assert_eq!(error.cost().metric_calls, 1);
        assert_eq!(error.cost().llm_calls, 5);
    });
}

#[test]
fn scoring_evaluator_passes_budget_snapshot_to_scorer() {
    block_on(async {
        let (mut graph, _budget, candidate) = graph_with_seed();
        let mut budget = BudgetLedger::new(Budget::metric_calls(16));
        budget
            .charge(StageId::custom("setup"), Cost::metric_calls(3))
            .unwrap();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![2]),
            Arc::new(|artifact: TextArtifact, case| {
                async move {
                    RunOutput::new(
                        (artifact.0 + case).to_string(),
                        vec!["runner trace".to_owned()],
                    )
                }
                .boxed()
            }),
            Arc::new(|ctx: ScoreContext<TextArtifact, i32>| {
                async move {
                    assert_eq!(ctx.budget.spent.metric_calls, 3);
                    assert_eq!(ctx.budget.limit.metric_calls, Some(16));
                    Ok(Score::new(ctx.output.output.parse::<f64>().unwrap(), "ok"))
                }
                .boxed()
            }),
            "scoring-evaluator-budget-test",
        );

        evaluator
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
            .unwrap();
    });
}

#[test]
fn scoring_evaluator_runs_case_jobs_with_bounded_parallelism_and_stable_order() {
    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![1, 2, 3, 4]),
            {
                let active = Arc::clone(&active);
                let max_active = Arc::clone(&max_active);
                Arc::new(move |artifact: TextArtifact, case| {
                    let active = Arc::clone(&active);
                    let max_active = Arc::clone(&max_active);
                    async move {
                        let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                        max_active.fetch_max(now, Ordering::SeqCst);
                        let (tx, rx) = oneshot::channel();
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_millis(20));
                            let _ = tx.send(RunOutput::new(
                                (artifact.0 + case).to_string(),
                                vec![format!("case {case}")],
                            ));
                        });
                        let output = rx.await.expect("worker sends output");
                        active.fetch_sub(1, Ordering::SeqCst);
                        output
                    }
                    .boxed()
                })
            },
            Arc::new(|ctx: ScoreContext<TextArtifact, i32>| {
                async move { Ok(Score::new(ctx.output.output.parse::<f64>().unwrap(), "ok")) }
                    .boxed()
            }),
            "scoring-evaluator-parallel-test",
        )
        .with_parallelism(NonZeroUsize::new(2).unwrap());

        let metered = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Independent {
                        candidates: vec![candidate],
                    },
                    vec![
                        CaseId::new(0),
                        CaseId::new(1),
                        CaseId::new(2),
                        CaseId::new(3),
                    ],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .unwrap();

        assert_eq!(metered.cost.metric_calls, 4);
        assert_eq!(max_active.load(Ordering::SeqCst), 2);
        let [Assessment::Independent { evidence, .. }] = metered.value.as_slice() else {
            panic!("expected one independent assessment");
        };
        let scores = evidence
            .outcomes()
            .iter()
            .map(|outcome| outcome.evidence().score().score())
            .collect::<Vec<_>>();
        assert_eq!(scores, vec![41.0, 42.0, 43.0, 44.0]);
    });
}

fn scoring_evaluator(
    scorer: impl Fn(ScoreContext<TextArtifact, i32>) -> Score + Send + Sync + 'static,
) -> ScoringEvaluator<TextArtifact, i32> {
    let scorer = Arc::new(scorer);
    ScoringEvaluator::new(
        Arc::new(vec![2]),
        Arc::new(|artifact, case| {
            async move {
                RunOutput::new(
                    (artifact.0 + case).to_string(),
                    vec!["runner trace".to_owned()],
                )
            }
            .boxed()
        }),
        Arc::new(move |ctx| {
            let score = scorer(ctx);
            async move { Ok(score) }.boxed()
        }),
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

#[derive(Debug)]
struct JudgeSourceError;

impl std::fmt::Display for JudgeSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("judge source")
    }
}

impl std::error::Error for JudgeSourceError {}

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
        let mut bytes = [0; ContentId::BYTES];
        bytes[..std::mem::size_of::<i32>()].copy_from_slice(&self.0.to_le_bytes());
        ArtifactIdentity::Content(ContentId::from_bytes(bytes))
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
