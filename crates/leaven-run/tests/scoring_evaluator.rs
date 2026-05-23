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
use leaven_engine::{BudgetLedger, CachePolicy, Evaluator, RunContext, RunEvent, RunGraph};
use leaven_eval::Case;
use leaven_evidence::{
    CaseAssessmentEvidence, DataClass, DataClassSet, OutputMetadata, OutputRecord, OutputVisibility,
};
use leaven_kernel::{
    Budget, CandidateId, CaseId, ContentId, Cost, EvaluationRequestId, EvaluatorId, Fingerprint,
    ResolvedEvaluationSetId, RunId, StageId, now,
};
use leaven_run::{
    JudgeScoreContext, JudgingEvaluator, PublicEvaluationJobContext, PublicFailedCallKind,
    PublicFailedCallReceiptContext, PublicFailedCallReceiptProjectionError, RunCase, RunError,
    RunOutput, RunProblem, RuntimeFingerprint, Score, ScoreContext, ScoreError, ScoringEvaluator,
    ScoringEvaluatorIdentity,
};
use leaven_store_inline::InlineEvidenceStore;

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
                    Ok(RunOutput::new((artifact.0 + input).to_string())
                        .with_trace(format!("runner input={input}"))
                        .with_cost(Cost::llm_calls(u64::try_from(input).unwrap())))
                }
                .boxed()
            }),
            Arc::new(
                |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                    async move {
                        let rendered = ctx.output.output.clone();
                        Ok(Score::new(rendered.parse::<f64>().unwrap(), "ok")
                            .with_output(ctx.report_text_output(rendered))
                            .with_trace("scorer accepted numeric output")
                            .with_cost(Cost::llm_calls(1)))
                    }
                    .boxed()
                },
            ),
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
        assert_eq!(evidence.output(), &expected_candidate_output("42"));
        assert_eq!(evidence.feedback(), "ok");
        assert!(evidence.case_data_reads().is_empty());
        assert_eq!(
            evidence.trace(),
            &[
                "runner input=2".to_owned(),
                "scorer accepted numeric output".to_owned()
            ]
        );
    });
}

#[test]
fn scoring_evaluator_hides_target_from_runner_and_loads_target_with_case_data_receipt() {
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
        RunOutput::<String>::default()
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
                        Ok(RunOutput::new(
                            (artifact.0 + case.input().addend).to_string(),
                        ))
                    }
                    .boxed()
                })
            },
            {
                let scorer_seen_target = Arc::clone(&scorer_seen_target);
                Arc::new(
                    move |ctx: ScoreContext<TextArtifact, PromptInput, AnswerTarget, String>| {
                        let scorer_seen_target = Arc::clone(&scorer_seen_target);
                        async move {
                            assert_eq!(ctx.case.id(), CaseId::new(700));
                            assert_eq!(ctx.case.input().addend, 2);
                            assert!(ctx.case.metadata().is_empty());
                            let debug = format!("{:?}", ctx.case);
                            assert!(debug.contains("load_target"));
                            assert!(!debug.contains("42"));
                            let target = ctx.load_target().expect("target load succeeds");
                            *scorer_seen_target.lock().unwrap() = Some(target.answer);
                            let rendered = ctx.output.output.clone();
                            Ok(Score::new(
                                f64::from(u8::from(rendered == target.answer.to_string())),
                                "target checked",
                            )
                            .with_output(ctx.report_text_output(rendered)))
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
        assert_one_target_case_data_read(evidence);
    });
}

fn assert_one_target_case_data_read(evidence: &CaseAssessmentEvidence) {
    assert_eq!(evidence.case_data_reads().len(), 1);
    assert_eq!(evidence.case_data_reads()[0].operation(), "case_query.load");
    assert_eq!(evidence.case_data_reads()[0].fields(), &["target"]);
    assert_eq!(
        evidence.case_data_reads()[0].data_classes(),
        &["case.target"]
    );
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
fn run_error_preserves_source_trace_message_and_cost() {
    let error = RunError::with_source("solver failed", JudgeSourceError)
        .with_trace("cache-only replay missed")
        .with_cost(Cost::llm_calls(2));

    assert_eq!(error.message(), "solver failed");
    assert_eq!(error.trace(), &["cache-only replay missed".to_owned()]);
    assert_eq!(error.cost().llm_calls, 2);
    assert_eq!(error.to_string(), "runner failed: solver failed");
    let source = std::error::Error::source(&error).expect("source is preserved");
    assert_eq!(source.to_string(), "judge source");
}

#[test]
fn scoring_evaluator_surfaces_async_runner_failures_before_scoring() {
    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let scorer_calls = Arc::new(AtomicUsize::new(0));
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|_artifact: TextArtifact, _case| {
                async move {
                    Err(RunError::new("solver cache miss")
                        .with_trace("cache-only replay failed before provider call")
                        .with_cost(Cost::llm_calls(2)))
                }
                .boxed()
            }),
            Arc::new({
                let scorer_calls = Arc::clone(&scorer_calls);
                move |_ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                    let scorer_calls = Arc::clone(&scorer_calls);
                    async move {
                        scorer_calls.fetch_add(1, Ordering::Relaxed);
                        Ok(Score::new(0.0, "should not score"))
                    }
                    .boxed()
                }
            }),
            &identity("scoring-evaluator-runner-failure-test"),
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

        assert!(error.to_string().contains("runner function failed"));
        assert_eq!(error.cost().metric_calls, 0);
        assert_eq!(error.cost().llm_calls, 2);
        assert_eq!(scorer_calls.load(Ordering::Relaxed), 0);
    });
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
                    Ok(RunOutput::new((artifact.0 + *case.input()).to_string())
                        .with_cost(Cost::llm_calls(2)))
                }
                .boxed()
            }),
            Arc::new(
                |_ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                    async move {
                        Err(ScoreError::new("judge unavailable")
                            .with_trace("judge call reached provider")
                            .with_cost(Cost::llm_calls(3)))
                    }
                    .boxed()
                },
            ),
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
                async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
            }),
            Arc::new(
                |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                    async move {
                        assert_eq!(ctx.budget.spent.metric_calls, 3);
                        assert_eq!(ctx.budget.limit.metric_calls, Some(16));
                        let rendered = ctx.output.output.clone();
                        Ok(Score::new(rendered.parse::<f64>().unwrap(), "ok")
                            .with_output(ctx.report_text_output(rendered)))
                    }
                    .boxed()
                },
            ),
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
                        Ok(output)
                    }
                    .boxed()
                })
            },
            Arc::new(
                |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                    async move {
                        let rendered = ctx.output.output.clone();
                        Ok(Score::new(rendered.parse::<f64>().unwrap(), "ok")
                            .with_output(ctx.report_text_output(rendered)))
                    }
                    .boxed()
                },
            ),
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

#[test]
fn scoring_evaluator_preserves_typed_output_through_scoring_then_renders() {
    #[derive(Clone, Debug)]
    struct TypedPrediction {
        answer: i32,
        rationale: String,
    }

    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
                async move {
                    let prediction = TypedPrediction {
                        answer: artifact.0 + *case.input(),
                        rationale: "typed metadata".to_owned(),
                    };
                    Ok(RunOutput::typed(prediction)
                        .with_reportable_text(format!("answer={}", artifact.0 + *case.input())))
                }
                .boxed()
            }),
            Arc::new(
                |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, TypedPrediction>| {
                    async move {
                        assert_eq!(ctx.output.output.answer, 42);
                        assert_eq!(ctx.output.output.rationale, "typed metadata");
                        Ok(
                            Score::new(f64::from(ctx.output.output.answer), "scored typed output")
                                .with_output(ctx.report_text_output(format!(
                                    "answer={}",
                                    ctx.output.output.answer
                                ))),
                        )
                    }
                    .boxed()
                },
            ),
            &identity("typed-output-score-output-test"),
        );

        let metered = evaluator
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

        let Assessment::Independent { evidence, .. } = &metered.value[0] else {
            panic!("expected independent assessment");
        };
        assert!((evidence.score().score() - 42.0).abs() < f64::EPSILON);
        assert_eq!(evidence.output(), &expected_candidate_output("answer=42"));
    });
}

#[test]
fn scoring_evaluator_preserves_candidate_artifact_reportable_output() {
    #[derive(Clone, Debug)]
    struct TypedPrediction(i32);

    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
                async move {
                    let answer = artifact.0 + *case.input();
                    Ok(
                        RunOutput::typed(TypedPrediction(answer)).with_reportable_output(
                            candidate_artifact_output(format!("artifact answer={answer}")),
                        ),
                    )
                }
                .boxed()
            }),
            Arc::new(
                |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, TypedPrediction>| {
                    async move {
                        Ok(
                            Score::new(f64::from(ctx.output.output.0), "artifact output")
                                .with_output(ctx.report_text_output(format!(
                                    "artifact answer={}",
                                    ctx.output.output.0
                                ))),
                        )
                    }
                    .boxed()
                },
            ),
            &identity("typed-output-candidate-artifact-score-output-test"),
        );

        let metered = evaluator
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

        let Assessment::Independent { evidence, .. } = &metered.value[0] else {
            panic!("expected independent assessment");
        };
        assert_eq!(
            evidence.output(),
            &candidate_artifact_output("artifact answer=42")
        );
        assert!(
            evidence
                .output()
                .data_classes()
                .contains(&DataClass::candidate_artifact())
        );
    });
}

#[test]
fn scoring_evaluator_rejects_missing_reportable_output_for_typed_runner_outputs() {
    #[derive(Clone, Debug)]
    struct TypedPrediction(i32);

    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
                async move {
                    Ok(
                        RunOutput::typed(TypedPrediction(artifact.0 + *case.input()))
                            .with_cost(Cost::tokens(3, 0)),
                    )
                }
                .boxed()
            }),
            Arc::new(
                |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, TypedPrediction>| {
                    async move {
                        Ok(Score::new(f64::from(ctx.output.output.0), "ok")
                            .with_cost(Cost::metric_calls(5)))
                    }
                    .boxed()
                },
            ),
            &identity("typed-output-missing-report-output-test"),
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
            .expect("typed output without score-provided report output should fail evaluation");

        assert_eq!(error.cost().metric_calls, 6);
        assert_eq!(error.cost().prompt_tokens, 3);
        assert!(
            error
                .to_string()
                .contains("score did not provide reportable output")
        );
    });
}

#[test]
fn scoring_evaluator_rejects_empty_placeholder_report_output() {
    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
                async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
            }),
            Arc::new(
                |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                    async move {
                        Ok(Score::new(ctx.output.output.parse::<f64>().unwrap(), "ok")
                            .with_output(ctx.report_text_output("   ")))
                    }
                    .boxed()
                },
            ),
            &identity("typed-output-placeholder-output-test"),
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
            .expect("placeholder report output should fail evaluation");

        assert!(
            error
                .to_string()
                .contains("reportable output was an empty placeholder")
        );
    });
}

#[test]
fn scoring_evaluator_rejects_typed_score_output_without_runner_declaration() {
    #[derive(Clone, Debug)]
    struct TypedPrediction(i32);

    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
                async move {
                    Ok(RunOutput::typed(TypedPrediction(
                        artifact.0 + *case.input(),
                    )))
                }
                .boxed()
            }),
            Arc::new(
                |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, TypedPrediction>| {
                    async move {
                        Ok(
                            Score::new(f64::from(ctx.output.output.0), "typed").with_output(
                                ctx.report_text_output(format!("answer={}", ctx.output.output.0)),
                            ),
                        )
                    }
                    .boxed()
                },
            ),
            &identity("typed-output-missing-runner-declaration-test"),
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
            .expect("typed score output without runner declaration must fail evaluation");

        assert!(
            error
                .to_string()
                .contains("runner output did not declare reportable assessed output")
        );
    });
}

#[test]
fn scoring_evaluator_rejects_typed_runner_declaration_without_assessed_data_class() {
    #[derive(Clone, Debug)]
    struct TypedPrediction(i32);

    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
                async move {
                    let answer = artifact.0 + *case.input();
                    Ok(RunOutput::typed(TypedPrediction(answer))
                        .with_reportable_output(OutputRecord::inline(format!("answer={answer}"))))
                }
                .boxed()
            }),
            Arc::new(
                |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, TypedPrediction>| {
                    async move {
                        Ok(
                            Score::new(f64::from(ctx.output.output.0), "typed").with_output(
                                ctx.report_text_output(format!("answer={}", ctx.output.output.0)),
                            ),
                        )
                    }
                    .boxed()
                },
            ),
            &identity("typed-output-public-only-runner-declaration-test"),
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
            .expect("typed reportable output must carry candidate/artifact data class");

        assert!(
            error
                .to_string()
                .contains("runner output did not declare candidate or artifact assessed output")
        );
    });
}

#[test]
fn scoring_evaluator_rejects_same_context_dummy_report_output() {
    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
                async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
            }),
            Arc::new(
                |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                    async move {
                        Ok(
                            Score::new(ctx.output.output.parse::<f64>().unwrap(), "dummy")
                                .with_output(ctx.report_text_output("dummy but same context")),
                        )
                    }
                    .boxed()
                },
            ),
            &identity("typed-output-dummy-output-test"),
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
            .expect("dummy report output should fail evaluation");

        assert!(
            error
                .to_string()
                .contains("reportable output did not match assessed output")
        );
    });
}

#[test]
fn scoring_evaluator_rejects_typed_runner_candidate_labeled_dummy_declaration() {
    #[derive(Clone, Debug)]
    struct TypedPrediction(i32);

    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
                async move {
                    let answer = artifact.0 + *case.input();
                    Ok(
                        RunOutput::typed(TypedPrediction(answer)).with_reportable_output(
                            OutputRecord::candidate_inline(
                                "dummy output only present to satisfy schema",
                            ),
                        ),
                    )
                }
                .boxed()
            }),
            Arc::new(
                |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, TypedPrediction>| {
                    async move {
                        Ok(
                            Score::new(f64::from(ctx.output.output.0), "typed").with_output(
                                ctx.report_text_output(
                                    "dummy output only present to satisfy schema",
                                ),
                            ),
                        )
                    }
                    .boxed()
                },
            ),
            &identity("typed-output-candidate-labeled-dummy-declaration-test"),
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
            .expect("typed candidate-output declaration must be derived from the typed output");

        assert!(
            error
                .to_string()
                .contains("runner output did not derive candidate output from typed output")
        );
    });
}

#[test]
fn scoring_evaluator_rejects_mutated_context_dummy_report_output() {
    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
                async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
            }),
            Arc::new(
                |mut ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                    async move {
                        ctx.output = RunOutput::new("dummy forged through mutable context");
                        Ok(Score::new(0.0, "dummy").with_output(
                            ctx.report_text_output("dummy forged through mutable context"),
                        ))
                    }
                    .boxed()
                },
            ),
            &identity("typed-output-mutated-context-dummy-output-test"),
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
            .expect("mutating scorer context must not update the assessed output");

        assert!(
            error
                .to_string()
                .contains("reportable output did not match assessed output")
        );
    });
}

#[test]
fn scoring_evaluator_rejects_report_output_from_another_scoring_context() {
    use std::sync::Mutex;

    let stolen_output = Arc::new(Mutex::new(None));
    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![input_case(0, 2), input_case(1, 3)]),
            Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
                async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
            }),
            Arc::new({
                let stolen_output = Arc::clone(&stolen_output);
                move |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                    let stolen_output = Arc::clone(&stolen_output);
                    async move {
                        let value = ctx.output.output.parse::<f64>().unwrap();
                        if *ctx.case.input() == 2 {
                            let report_output = ctx.report_text_output(ctx.output.output.clone());
                            *stolen_output.lock().unwrap() = Some(report_output.clone());
                            Ok(Score::new(value, "first case").with_output(report_output))
                        } else {
                            let report_output = stolen_output
                                .lock()
                                .unwrap()
                                .clone()
                                .expect("first case stores a report output");
                            Ok(Score::new(value, "second case").with_output(report_output))
                        }
                    }
                    .boxed()
                }
            }),
            &identity("typed-output-context-scope-test"),
        )
        .with_parallelism(NonZeroUsize::new(1).unwrap());

        let error = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Independent {
                        candidates: vec![candidate],
                    },
                    vec![CaseId::new(0), CaseId::new(1)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .err()
            .expect("output from another scoring context must fail evaluation");

        assert!(
            error
                .to_string()
                .contains("reportable output came from another scoring context")
        );
    });
}

#[test]
fn judging_evaluator_preserves_pairwise_and_listwise_report_outputs() {
    block_on(async {
        let (mut graph, mut budget, left) = graph_with_seed();
        let (right, third) = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
            (
                ctx.insert_seed(TextArtifact(50), 1).unwrap(),
                ctx.insert_seed(TextArtifact(60), 2).unwrap(),
            )
        };
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = judging_evaluator(
            |ctx: JudgeScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                let rendered = ctx
                    .outputs
                    .iter()
                    .map(|output| output.output.output.clone())
                    .collect::<Vec<_>>()
                    .join("|");
                let score = match ctx.outputs.len() {
                    2 => 2.0,
                    3 => 3.0,
                    _ => panic!("unexpected judged output count"),
                };
                Score::new(score, "judged candidate outputs")
                    .with_output(ctx.report_text_output(rendered))
            },
        );

        let pairwise = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Pairwise {
                        left,
                        right,
                        order: PairOrder::Ordered,
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .unwrap();
        let Assessment::Pairwise {
            left: pair_left,
            right: pair_right,
            evidence,
            ..
        } = &pairwise.value[0]
        else {
            panic!("expected pairwise assessment");
        };
        assert_eq!((*pair_left, *pair_right), (left, right));
        assert_eq!(evidence.output(), &expected_candidate_output("42|52"));

        let listwise = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Listwise {
                        candidates: vec![left, right, third],
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .unwrap();
        let Assessment::Listwise {
            candidates,
            evidence,
            ..
        } = &listwise.value[0]
        else {
            panic!("expected listwise assessment");
        };
        assert_eq!(candidates, &vec![left, right, third]);
        assert_eq!(evidence.output(), &expected_candidate_output("42|52|62"));
    });
}

#[test]
fn judging_evaluator_rejects_missing_or_placeholder_group_scoped_reportable_output() {
    block_on(async {
        let (mut graph, mut budget, left) = graph_with_seed();
        let right = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact(50), 1).unwrap()
        };
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let missing_output = judging_evaluator(
            |_ctx: JudgeScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                Score::new(1.0, "missing output")
            },
        );
        let error = missing_output
            .evaluate(
                request(
                    ResolvedRequestKind::Pairwise {
                        left,
                        right,
                        order: PairOrder::Ordered,
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&missing_output))),
            )
            .await
            .err()
            .expect("missing judge output should fail");
        assert!(
            error
                .to_string()
                .contains("score did not provide reportable output")
        );

        let placeholder_output = judging_evaluator(
            |ctx: JudgeScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                Score::new(1.0, "placeholder").with_output(ctx.report_text_output(" \n\t "))
            },
        );
        let error = placeholder_output
            .evaluate(
                request(
                    ResolvedRequestKind::Listwise {
                        candidates: vec![left, right],
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&placeholder_output))),
            )
            .await
            .err()
            .expect("placeholder judge output should fail");
        assert!(
            error
                .to_string()
                .contains("reportable output was an empty placeholder")
        );
    });
}

#[test]
fn judging_evaluator_rejects_typed_runner_declaration_without_assessed_data_class() {
    #[derive(Clone, Debug)]
    struct TypedPrediction(i32);

    block_on(async {
        let (mut graph, mut budget, left) = graph_with_seed();
        let right = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact(50), 1).unwrap()
        };
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = JudgingEvaluator::new(
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
                async move {
                    let answer = artifact.0 + *case.input();
                    Ok(RunOutput::typed(TypedPrediction(answer))
                        .with_reportable_output(OutputRecord::inline(answer.to_string())))
                }
                .boxed()
            }),
            Arc::new(
                |ctx: JudgeScoreContext<
                    TextArtifact,
                    i32,
                    leaven_eval::NoTarget,
                    TypedPrediction,
                >| {
                    async move {
                        let rendered = ctx
                            .outputs
                            .iter()
                            .map(|output| output.output.output.0.to_string())
                            .collect::<Vec<_>>()
                            .join("|");
                        Ok(Score::new(1.0, "judged").with_output(ctx.report_text_output(rendered)))
                    }
                    .boxed()
                },
            ),
            &identity("judging-output-public-only-runner-declaration-test"),
        );

        let error = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Pairwise {
                        left,
                        right,
                        order: PairOrder::Ordered,
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .err()
            .expect("typed group reportable output must carry candidate/artifact data class");

        assert!(
            error
                .to_string()
                .contains("runner output did not declare candidate or artifact assessed output")
        );
    });
}

#[test]
fn judging_evaluator_rejects_same_context_dummy_report_output() {
    block_on(async {
        let (mut graph, mut budget, left) = graph_with_seed();
        let right = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact(50), 1).unwrap()
        };
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = judging_evaluator(
            |ctx: JudgeScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                assert_eq!(ctx.outputs.len(), 2);
                Score::new(1.0, "dummy")
                    .with_output(ctx.report_text_output("dummy but same candidate group"))
            },
        );

        let error = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Pairwise {
                        left,
                        right,
                        order: PairOrder::Ordered,
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .err()
            .expect("dummy judge output should fail");

        assert!(
            error
                .to_string()
                .contains("reportable output did not match assessed output")
        );
    });
}

#[test]
fn judging_evaluator_rejects_typed_runner_candidate_labeled_dummy_declaration() {
    #[derive(Clone, Debug)]
    struct TypedPrediction(i32);

    block_on(async {
        let (mut graph, mut budget, left) = graph_with_seed();
        let right = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact(50), 1).unwrap()
        };
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = JudgingEvaluator::new(
            Arc::new(vec![input_case(0, 2)]),
            Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
                async move {
                    let answer = artifact.0 + *case.input();
                    Ok(
                        RunOutput::typed(TypedPrediction(answer)).with_reportable_output(
                            OutputRecord::candidate_inline(
                                "dummy output only present to satisfy schema",
                            ),
                        ),
                    )
                }
                .boxed()
            }),
            Arc::new(
                |ctx: JudgeScoreContext<
                    TextArtifact,
                    i32,
                    leaven_eval::NoTarget,
                    TypedPrediction,
                >| {
                    async move {
                        let rendered = ctx
                            .outputs
                            .iter()
                            .map(|output| {
                                let _ = output.output.output.0;
                                "dummy output only present to satisfy schema"
                            })
                            .collect::<Vec<_>>()
                            .join("|");
                        Ok(Score::new(1.0, "judged").with_output(ctx.report_text_output(rendered)))
                    }
                    .boxed()
                },
            ),
            &identity("judging-output-candidate-labeled-dummy-declaration-test"),
        );

        let error = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Pairwise {
                        left,
                        right,
                        order: PairOrder::Ordered,
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .err()
            .expect("typed candidate-output declarations must be derived from typed outputs");

        assert!(
            error
                .to_string()
                .contains("runner output did not derive candidate output from typed output")
        );
    });
}

#[test]
fn judging_evaluator_rejects_report_output_from_another_candidate_group() {
    use std::sync::Mutex;

    let stolen_output = Arc::new(Mutex::new(None::<leaven_run::ReportableOutput>));
    block_on(async {
        let (mut graph, mut budget, left) = graph_with_seed();
        let right = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact(50), 1).unwrap()
        };
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let evaluator = JudgingEvaluator::new(
            Arc::new(vec![input_case(0, 2), input_case(1, 3)]),
            Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
                async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
            }),
            Arc::new({
                let stolen_output = Arc::clone(&stolen_output);
                move |ctx: JudgeScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                    let stolen_output = Arc::clone(&stolen_output);
                    async move {
                        if *ctx.case.input() == 2 {
                            let report_output = ctx.report_text_output("42|52");
                            *stolen_output.lock().unwrap() = Some(report_output.clone());
                            Ok(Score::new(1.0, "first pair").with_output(report_output))
                        } else {
                            let report_output = stolen_output
                                .lock()
                                .unwrap()
                                .clone()
                                .expect("first pair stores a report output");
                            Ok(Score::new(1.0, "second pair").with_output(report_output))
                        }
                    }
                    .boxed()
                }
            }),
            &identity("judging-output-scope-test"),
        )
        .with_parallelism(NonZeroUsize::new(1).unwrap());

        let error = evaluator
            .evaluate(
                request(
                    ResolvedRequestKind::Pairwise {
                        left,
                        right,
                        order: PairOrder::Ordered,
                    },
                    vec![CaseId::new(0), CaseId::new(1)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&evaluator))),
            )
            .await
            .err()
            .expect("output from another candidate-group context must fail evaluation");

        assert!(
            error
                .to_string()
                .contains("reportable output came from another scoring context")
        );
    });
}

#[test]
fn runtime_score_outputs_project_through_public_seam_for_all_assessment_shapes() {
    block_on(async {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
            .expect("public seam package loads from workspace");
        let (mut graph, mut budget, left) = graph_with_seed();
        let (right, third) = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
            (
                ctx.insert_seed(TextArtifact(50), 1).unwrap(),
                ctx.insert_seed(TextArtifact(60), 2).unwrap(),
            )
        };
        let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);

        let scorer = scoring_evaluator(|ctx| {
            Score::new(ctx.output.output.parse::<f64>().unwrap(), "independent")
                .with_output(ctx.report_text_output(ctx.output.output.clone()))
        });
        let independent = scorer
            .evaluate(
                request(
                    ResolvedRequestKind::Independent {
                        candidates: vec![left],
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&scorer))),
            )
            .await
            .unwrap();
        let Assessment::Independent { evidence, .. } = &independent.value[0] else {
            panic!("expected independent assessment");
        };
        assert_projected_public_output(&package, evidence, "42");

        let judge = judging_evaluator(
            |ctx: JudgeScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                let rendered = ctx
                    .outputs
                    .iter()
                    .map(|output| output.output.output.clone())
                    .collect::<Vec<_>>()
                    .join("|");
                Score::new(1.0, "judged").with_output(ctx.report_text_output(rendered))
            },
        );
        let pairwise = judge
            .evaluate(
                request(
                    ResolvedRequestKind::Pairwise {
                        left,
                        right,
                        order: PairOrder::Ordered,
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&judge))),
            )
            .await
            .unwrap();
        let Assessment::Pairwise { evidence, .. } = &pairwise.value[0] else {
            panic!("expected pairwise assessment");
        };
        assert_projected_public_output(&package, evidence, "42|52");

        let listwise = judge
            .evaluate(
                request(
                    ResolvedRequestKind::Listwise {
                        candidates: vec![left, right, third],
                    },
                    vec![CaseId::new(0)],
                    AssessmentGranularity::PerCase,
                ),
                ctx.evaluation_context(StageId::from_evaluator(Evaluator::id(&judge))),
            )
            .await
            .unwrap();
        let Assessment::Listwise { evidence, .. } = &listwise.value[0] else {
            panic!("expected listwise assessment");
        };
        assert_projected_public_output(&package, evidence, "42|52|62");
    });
}

#[test]
fn independent_runtime_evaluation_request_projects_to_public_seam_job() {
    block_on(async {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
            .expect("public seam package loads from workspace");
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let case_set = public_job_case_set();
        let store = public_job_store();
        let evaluator = public_job_scoring_evaluator();
        let report = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
            ctx.evaluate_with(
                &evaluator,
                leaven_core::EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Validation,
                },
            )
            .await
            .unwrap()
        };

        let ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let graph_view = ctx.graph();
        let public_job = public_job_context("sc_eval_runtime")
            .evaluation_job_document(
                &graph_view,
                &graph_view
                    .evaluation_request(report.request_id)
                    .expect("runtime evaluation request is recorded"),
            )
            .unwrap();
        let validated = package
            .validate_evaluation_job_document(&public_job)
            .unwrap();
        assert_public_evaluation_job(
            &validated,
            &public_job,
            leaven_public_seam::EvaluationJobKind::Independent,
            1,
        );
        assert_public_evaluation_request_receipt(
            &package,
            &validated,
            &public_job_context("sc_eval_runtime")
                .evaluation_request_receipt_plan_result(&public_job)
                .unwrap(),
        );
    });
}

#[test]
fn pairwise_runtime_evaluation_request_projects_to_public_seam_job() {
    block_on(async {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
            .expect("public seam package loads from workspace");
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let right = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact(50), 1).unwrap()
        };
        let case_set = public_job_case_set();
        let store = public_job_store();
        let judge = public_job_judging_evaluator();
        let report = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
            ctx.evaluate_with(
                &judge,
                leaven_core::EvaluationRequest::Pairwise {
                    left: candidate,
                    right,
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Validation,
                    order: PairOrder::Ordered,
                },
            )
            .await
            .unwrap()
        };

        let ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let graph_view = ctx.graph();
        let public_job = public_job_context("sc_eval_pairwise")
            .evaluation_job_document(
                &graph_view,
                &graph_view
                    .evaluation_request(report.request_id)
                    .expect("runtime pairwise request is recorded"),
            )
            .unwrap();
        let validated = package
            .validate_evaluation_job_document(&public_job)
            .unwrap();
        assert_public_evaluation_job(
            &validated,
            &public_job,
            leaven_public_seam::EvaluationJobKind::Pairwise,
            2,
        );
        assert_public_evaluation_request_receipt(
            &package,
            &validated,
            &public_job_context("sc_eval_pairwise")
                .evaluation_request_receipt_plan_result(&public_job)
                .unwrap(),
        );
    });
}

#[test]
fn listwise_runtime_evaluation_request_projects_to_public_seam_job() {
    block_on(async {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
            .expect("public seam package loads from workspace");
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let (right, third) = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
            (
                ctx.insert_seed(TextArtifact(50), 1).unwrap(),
                ctx.insert_seed(TextArtifact(60), 2).unwrap(),
            )
        };
        let case_set = public_job_case_set();
        let store = public_job_store();
        let judge = public_job_judging_evaluator();
        let report = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
            ctx.evaluate_with(
                &judge,
                leaven_core::EvaluationRequest::Listwise {
                    candidates: vec![candidate, right, third],
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Validation,
                },
            )
            .await
            .unwrap()
        };

        let ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let graph_view = ctx.graph();
        let public_job = public_job_context("sc_eval_listwise")
            .evaluation_job_document(
                &graph_view,
                &graph_view
                    .evaluation_request(report.request_id)
                    .expect("runtime listwise request is recorded"),
            )
            .unwrap();
        let validated = package
            .validate_evaluation_job_document(&public_job)
            .unwrap();
        assert_public_evaluation_job(
            &validated,
            &public_job,
            leaven_public_seam::EvaluationJobKind::Listwise,
            3,
        );
        assert_public_evaluation_request_receipt(
            &package,
            &validated,
            &public_job_context("sc_eval_listwise")
                .evaluation_request_receipt_plan_result(&public_job)
                .unwrap(),
        );
    });
}

#[test]
fn unsupported_runtime_evaluation_granularity_rejects_public_seam_job_projection() {
    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let case_set = public_job_case_set();
        let store = public_job_store();
        let evaluator = public_job_scoring_evaluator();
        let unsupported_request_id = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
            let err = ctx
                .evaluate_with(
                    &evaluator,
                    leaven_core::EvaluationRequest::Independent {
                        candidates: vec![candidate],
                        set: EvaluationSet::All,
                        granularity: AssessmentGranularity::Both,
                        purpose: EvaluationPurpose::Validation,
                    },
                )
                .await
                .unwrap_err();
            assert!(err.to_string().contains("per-case granularity"));
            ctx.graph()
                .events()
                .filter_map(|event| match event {
                    leaven_engine::RunEvent::EvaluationRequested { request_id, .. } => {
                        Some(*request_id)
                    }
                    _ => None,
                })
                .last()
                .expect("failed evaluation request is still recorded")
        };
        let ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget);
        let graph_view = ctx.graph();
        let unsupported_request = graph_view
            .evaluation_request(unsupported_request_id)
            .expect("unsupported request is recorded before evaluator failure");
        let error = public_job_context("sc_eval_runtime")
            .evaluation_job_document(&graph_view, &unsupported_request)
            .unwrap_err();
        assert!(error.to_string().contains("both"));
    });
}

#[test]
fn failed_runtime_lm_cost_projects_to_public_seam_call_and_charge_receipts() {
    block_on(async {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
            .expect("public seam package loads from workspace");
        let (mut graph, mut budget, candidate) = graph_with_seed();
        let case_set = public_job_case_set();
        let store = public_job_store();
        let evaluator = public_job_failing_lm_scoring_evaluator();
        let request_id = {
            let mut ctx = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
            let err = ctx
                .evaluate_with(
                    &evaluator,
                    leaven_core::EvaluationRequest::Independent {
                        candidates: vec![candidate],
                        set: EvaluationSet::All,
                        granularity: AssessmentGranularity::PerCase,
                        purpose: EvaluationPurpose::Validation,
                    },
                )
                .await
                .unwrap_err();
            assert!(err.to_string().contains("scoring function failed"));
            assert_eq!(ctx.budget().spent.llm_calls, 1);
            assert_eq!(ctx.budget().spent.prompt_tokens, 11);
            assert_eq!(ctx.budget().spent.completion_tokens, 3);
            ctx.graph()
                .events()
                .filter_map(|event| match event {
                    RunEvent::EvaluationRequested { request_id, .. } => Some(*request_id),
                    _ => None,
                })
                .last()
                .expect("failing evaluation still records the request")
        };
        let graph_view = RunContext::<RunProblem<TextArtifact, i32>>::new(&mut graph, &mut budget)
            .graph()
            .events()
            .cloned()
            .collect::<Vec<_>>();
        let charge_event = failed_paid_lm_charge_event(&graph_view);
        let failure_event = failed_paid_lm_failure_event(&graph_view);
        let receipt_context = failed_paid_lm_receipt_context();
        let request = failed_paid_lm_request(request_id);
        let result = receipt_context
            .failed_paid_call_plan_result(
                charge_event,
                failure_event,
                PublicFailedCallKind::LmComplete,
                "runtime_lm_failure",
                &request,
                "fp_runtime_sha256_scoringlm",
            )
            .unwrap();

        assert_failed_paid_lm_result(
            &package,
            &receipt_context,
            &request,
            result,
            charge_event,
            failure_event,
        );
    });
}

fn assert_failed_paid_lm_result(
    package: &leaven_public_seam::PublicSeamPackage,
    receipt_context: &PublicFailedCallReceiptContext,
    request: &serde_json::Value,
    result: serde_json::Value,
    charge_event: &RunEvent,
    failure_event: &RunEvent,
) {
    package.validate_plan_result_document(&result).unwrap();
    assert_eq!(result["receipts"][0]["status"], "failed");
    assert_eq!(result["receipts"][0]["cost"]["lm_calls"], 1);
    assert_eq!(result["receipts"][0]["cost"]["input_tokens"], 11);
    assert_eq!(result["receipts"][0]["cost"]["output_tokens"], 3);
    assert_eq!(
        result["charges"][0]["source_receipt"],
        result["receipts"][0]["receipt"]
    );

    let mut partial_charge = result;
    partial_charge["charges"][0]["cost"]["lm_calls"] = serde_json::json!(0);
    assert!(
        package
            .validate_plan_result_document(&partial_charge)
            .is_err()
    );

    let non_charge_error = receipt_context
        .failed_paid_call_plan_result(
            &RunEvent::OptimizationStarted {
                run_id: RunId::new(),
            },
            failure_event,
            PublicFailedCallKind::LmComplete,
            "runtime_lm_failure",
            request,
            "fp_runtime_sha256_scoringlm",
        )
        .unwrap_err();
    assert!(matches!(
        non_charge_error,
        PublicFailedCallReceiptProjectionError::NotBudgetChargeEvent
    ));

    let zero_charge_error = receipt_context
        .failed_paid_call_plan_result(
            &RunEvent::BudgetCharged {
                stage: failed_paid_lm_stage(failure_event),
                cost: Cost::zero(),
                remaining: BudgetLedger::default().snapshot(),
            },
            failure_event,
            PublicFailedCallKind::LmComplete,
            "runtime_lm_failure",
            request,
            "fp_runtime_sha256_scoringlm",
        )
        .unwrap_err();
    assert!(matches!(
        zero_charge_error,
        PublicFailedCallReceiptProjectionError::EmptyCost
    ));

    let missing_failure_error = receipt_context
        .failed_paid_call_plan_result(
            charge_event,
            charge_event,
            PublicFailedCallKind::LmComplete,
            "runtime_lm_failure",
            request,
            "fp_runtime_sha256_scoringlm",
        )
        .unwrap_err();
    assert!(matches!(
        missing_failure_error,
        PublicFailedCallReceiptProjectionError::NotFailureEvent
    ));
}

fn public_job_failing_lm_scoring_evaluator()
-> ScoringEvaluator<TextArtifact, i32, leaven_eval::NoTarget, String> {
    ScoringEvaluator::new(
        Arc::new(vec![input_case(0, 2), input_case(1, 3)]),
        Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
            async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
        }),
        Arc::new(
            |_ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                async move {
                    Err(ScoreError::new("provider failed after spending budget")
                        .with_cost(Cost::llm_calls(1).combine(&Cost::tokens(11, 3))))
                }
                .boxed()
            },
        ),
        &identity("public-seam-failed-paid-lm"),
    )
}

fn failed_paid_lm_charge_event(events: &[RunEvent]) -> &RunEvent {
    events
        .iter()
        .find(|event| {
            matches!(
                event,
                RunEvent::BudgetCharged {
                    cost,
                    ..
                } if cost.llm_calls == 1
                    && cost.prompt_tokens == 11
                    && cost.completion_tokens == 3
            )
        })
        .expect("failed paid evaluation emits an engine budget charge")
}

fn failed_paid_lm_failure_event(events: &[RunEvent]) -> &RunEvent {
    events
        .iter()
        .find(|event| {
            matches!(
                event,
                RunEvent::Error {
                    stage: Some(StageId::Evaluator(_)),
                    error,
                    ..
                } if error.message.contains("scoring function failed")
            )
        })
        .expect("failed paid evaluation emits an engine error")
}

fn failed_paid_lm_stage(event: &RunEvent) -> StageId {
    let RunEvent::Error {
        stage: Some(stage), ..
    } = event
    else {
        panic!("failed paid lm helper expects an engine error event");
    };
    stage.clone()
}

fn failed_paid_lm_receipt_context() -> PublicFailedCallReceiptContext {
    PublicFailedCallReceiptContext::new(
        "plan_runtime_failed_paid_lm",
        "rev_runtime_base",
        "fp_cap_sha256_runtimefailedlm",
        "fp_policy_sha256_runtimefailedlm",
    )
    .with_timing(
        "2026-05-23T12:10:00Z",
        "2026-05-23T12:10:02Z",
        "2026-05-23T12:10:02Z",
    )
}

fn failed_paid_lm_request(request_id: EvaluationRequestId) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "leaven.runtime_failed_call_request.v1",
        "call_kind": "lm_complete",
        "op_var": "runtime_lm_failure",
        "evaluation_request_id": format!("evalreq_{}", request_id.as_uuid())
    })
}

fn public_job_case_set() -> leaven_engine::CaseSet<Case<i32, leaven_eval::NoTarget>> {
    leaven_engine::CaseSet::new(vec![input_case(0, 2), input_case(1, 3)])
}

fn public_job_store() -> InlineEvidenceStore<leaven_evidence::CaseAssessmentEvidence> {
    InlineEvidenceStore::new("public-seam-job-identity")
}

fn public_job_context(stage_call_id: &str) -> PublicEvaluationJobContext {
    PublicEvaluationJobContext::new(
        stage_call_id,
        "rev_runtime_base",
        "fp_cap_sha256_runtimejob",
        "fp_policy_sha256_runtimejob",
        "2026-05-23T13:00:00Z",
    )
    .with_evaluation_request_receipt_timing("2026-05-23T12:00:00Z", "2026-05-23T12:00:01Z")
}

fn public_job_scoring_evaluator()
-> ScoringEvaluator<TextArtifact, i32, leaven_eval::NoTarget, String> {
    ScoringEvaluator::new(
        Arc::new(vec![input_case(0, 2), input_case(1, 3)]),
        Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
            async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
        }),
        Arc::new(
            |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                let score = Score::new(ctx.output.output.parse::<f64>().unwrap(), "validation")
                    .with_output(ctx.report_text_output(ctx.output.output.clone()));
                async move { Ok(score) }.boxed()
            },
        ),
        &identity("public-seam-evaluation-job"),
    )
}

fn public_job_judging_evaluator()
-> JudgingEvaluator<TextArtifact, i32, leaven_eval::NoTarget, String> {
    JudgingEvaluator::new(
        Arc::new(vec![input_case(0, 2), input_case(1, 3)]),
        Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
            async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
        }),
        Arc::new(
            |ctx: JudgeScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                let rendered = ctx
                    .outputs
                    .iter()
                    .map(|output| output.output.output.clone())
                    .collect::<Vec<_>>()
                    .join("|");
                let score = Score::new(1.0, "judged").with_output(ctx.report_text_output(rendered));
                async move { Ok(score) }.boxed()
            },
        ),
        &identity("public-seam-evaluation-job-judge"),
    )
}

fn assert_public_evaluation_job(
    validated: &leaven_public_seam::EvaluationJobDocument,
    public_job: &serde_json::Value,
    kind: leaven_public_seam::EvaluationJobKind,
    candidate_count: usize,
) {
    assert_eq!(
        validated.request_id(),
        public_job["evaluation_request_id"].as_str().unwrap()
    );
    assert_eq!(validated.kind(), kind);
    assert_eq!(validated.candidate_ids().len(), candidate_count);
    assert_eq!(
        validated.case_ids(),
        &["case_0".to_owned(), "case_1".to_owned()]
    );
    assert_eq!(validated.case_count(), 2);
    assert_eq!(validated.base_revision(), "rev_runtime_base");
    assert_eq!(
        validated.capability_fingerprint(),
        "fp_cap_sha256_runtimejob"
    );
    assert_eq!(
        public_job["resolved_set"]["partition_summary"]["resolved"],
        2
    );
}

fn assert_public_evaluation_request_receipt(
    package: &leaven_public_seam::PublicSeamPackage,
    job: &leaven_public_seam::EvaluationJobDocument,
    result: &serde_json::Value,
) {
    assert_eq!(
        result["policy_fingerprint"].as_str(),
        Some("fp_policy_sha256_runtimejob")
    );
    let receipt = package
        .validate_evaluation_request_receipt_document(job, result)
        .unwrap();
    assert_eq!(receipt.request_id(), job.request_id());
    assert_eq!(receipt.base_revision(), job.base_revision());
    assert_eq!(receipt.candidate_ids(), job.candidate_ids());
    assert_eq!(receipt.case_ids(), job.case_ids());
}

fn assert_projected_public_output(
    package: &leaven_public_seam::PublicSeamPackage,
    evidence: &leaven_evidence::CaseAssessmentEvidence,
    expected: &str,
) {
    assert_eq!(
        package
            .project_output_record(evidence.output(), None)
            .unwrap()
            .as_value(),
        &serde_json::json!({
            "kind": "text",
            "summary": expected,
            "value": expected,
            "visibility": "public",
            "data_classes": ["candidate.output", "public"]
        })
    );
    assert!(
        evidence
            .output()
            .data_classes()
            .contains(&DataClass::candidate_output())
    );
}

fn expected_candidate_output(output: impl Into<String>) -> OutputRecord {
    OutputRecord::candidate_inline(output)
}

fn candidate_artifact_output(output: impl Into<String>) -> OutputRecord {
    OutputRecord::inline(output).with_metadata(OutputMetadata::new(
        OutputVisibility::Public,
        DataClassSet::new([DataClass::candidate_artifact(), DataClass::public()]),
    ))
}

// Test helper. The scorer closure must produce a `Score` with context-scoped
// reportable output already attached (`Score::with_output`) — same
// contract as the production scorer path. Earlier revisions of this helper
// auto-filled the output, mirroring the back-compat shim that was removed
// from production; that was reintroducing the smell the hard cutover deleted.
fn scoring_evaluator(
    scorer: impl Fn(ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>) -> Score
    + Send
    + Sync
    + 'static,
) -> ScoringEvaluator<TextArtifact, i32, leaven_eval::NoTarget, String> {
    let scorer = Arc::new(scorer);
    ScoringEvaluator::new(
        Arc::new(vec![input_case(0, 2)]),
        Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
            async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
        }),
        Arc::new(move |ctx| {
            let score = scorer(ctx);
            async move { Ok(score) }.boxed()
        }),
        &identity("scoring-evaluator-test"),
    )
}

fn judging_evaluator(
    scorer: impl Fn(JudgeScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>) -> Score
    + Send
    + Sync
    + 'static,
) -> JudgingEvaluator<TextArtifact, i32, leaven_eval::NoTarget, String> {
    let scorer = Arc::new(scorer);
    JudgingEvaluator::new(
        Arc::new(vec![input_case(0, 2)]),
        Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
            async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
        }),
        Arc::new(move |ctx| {
            let score = scorer(ctx);
            async move { Ok(score) }.boxed()
        }),
        &identity("judging-evaluator-test"),
    )
}

fn input_case(index: usize, input: i32) -> Case<i32> {
    Case::input(CaseId::from_index(index), input)
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
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
            async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
        }),
        Arc::new(
            |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                async move { Ok(Score::new(ctx.output.output.parse().unwrap(), "ok")) }.boxed()
            },
        ),
        &identity("fingerprint-test"),
    );
    let changed_runner = ScoringEvaluator::new(
        Arc::new(vec![input_case(0, 2)]),
        Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
            async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
        }),
        Arc::new(
            |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                async move { Ok(Score::new(ctx.output.output.parse().unwrap(), "ok")) }.boxed()
            },
        ),
        &ScoringEvaluatorIdentity {
            runner: RuntimeFingerprint::new(Fingerprint::from_bytes([77; 32])),
            ..identity("fingerprint-test")
        },
    );
    let changed_cases = ScoringEvaluator::new(
        Arc::new(vec![input_case(0, 2), input_case(1, 3)]),
        Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
            async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
        }),
        Arc::new(
            |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                async move { Ok(Score::new(ctx.output.output.parse().unwrap(), "ok")) }.boxed()
            },
        ),
        &ScoringEvaluatorIdentity {
            dataset: Fingerprint::from_bytes([78; 32]),
            splits: Fingerprint::from_bytes([79; 32]),
            ..identity("fingerprint-test")
        },
    );
    let changed_cache_policy = ScoringEvaluator::new(
        Arc::new(vec![input_case(0, 2)]),
        Arc::new(|artifact: TextArtifact, case: RunCase<i32>| {
            async move { Ok(RunOutput::new((artifact.0 + *case.input()).to_string())) }.boxed()
        }),
        Arc::new(
            |ctx: ScoreContext<TextArtifact, i32, leaven_eval::NoTarget, String>| {
                async move { Ok(Score::new(ctx.output.output.parse().unwrap(), "ok")) }.boxed()
            },
        ),
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
