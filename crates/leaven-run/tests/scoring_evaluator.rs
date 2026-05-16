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
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    CaseSetVersion, EvaluationPurpose, EvaluationSet, PairOrder, ResolvedEvaluationRequest,
    ResolvedEvaluationSet, ResolvedRequestKind,
};
use leaven_engine::{BudgetLedger, CachePolicy, Evaluator, RunContext, RunGraph};
use leaven_eval::Case;
use leaven_kernel::{
    Budget, CandidateId, CaseId, ContentId, Cost, EvaluatorId, Fingerprint,
    ResolvedEvaluationSetId, RunId, StageId, now,
};
use leaven_run::{
    RunCase, RunOutput, RunProblem, RuntimeFingerprint, Score, ScoreContext, ScoreError,
    ScoringEvaluator, ScoringEvaluatorIdentity,
};

const TEST_RUNNER_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([7; 32]);
const TEST_SCORER_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([8; 32]);
const TEST_DATASET_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([9; 32]);
const TEST_SPLIT_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([10; 32]);

fn identity(label: &str) -> ScoringEvaluatorIdentity {
    ScoringEvaluatorIdentity {
        label: label.to_owned(),
        runner: RuntimeFingerprint::new(TEST_RUNNER_FINGERPRINT),
        scorer: RuntimeFingerprint::new(TEST_SCORER_FINGERPRINT),
        dataset: TEST_DATASET_FINGERPRINT,
        splits: TEST_SPLIT_FINGERPRINT,
        cache_policy: CachePolicy::Never,
    }
}

#[test]
fn scoring_evaluator_rejects_unsupported_request_shapes_and_bad_inputs() {
    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = scoring_evaluator(|ctx| {
            Score::new(
                ctx.output.output.parse::<f64>().unwrap(),
                format!("case {}", ctx.case.input()),
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
            Arc::new(vec![input_case(0, 2), input_case(1, 3)]),
            Arc::new(|artifact: TextArtifact, case| {
                async move {
                    let input = *case.input();
                    RunOutput::new((artifact.0 + input).to_string())
                        .with_cost(Cost::llm_calls(u64::try_from(input).unwrap()))
                }
                .boxed()
            }),
            Arc::new(|ctx: ScoreContext<TextArtifact, i32>| {
                async move {
                    Ok(Score::new(ctx.output.output.parse::<f64>().unwrap(), "ok")
                        .with_cost(Cost::llm_calls(1)))
                }
                .boxed()
            }),
            &identity("scoring-evaluator-test"),
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
        assert_eq!(assessment_costs, vec![(1, 3), (1, 4), (1, 3), (1, 4)]);
        let [
            Assessment::Independent {
                target, evidence, ..
            },
            ..,
        ] = metered.value.as_slice()
        else {
            panic!("expected independent assessments");
        };
        assert!(matches!(
            target,
            AssessmentTarget::Case {
                case,
                ..
            } if *case == CaseId::from_index(0)
        ));
        assert_eq!(
            evidence.output(),
            &leaven_evidence::OutputRecord::inline("42")
        );
        assert_eq!(evidence.feedback(), "ok");
    });
}

#[test]
fn scoring_evaluator_hides_target_from_runner_and_passes_target_to_scorer() {
    #[derive(Clone, Debug, Eq, PartialEq)]
    struct PromptInput {
        addend: i32,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct AnswerTarget {
        answer: i32,
    }

    fn ordinary_runner_signature_has_no_target_type_parameter<F, Fut>(_runner: F)
    where
        F: Fn(TextArtifact, RunCase<PromptInput>) -> Fut,
    {
    }

    ordinary_runner_signature_has_no_target_type_parameter(|_artifact, case| async move {
        let _runner_visible = (case.id(), case.input().addend);
        RunOutput::default()
    });

    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed_for::<PromptInput, AnswerTarget>();
        let mut ctx = RunContext::<RunProblem<TextArtifact, PromptInput, AnswerTarget>>::new(
            &mut graph,
            &mut budget,
        );
        let runner_seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let scorer_seen_target = Arc::new(std::sync::Mutex::new(None));
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![Case::targeted(
                CaseId::new(700),
                PromptInput { addend: 2 },
                AnswerTarget { answer: 42 },
            )]),
            {
                let runner_seen = Arc::clone(&runner_seen);
                Arc::new(move |artifact: TextArtifact, case: RunCase<PromptInput>| {
                    let runner_seen = Arc::clone(&runner_seen);
                    async move {
                        runner_seen
                            .lock()
                            .unwrap()
                            .push((case.id(), case.input().addend));
                        RunOutput::new((artifact.0 + case.input().addend).to_string())
                    }
                    .boxed()
                })
            },
            {
                let scorer_seen_target = Arc::clone(&scorer_seen_target);
                Arc::new(
                    move |ctx: ScoreContext<TextArtifact, PromptInput, AnswerTarget>| {
                        let scorer_seen_target = Arc::clone(&scorer_seen_target);
                        async move {
                            assert_eq!(ctx.case.id(), CaseId::new(700));
                            assert_eq!(ctx.case.input().addend, 2);
                            assert!(ctx.case.metadata().is_empty());
                            let target = ctx.case.target().expect("target is scorer-visible");
                            *scorer_seen_target.lock().unwrap() = Some(target.answer);
                            Ok(Score::new(
                                f64::from(u8::from(ctx.output.output == target.answer.to_string())),
                                "target checked",
                            ))
                        }
                        .boxed()
                    },
                )
            },
            &identity("scoring-evaluator-target-visibility-test"),
        );

        let metered = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Independent {
                        candidates: vec![candidate],
                    },
                    vec![CaseId::new(700)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .unwrap();

        assert_eq!(
            runner_seen.lock().unwrap().as_slice(),
            &[(CaseId::new(700), 2)]
        );
        assert_eq!(*scorer_seen_target.lock().unwrap(), Some(42));
        let [Assessment::Independent { evidence, .. }] = metered.value.as_slice() else {
            panic!("expected one independent assessment");
        };
        assert_eq!(evidence.feedback(), "target checked");
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
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|artifact: TextArtifact, case| {
                async move {
                    RunOutput::new((artifact.0 + *case.input()).to_string())
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
            &identity("scoring-evaluator-failure-test"),
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
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|artifact: TextArtifact, case| {
                async move { RunOutput::new((artifact.0 + *case.input()).to_string()) }.boxed()
            }),
            Arc::new(|ctx: ScoreContext<TextArtifact, i32>| {
                async move {
                    assert_eq!(ctx.budget.spent.metric_calls, 3);
                    assert_eq!(ctx.budget.limit.metric_calls, Some(16));
                    Ok(Score::new(ctx.output.output.parse::<f64>().unwrap(), "ok"))
                }
                .boxed()
            }),
            &identity("scoring-evaluator-budget-test"),
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
            Arc::new(vec![
                input_case(0, 1),
                input_case(1, 2),
                input_case(2, 3),
                input_case(3, 4),
            ]),
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
                            let _ =
                                tx.send(RunOutput::new((artifact.0 + *case.input()).to_string()));
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
            &identity("scoring-evaluator-parallel-test"),
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
        let scores = metered
            .value
            .iter()
            .map(|assessment| match assessment {
                Assessment::Independent {
                    target, evidence, ..
                } => {
                    assert!(matches!(target, AssessmentTarget::Case { .. }));
                    evidence.score().score()
                }
                _ => panic!("expected independent assessment"),
            })
            .collect::<Vec<_>>();
        assert_eq!(scores, vec![41.0, 42.0, 43.0, 44.0]);
    });
}

fn scoring_evaluator(
    scorer: impl Fn(ScoreContext<TextArtifact, i32>) -> Score + Send + Sync + 'static,
) -> ScoringEvaluator<TextArtifact, i32> {
    let scorer = Arc::new(scorer);
    ScoringEvaluator::new(
        Arc::new(vec![input_case(0, 2)]),
        Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
            async move { RunOutput::new((artifact.0 + *case.input()).to_string()) }.boxed()
        }),
        Arc::new(move |ctx| {
            let score = scorer(ctx);
            async move { Ok(score) }.boxed()
        }),
        &identity("scoring-evaluator-test"),
    )
}

fn input_case(index: usize, input: i32) -> Case<i32> {
    Case::input(CaseId::from_index(index), input)
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
    graph_with_seed_for::<i32, leaven_eval::NoTarget>()
}

fn graph_with_seed_for<I, T>() -> (
    RunGraph<RunProblem<TextArtifact, I, T>>,
    BudgetLedger,
    CandidateId,
)
where
    I: Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    let mut graph = RunGraph::<RunProblem<TextArtifact, I, T>>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let candidate = {
        let mut ctx = RunContext::<RunProblem<TextArtifact, I, T>>::new(&mut graph, &mut budget);
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

    let cached = scoring_evaluator(|ctx| Score::new(ctx.output.output.parse().unwrap(), "ok"))
        .with_cache_policy(CachePolicy::Deterministic);
    assert_eq!(cached.cache_policy(&request), CachePolicy::Deterministic);
}

#[test]
fn scoring_evaluator_fingerprint_includes_runtime_and_case_identity() {
    let base = ScoringEvaluator::new(
        Arc::new(vec![input_case(0, 2)]),
        Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
            async move { RunOutput::new((artifact.0 + *case.input()).to_string()) }.boxed()
        }),
        Arc::new(|ctx: ScoreContext<TextArtifact, i32>| {
            async move { Ok(Score::new(ctx.output.output.parse().unwrap(), "ok")) }.boxed()
        }),
        &identity("fingerprint-test"),
    );
    let changed_runner = ScoringEvaluator::new(
        Arc::new(vec![input_case(0, 2)]),
        Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
            async move { RunOutput::new((artifact.0 + *case.input()).to_string()) }.boxed()
        }),
        Arc::new(|ctx: ScoreContext<TextArtifact, i32>| {
            async move { Ok(Score::new(ctx.output.output.parse().unwrap(), "ok")) }.boxed()
        }),
        &ScoringEvaluatorIdentity {
            runner: RuntimeFingerprint::new(Fingerprint::from_bytes([77; 32])),
            ..identity("fingerprint-test")
        },
    );
    let changed_cases = ScoringEvaluator::new(
        Arc::new(vec![input_case(0, 2), input_case(1, 3)]),
        Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
            async move { RunOutput::new((artifact.0 + *case.input()).to_string()) }.boxed()
        }),
        Arc::new(|ctx: ScoreContext<TextArtifact, i32>| {
            async move { Ok(Score::new(ctx.output.output.parse().unwrap(), "ok")) }.boxed()
        }),
        &ScoringEvaluatorIdentity {
            dataset: Fingerprint::from_bytes([78; 32]),
            splits: Fingerprint::from_bytes([79; 32]),
            ..identity("fingerprint-test")
        },
    );
    let changed_cache_policy = ScoringEvaluator::new(
        Arc::new(vec![input_case(0, 2)]),
        Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
            async move { RunOutput::new((artifact.0 + *case.input()).to_string()) }.boxed()
        }),
        Arc::new(|ctx: ScoreContext<TextArtifact, i32>| {
            async move { Ok(Score::new(ctx.output.output.parse().unwrap(), "ok")) }.boxed()
        }),
        &ScoringEvaluatorIdentity {
            cache_policy: CachePolicy::Deterministic,
            ..identity("fingerprint-test")
        },
    );

    assert_ne!(
        Evaluator::<RunProblem<TextArtifact, i32>>::fingerprint(&base),
        Evaluator::<RunProblem<TextArtifact, i32>>::fingerprint(&changed_runner)
    );
    assert_ne!(
        Evaluator::<RunProblem<TextArtifact, i32>>::fingerprint(&base),
        Evaluator::<RunProblem<TextArtifact, i32>>::fingerprint(&changed_cases)
    );
    assert_ne!(
        Evaluator::<RunProblem<TextArtifact, i32>>::fingerprint(&base),
        Evaluator::<RunProblem<TextArtifact, i32>>::fingerprint(&changed_cache_policy)
    );
}
