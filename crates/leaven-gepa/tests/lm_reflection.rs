use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentTarget, CacheIdentity, ExternalRef, InfoRef,
    OptimizationProblem, ProposalBatch, ProposalBatchSemantics, ResolvedEvaluationRequest,
    ResolvedRequestKind,
};
use leaven_engine::{
    BudgetLedger, CachePolicy, CaseSet, Engine, EvaluationContext, EvaluationError, Evaluator,
    RunContext, RunGraph,
};
use leaven_evidence::{CaseAssessmentEvidence, OutputRecord, ScalarEvidence};
use leaven_gepa::{
    DEFAULT_REFLECTION_PROMPT_TEMPLATE, DefaultReflectionRenderer, FullValidation, Gepa,
    GepaReflectiveDataset, GepaReflector, LmBackedReflector, LmBackedReflectorConfig,
    MinibatchThenValidation, PlainTextEditParser, ReflectRequest, ReflectionOutputParser,
    ReflectionRenderInput, ReflectionRenderer, ReflectiveCase, ReflectiveValue,
};
use leaven_kernel::{
    AssessmentId, Budget, CandidateId, CaseId, ContentId, Cost, EvaluatorId, Fingerprint,
    MetadataBag, Metered, ProposerId, StageId,
};
use leaven_lm::{Lm, LmError, LmId, LmRequest, LmResponse, Message, Role, TokenUsage};
use leaven_population::ParetoFrontier;
use leaven_store_inline::InlineEvidenceStore;
use leaven_surface::{EditSurface, Part, PartAddress, SurfaceError, SurfaceFingerprint};

fn reflective_case(
    case_id: Option<CaseId>,
    input: &str,
    output: Option<&str>,
    score: Option<f64>,
    feedback: &str,
) -> ReflectiveCase {
    let mut case = ReflectiveCase::from_example(
        ReflectiveValue::Text(input.to_owned()),
        None,
        output.map(|value| ReflectiveValue::Text(value.to_owned())),
        score,
        feedback.to_owned(),
    );
    case.case_id = case_id;
    case
}

#[test]
fn lm_backed_reflector_renders_feedback_records_and_applies_candidate() {
    block_on(async {
        let case_set = CaseSet::new(vec!["the case input", "validation input"])
            .with_partition(
                leaven_core::PartitionId::from("TRAIN"),
                vec![leaven_kernel::CaseId::new(0)],
            )
            .with_partition(
                leaven_core::PartitionId::from("VALIDATION"),
                vec![leaven_kernel::CaseId::new(1)],
            );
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut engine = Engine::<TestProblem>::builder()
            .budget(Budget::unlimited())
            .evaluator(FeedbackEvaluator)
            .build();
        let seed = engine
            .insert_seed(TestArtifact("seed".to_owned()), 0)
            .unwrap();
        let lm = RecordingLm::new("-lm", 11, 7);
        let requests = lm.requests();
        let reflector = LmBackedReflector::new(
            lm,
            "mock-reflector",
            DefaultReflectionRenderer,
            PlainTextEditParser,
        )
        .with_config(LmBackedReflectorConfig::default())
        .with_id("gepa/test-lm-backed-reflector");
        let mut gepa = Gepa::new(
            WholeTextSurface,
            ParetoFrontier::by_case().build(),
            reflector,
        )
        .validation_policy(MinibatchThenValidation)
        .max_iterations(1);

        engine.run(&mut gepa, &case_set, &store).await.unwrap();

        let view = engine.view();
        let candidate = view
            .candidate_tree()
            .children(seed)
            .first()
            .copied()
            .expect("LM-backed reflection applied a child candidate");
        assert_eq!(view.artifact(candidate).unwrap().0, "seed-lm");

        let captured = requests.lock().expect("requests lock").clone();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].model.as_str(), "mock-reflector");
        let rendered = captured[0]
            .messages
            .iter()
            .map(Message::content)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("I provided an assistant with the following instructions"));
        assert!(rendered.contains("```"));
        assert!(rendered.contains("seed"));
        assert!(rendered.contains("candidate missed the target suffix"));
        assert!(rendered.contains("candidate output"));

        let proposal = view
            .proposal_that_created(candidate)
            .expect("applied candidate has proposal");
        assert!(
            proposal
                .provenance()
                .informed_by_refs()
                .contains(&leaven_core::InfoRef::Candidate(seed))
        );
        assert!(
            proposal
                .provenance()
                .informed_by_refs()
                .iter()
                .any(|source| matches!(source, leaven_core::InfoRef::Assessment(_)))
        );

        let budget = engine.budget().snapshot();
        assert_eq!(budget.spent.llm_calls, 1);
        assert_eq!(budget.spent.prompt_tokens, 11);
        assert_eq!(budget.spent.completion_tokens, 7);
        let proposer_stage =
            StageId::from_proposer(ProposerId::from("gepa/test-lm-backed-reflector"));
        let proposer_cost = budget
            .stages
            .get(&proposer_stage)
            .expect("LM-backed reflector charged as proposer stage");
        assert_eq!(proposer_cost.llm_calls, 1);
    });
}

#[test]
fn multi_iteration_reflection_uses_selected_parent_assessment_feedback() {
    block_on(async {
        let case_set = CaseSet::new(vec!["the case input", "validation input"])
            .with_partition(
                leaven_core::PartitionId::from("TRAIN"),
                vec![leaven_kernel::CaseId::new(0)],
            )
            .with_partition(
                leaven_core::PartitionId::from("VALIDATION"),
                vec![leaven_kernel::CaseId::new(1)],
            );
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut engine = Engine::<TestProblem>::builder()
            .budget(Budget::unlimited())
            .evaluator(ArtifactFeedbackEvaluator)
            .build();
        engine
            .insert_seed(TestArtifact("seed".to_owned()), 0)
            .unwrap();
        let lm = RecordingLm::new("-lm", 1, 1);
        let requests = lm.requests();
        let reflector = LmBackedReflector::new(
            lm,
            "mock-reflector",
            DefaultReflectionRenderer,
            PlainTextEditParser,
        );
        let mut gepa = Gepa::new(
            WholeTextSurface,
            ParetoFrontier::by_case().build(),
            reflector,
        )
        .skip_perfect_score(false)
        .validation_policy(FullValidation)
        .max_iterations(2);

        engine.run(&mut gepa, &case_set, &store).await.unwrap();

        let captured = requests.lock().expect("requests lock").clone();
        assert_eq!(captured.len(), 2);
        let second_rendered = captured[1]
            .messages
            .iter()
            .map(Message::content)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(second_rendered.contains("seed-lm"));
        assert!(second_rendered.contains("## Feedback\nfeedback for seed-lm\n"));
        assert!(!second_rendered.contains("## Feedback\nfeedback for seed\n"));
    });
}

#[test]
fn default_reflective_dataset_projects_parse_failures_without_hidden_case_targets() {
    block_on(async {
        let case_set = CaseSet::new(vec![SecretCase {
            input: "what is 19 + 23?",
            hidden_target: "SECRET_TARGET_DO_NOT_RENDER",
        }])
        .with_partition(
            leaven_core::PartitionId::from("TRAIN"),
            vec![leaven_kernel::CaseId::new(0)],
        );
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut engine = Engine::<SecretProblem>::builder()
            .budget(Budget::unlimited())
            .evaluator(ParseFailureEvaluator)
            .build();
        engine
            .insert_seed(TestArtifact("seed".to_owned()), 0)
            .unwrap();
        let lm = RecordingLm::new("-lm", 1, 1);
        let requests = lm.requests();
        let reflector = LmBackedReflector::new(
            lm,
            "mock-reflector",
            DefaultReflectionRenderer,
            PlainTextEditParser,
        );
        let mut gepa = Gepa::new(
            WholeTextSurface,
            ParetoFrontier::by_case().build(),
            reflector,
        )
        .reflective_dataset(GepaReflectiveDataset::with_case_input(
            |case: &SecretCase| {
                let _hidden_target_is_available_but_not_projected = case.hidden_target;
                case.input.to_owned()
            },
        ))
        .validation_policy(MinibatchThenValidation)
        .max_iterations(1);

        engine.run(&mut gepa, &case_set, &store).await.unwrap();

        let captured = requests.lock().expect("requests lock").clone();
        assert_eq!(captured.len(), 1);
        let rendered = captured[0]
            .messages
            .iter()
            .map(Message::content)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("## Input\nwhat is 19 + 23?"));
        assert!(rendered.contains("## Output\nforty-two"));
        assert!(rendered.contains("## Score\n0"));
        assert!(rendered.contains("could not be parsed as an integer"));
        assert!(rendered.contains("The correct answer is 42"));
        assert!(!rendered.contains("SECRET_TARGET_DO_NOT_RENDER"));
    });
}

#[test]
fn reflect_request_informed_by_unions_request_and_example_source_refs() {
    let candidate = CandidateId::new();
    let assessment = AssessmentId::new();
    let example_source = InfoRef::External(ExternalRef {
        kind: "fixture".to_owned(),
        id: "trace-row".to_owned(),
    });

    let request = ReflectRequest::for_part(candidate, "text", "text")
        .with_source_refs([
            InfoRef::Candidate(candidate),
            InfoRef::Assessment(assessment),
        ])
        .with_examples([{
            let mut case = reflective_case(
                Some(CaseId::new(7)),
                "find the remainder",
                Some("31"),
                Some(0.25),
                "needs modular arithmetic",
            );
            case.source_refs.push(example_source.clone());
            case
        }]);

    let refs = request.informed_by();
    assert!(refs.contains(&InfoRef::Candidate(candidate)));
    assert!(refs.contains(&InfoRef::Assessment(assessment)));
    assert!(refs.contains(&example_source));
}

#[test]
fn default_renderer_and_plain_text_parser_cover_empty_feedback_and_bad_part() {
    let parent = CandidateId::new();
    let artifact = TestArtifact("seed".to_owned());
    let surface = WholeTextSurface;
    let config = LmBackedReflectorConfig::default();
    let request = ReflectRequest::for_part(parent, "text", "text");

    let rendered = DefaultReflectionRenderer
        .render(ReflectionRenderInput::<TestProblem, WholeTextSurface> {
            request: &request,
            artifact: &artifact,
            surface: &surface,
            model: "mock-renderer".into(),
            config: &config,
        })
        .unwrap();
    let rendered_text = rendered
        .messages
        .iter()
        .map(Message::content)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered_text.contains("(no reflective examples were selected)"));

    let no_output_request =
        ReflectRequest::for_part(parent, "text", "text").with_examples([reflective_case(
            Some(CaseId::new(9)),
            "the input",
            None,
            Some(1.0),
            "already correct",
        )]);
    let no_output_rendered = DefaultReflectionRenderer
        .render(ReflectionRenderInput::<TestProblem, WholeTextSurface> {
            request: &no_output_request,
            artifact: &artifact,
            surface: &surface,
            model: "mock-renderer".into(),
            config: &config,
        })
        .unwrap();
    let no_output_text = no_output_rendered
        .messages
        .iter()
        .map(Message::content)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(no_output_text.contains("already correct"));
    assert!(no_output_text.contains("## Input\nthe input"));
    assert!(!no_output_text.contains("## Trace"));

    let first_attempt = ReflectRequest::for_part(parent, "text", "text").with_attempt_index(1);
    let second_attempt = ReflectRequest::for_part(parent, "text", "text").with_attempt_index(2);
    let first_rendered = DefaultReflectionRenderer
        .render(ReflectionRenderInput::<TestProblem, WholeTextSurface> {
            request: &first_attempt,
            artifact: &artifact,
            surface: &surface,
            model: "mock-renderer".into(),
            config: &config,
        })
        .unwrap();
    let second_rendered = DefaultReflectionRenderer
        .render(ReflectionRenderInput::<TestProblem, WholeTextSurface> {
            request: &second_attempt,
            artifact: &artifact,
            surface: &surface,
            model: "mock-renderer".into(),
            config: &config,
        })
        .unwrap();
    assert_eq!(first_rendered.messages, second_rendered.messages);
    assert_eq!(
        first_rendered
            .provider_hints
            .metadata
            .get("gepa_attempt_index")
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(
        second_rendered
            .provider_hints
            .metadata
            .get("gepa_attempt_index")
            .map(String::as_str),
        Some("2")
    );

    let request = request.with_source_refs([
        InfoRef::Candidate(parent),
        InfoRef::Assessment(AssessmentId::new()),
    ]);
    let batch: ProposalBatch<TestProblem> = PlainTextEditParser
        .parse("-direct", &request, &artifact, &surface)
        .unwrap();
    assert_eq!(batch.semantics, ProposalBatchSemantics::Alternatives);
    assert_eq!(batch.proposals.len(), 1);
    assert!(!batch.proposals[0].provenance.informed_by_refs().is_empty());

    let bad_request = ReflectRequest::for_part(parent, "missing", "missing");
    let result: Result<ProposalBatch<TestProblem>, _> =
        PlainTextEditParser.parse("-direct", &bad_request, &artifact, &surface);
    let Err(error) = result else {
        panic!("bad part should fail parse");
    };
    assert!(
        error
            .to_string()
            .contains("GEPA surface edit lowering failed")
    );
}

#[test]
fn default_renderer_uses_gepa_prompt_template_and_config_override() {
    let parent = CandidateId::new();
    let artifact = TestArtifact("old instruction".to_owned());
    let surface = WholeTextSurface;
    let request =
        ReflectRequest::for_part(parent, "text", "text").with_examples([reflective_case(
            Some(CaseId::new(1)),
            "an example input",
            Some("42"),
            Some(0.0),
            "needs a modular arithmetic strategy",
        )]);

    let default_rendered = DefaultReflectionRenderer
        .render(ReflectionRenderInput::<TestProblem, WholeTextSurface> {
            request: &request,
            artifact: &artifact,
            surface: &surface,
            model: "mock-renderer".into(),
            config: &LmBackedReflectorConfig::default(),
        })
        .unwrap();
    let default_text = default_rendered
        .messages
        .iter()
        .map(Message::content)
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(default_rendered.messages.len(), 1);
    assert_eq!(
        default_rendered.messages.as_slice()[0].role(),
        Role::User,
        "GEPA default reflection should be the upstream prompt as a user turn, not a synthetic system prompt",
    );
    assert!(default_text.contains(DEFAULT_REFLECTION_PROMPT_TEMPLATE.lines().next().unwrap()));
    assert!(default_text.contains("old instruction"));
    assert!(default_text.contains("needs a modular arithmetic strategy"));
    assert!(default_text.contains("## Output\n42"));

    let config = LmBackedReflectorConfig::default()
        .with_prompt_template("CURRENT=<curr_param>\nFEEDBACK=<side_info>");
    let custom_rendered = DefaultReflectionRenderer
        .render(ReflectionRenderInput::<TestProblem, WholeTextSurface> {
            request: &request,
            artifact: &artifact,
            surface: &surface,
            model: "mock-renderer".into(),
            config: &config,
        })
        .unwrap();
    let custom_text = custom_rendered
        .messages
        .iter()
        .map(Message::content)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(custom_text.contains("CURRENT=old instruction"));
    assert!(custom_text.contains("FEEDBACK=# Example 1"));
}

#[test]
fn default_renderer_reports_bad_surface_and_bad_template() {
    let parent = CandidateId::new();
    let artifact = TestArtifact("old instruction".to_owned());
    let request = ReflectRequest::for_part(parent, "text", "text");

    let error = DefaultReflectionRenderer
        .render(
            ReflectionRenderInput::<TestProblem, FailingProjectionSurface> {
                request: &request,
                artifact: &artifact,
                surface: &FailingProjectionSurface,
                model: "mock-renderer".into(),
                config: &LmBackedReflectorConfig::default(),
            },
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("GEPA reflection surface projection failed")
    );

    let missing_part = ReflectRequest::for_part(parent, "missing", "missing");
    let error = DefaultReflectionRenderer
        .render(ReflectionRenderInput::<TestProblem, WholeTextSurface> {
            request: &missing_part,
            artifact: &artifact,
            surface: &WholeTextSurface,
            model: "mock-renderer".into(),
            config: &LmBackedReflectorConfig::default(),
        })
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("selected GEPA reflection part \"missing\" is missing from surface")
    );

    let bad_template = LmBackedReflectorConfig::default().with_prompt_template("missing both");
    let error = DefaultReflectionRenderer
        .render(ReflectionRenderInput::<TestProblem, WholeTextSurface> {
            request: &request,
            artifact: &artifact,
            surface: &WholeTextSurface,
            model: "mock-renderer".into(),
            config: &bad_template,
        })
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("GEPA reflection prompt template is missing placeholder(s)")
    );
}

#[test]
fn plain_text_parser_extracts_gepa_fenced_replacement() {
    let parent = CandidateId::new();
    let artifact = TestArtifact("seed".to_owned());
    let surface = WholeTextSurface;
    let request = ReflectRequest::for_part(parent, "text", "text");
    let batch: ProposalBatch<TestProblem> = PlainTextEditParser
        .parse(
            "Checklist:\n- use modular arithmetic\n```\nnew instruction\n```",
            &request,
            &artifact,
            &surface,
        )
        .unwrap();

    let leaven_core::ProposalEffect::Change { change, .. } = &batch.proposals[0].effect else {
        panic!("plain text parser should produce a mutation proposal");
    };
    assert_eq!(change, "new instruction");
}

#[test]
fn plain_text_parser_matches_upstream_language_fence_detection() {
    let parent = CandidateId::new();
    let artifact = TestArtifact("seed".to_owned());
    let surface = WholeTextSurface;
    let request = ReflectRequest::for_part(parent, "text", "text");

    let batch: ProposalBatch<TestProblem> = PlainTextEditParser
        .parse(
            "``` text\nnew instruction\n```",
            &request,
            &artifact,
            &surface,
        )
        .unwrap();

    let leaven_core::ProposalEffect::Change { change, .. } = &batch.proposals[0].effect else {
        panic!("plain text parser should produce a mutation proposal");
    };
    assert_eq!(change, "text\nnew instruction");
}

#[test]
fn plain_text_parser_matches_upstream_output_extractor_cases() {
    let parent = CandidateId::new();
    let artifact = TestArtifact("seed".to_owned());
    let surface = WholeTextSurface;
    let request = ReflectRequest::for_part(parent, "text", "text");

    let cases = [
        (
            "Here's the improved instruction:\n```markdown\nThis is the actual instruction content.\nIt should not include the word 'markdown'.\n```\n",
            "This is the actual instruction content.\nIt should not include the word 'markdown'.",
        ),
        (
            "Here's the instruction:\n```\nThis is the instruction without language specifier.\n```\nDone.",
            "This is the instruction without language specifier.",
        ),
        (
            "```markdown\nDon't get confused by these backticks: ```\n```",
            "Don't get confused by these backticks: ```",
        ),
        (
            "```\n\nHere are the instructions.\n\n```",
            "Here are the instructions.",
        ),
        (
            "Begin text\n```plaintext\nBegin instructions\n\n```\nInternal block 1\n```\n\n```python\nInternal block 2\n```\n\nEnd instructions\n```\nEnd text\n",
            "Begin instructions\n\n```\nInternal block 1\n```\n\n```python\nInternal block 2\n```\n\nEnd instructions",
        ),
        (
            "```text\nHere are the instructions.",
            "Here are the instructions.",
        ),
        (
            "Here are the instructions.\n```",
            "Here are the instructions.",
        ),
        (
            "\nHere are some backticks:\n```\nI hope you didn't get confused.\n                ",
            "Here are some backticks:\n```\nI hope you didn't get confused.",
        ),
        (
            "\n                Here are the instructions.\n                ",
            "Here are the instructions.",
        ),
    ];

    for (assistant_text, expected) in cases {
        let batch: ProposalBatch<TestProblem> = PlainTextEditParser
            .parse(assistant_text, &request, &artifact, &surface)
            .unwrap();
        let leaven_core::ProposalEffect::Change { change, .. } = &batch.proposals[0].effect else {
            panic!("plain text parser should produce a mutation proposal");
        };
        assert_eq!(change, expected);
    }
}

#[test]
fn plain_text_parser_handles_unclosed_and_inline_fences() {
    let parent = CandidateId::new();
    let artifact = TestArtifact("seed".to_owned());
    let surface = WholeTextSurface;
    let request = ReflectRequest::for_part(parent, "text", "text");

    let unclosed: ProposalBatch<TestProblem> = PlainTextEditParser
        .parse("```text\nnew instruction", &request, &artifact, &surface)
        .unwrap();
    let leaven_core::ProposalEffect::Change { change, .. } = &unclosed.proposals[0].effect else {
        panic!("plain text parser should produce a mutation proposal");
    };
    assert_eq!(change, "new instruction");

    let inline: ProposalBatch<TestProblem> = PlainTextEditParser
        .parse("```new instruction", &request, &artifact, &surface)
        .unwrap();
    let leaven_core::ProposalEffect::Change { change, .. } = &inline.proposals[0].effect else {
        panic!("plain text parser should produce a mutation proposal");
    };
    assert_eq!(change, "new instruction");
}

#[test]
fn default_lm_backed_reflector_constructor_is_typed() {
    let _reflector: LmBackedReflector<RecordingLm, DefaultReflectionRenderer, PlainTextEditParser> =
        LmBackedReflector::with_default_renderer(RecordingLm::new("-default", 1, 1), "default");
    let gepa = Gepa::reflect_with_lm(RecordingLm::new("-builder", 1, 1), "builder")
        .with_reflector_config(LmBackedReflectorConfig::default())
        .surface(WholeTextSurface)
        .population(ParetoFrontier::by_case().build());

    assert!(gepa.population().best().is_none());

    let gepa = Gepa::reflect_with_lm(RecordingLm::new("-build", 1, 1), "builder")
        .surface(WholeTextSurface)
        .build();
    assert!(gepa.population().best().is_none());

    let gepa = Gepa::reference()
        .reflect_with_lm(RecordingLm::new("-reference", 1, 1), "reference")
        .surface(WholeTextSurface)
        .build();
    assert!(gepa.population().best().is_none());

    let gepa = Gepa::reference()
        .surface(WholeTextSurface)
        .reflect_with_lm(RecordingLm::new("-surface-reference", 1, 1), "reference");
    assert!(gepa.population().best().is_none());
}

#[test]
fn lm_backed_reflector_rejects_missing_parent_before_lm_call() {
    block_on(async {
        let lm = RecordingLm::new("-unused", 1, 1);
        let requests = lm.requests();
        let mut reflector = LmBackedReflector::with_default_renderer(lm, "missing-parent");
        let mut graph = RunGraph::<TestProblem>::new(leaven_kernel::RunId::new());
        let mut budget = BudgetLedger::default();
        let mut ctx = RunContext::new(&mut graph, &mut budget);
        let request = ReflectRequest {
            parent: CandidateId::new(),
            part: "text",
            part_label: "text".to_owned(),
            examples: Vec::new(),
            source_refs: Vec::new(),
            attempt_index: None,
        };

        let error = reflector
            .reflect_candidate(&mut ctx, &WholeTextSurface, request)
            .await
            .expect_err("missing parent must fail before LM request");

        assert!(format!("{error:?}").contains("selected parent"));
        assert!(requests.lock().expect("requests lock").is_empty());
    });
}

#[test]
fn lm_backed_reflector_surfaces_lm_failures_without_candidate() {
    block_on(async {
        let case_set = CaseSet::new(vec!["the case input"]).with_partition(
            leaven_core::PartitionId::from("TRAIN"),
            vec![leaven_kernel::CaseId::new(0)],
        );
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut engine = Engine::<TestProblem>::builder()
            .budget(Budget::unlimited())
            .evaluator(FeedbackEvaluator)
            .build();
        let seed = engine
            .insert_seed(TestArtifact("seed".to_owned()), 0)
            .unwrap();
        let reflector = LmBackedReflector::new(
            FailingLm,
            "mock-reflector",
            DefaultReflectionRenderer,
            PlainTextEditParser,
        );
        let mut gepa = Gepa::new(
            WholeTextSurface,
            ParetoFrontier::by_case().build(),
            reflector,
        )
        .validation_policy(MinibatchThenValidation);

        let error = engine.run(&mut gepa, &case_set, &store).await.unwrap_err();

        assert!(error.to_string().contains("GEPA reflection failed"));
        assert!(engine.view().candidate_tree().children(seed).is_empty());
    });
}

#[test]
fn lm_backed_reflector_surfaces_parser_failures_without_candidate() {
    block_on(async {
        let case_set = CaseSet::new(vec!["the case input"]).with_partition(
            leaven_core::PartitionId::from("TRAIN"),
            vec![leaven_kernel::CaseId::new(0)],
        );
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut engine = Engine::<TestProblem>::builder()
            .budget(Budget::unlimited())
            .evaluator(FeedbackEvaluator)
            .build();
        let seed = engine
            .insert_seed(TestArtifact("seed".to_owned()), 0)
            .unwrap();
        let reflector = LmBackedReflector::new(
            RecordingLm::new("-lm", 1, 1),
            "mock-reflector",
            DefaultReflectionRenderer,
            PlainTextEditParser,
        );
        let mut gepa = Gepa::new(
            RejectingSurface,
            ParetoFrontier::by_case().build(),
            reflector,
        )
        .validation_policy(MinibatchThenValidation);

        let error = engine.run(&mut gepa, &case_set, &store).await.unwrap_err();

        assert!(error.to_string().contains("GEPA reflection failed"));
        assert!(engine.view().candidate_tree().children(seed).is_empty());
    });
}

#[derive(Clone, Debug)]
struct TestArtifact(String);

impl Artifact for TestArtifact {
    type Change = String;
    type ApplyError = std::convert::Infallible;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(content_id(&self.0))
    }

    fn cache_identity(&self) -> Option<CacheIdentity> {
        Some(CacheIdentity::Content(content_id(&self.0)))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(format!("{}{change}", self.0)))
    }
}

struct TestProblem;

impl OptimizationProblem for TestProblem {
    type Artifact = TestArtifact;
    type Case = &'static str;
    type Evidence = CaseAssessmentEvidence;
    type ProposalAnnotations = ();
}

struct SecretProblem;

impl OptimizationProblem for SecretProblem {
    type Artifact = TestArtifact;
    type Case = SecretCase;
    type Evidence = CaseAssessmentEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug)]
struct SecretCase {
    input: &'static str,
    hidden_target: &'static str,
}

#[derive(Clone)]
struct RecordingLm {
    response: String,
    input_tokens: u64,
    output_tokens: u64,
    requests: Arc<Mutex<Vec<LmRequest>>>,
}

impl RecordingLm {
    fn new(response: impl Into<String>, input_tokens: u64, output_tokens: u64) -> Self {
        Self {
            response: response.into(),
            input_tokens,
            output_tokens,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requests(&self) -> Arc<Mutex<Vec<LmRequest>>> {
        self.requests.clone()
    }
}

impl Lm for RecordingLm {
    fn id(&self) -> LmId {
        LmId::from("recording")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([9; 32])
    }

    async fn complete(&self, request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
        self.requests.lock().expect("requests lock").push(request);
        let usage = TokenUsage {
            input_tokens: self.input_tokens,
            cached_input_tokens: 0,
            output_tokens: self.output_tokens,
            reasoning_tokens: 0,
        };
        let response = LmResponse::new(Message::assistant(self.response.clone()), usage.clone())
            .map_err(|error| LmError::invalid_response("recording", error.to_string()))?;
        Ok(Metered::new(response, usage.to_cost()))
    }
}

#[derive(Clone)]
struct FailingLm;

impl Lm for FailingLm {
    fn id(&self) -> LmId {
        LmId::from("failing")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([8; 32])
    }

    async fn complete(&self, _request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
        Err(LmError::provider("failing", None, "boom"))
    }
}

struct ArtifactFeedbackEvaluator;

impl Evaluator<TestProblem> for ArtifactFeedbackEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([7; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        let set = leaven_kernel::EvaluationSetId::from_uuid(request.set.id.as_uuid());
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        let mut assessments = Vec::new();
        for candidate in candidates {
            let artifact = ctx.graph().artifact(candidate).expect("candidate artifact");
            let improvement_count = artifact.0.matches("-lm").count();
            let improvement_score = f64::from(
                u32::try_from(improvement_count).expect("test improvement count fits u32"),
            );
            for case in request.set.case_ids.iter().copied() {
                assessments.push(Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Case { set, case },
                    evidence: CaseAssessmentEvidence::new(
                        ScalarEvidence::new(improvement_score).unwrap(),
                        OutputRecord::inline(format!("output for {}", artifact.0)),
                        format!("feedback for {}", artifact.0),
                    ),
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                });
            }
        }
        let cost = Cost::metric_calls(assessments.len() as u64);
        Ok(Metered::new(assessments, cost))
    }
}

struct FeedbackEvaluator;

impl Evaluator<TestProblem> for FeedbackEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([3; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        let set = leaven_kernel::EvaluationSetId::from_uuid(request.set.id.as_uuid());
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        let mut assessments = Vec::new();
        for candidate in candidates {
            for case in request.set.case_ids.iter().copied() {
                assessments.push(Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Case { set, case },
                    evidence: CaseAssessmentEvidence::new(
                        ScalarEvidence::new(if candidate_suffix(candidate) {
                            1.0
                        } else {
                            0.0
                        })
                        .unwrap(),
                        OutputRecord::inline("candidate output"),
                        "candidate missed the target suffix",
                    ),
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                });
            }
        }
        let cost = Cost::metric_calls(assessments.len() as u64);
        Ok(Metered::new(assessments, cost))
    }
}

struct ParseFailureEvaluator;

impl Evaluator<SecretProblem> for ParseFailureEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([10; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, SecretProblem>,
    ) -> Result<Metered<Vec<Assessment<SecretProblem>>>, EvaluationError> {
        let set = leaven_kernel::EvaluationSetId::from_uuid(request.set.id.as_uuid());
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        let mut assessments = Vec::new();
        for candidate in candidates {
            for case in request.set.case_ids.iter().copied() {
                assessments.push(Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Case { set, case },
                    evidence: CaseAssessmentEvidence::new(
                        ScalarEvidence::new(0.0).unwrap(),
                        OutputRecord::inline("forty-two"),
                        "The answer could not be parsed as an integer. The correct answer is 42.",
                    ),
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                });
            }
        }
        let cost = Cost::metric_calls(assessments.len() as u64);
        Ok(Metered::new(assessments, cost))
    }
}

#[derive(Clone, Debug)]
struct WholeTextSurface;

impl EditSurface<TestArtifact> for WholeTextSurface {
    type PartId = &'static str;
    type Address = PartAddress;
    type View<'a> = &'a str;
    type Edit = String;

    fn fingerprint(&self) -> SurfaceFingerprint {
        SurfaceFingerprint(Fingerprint::from_bytes([4; 32]))
    }

    fn parts<'a>(
        &self,
        artifact: &'a TestArtifact,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        Ok(vec![Part {
            id: "text",
            address: PartAddress("text".to_owned()),
            view: artifact.0.as_str(),
        }])
    }

    fn change_part(
        &self,
        _artifact: &TestArtifact,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<<TestArtifact as Artifact>::Change, SurfaceError> {
        if id == "text" {
            Ok(edit)
        } else {
            Err(SurfaceError::UnknownPart)
        }
    }
}

#[derive(Clone, Debug)]
struct RejectingSurface;

impl EditSurface<TestArtifact> for RejectingSurface {
    type PartId = &'static str;
    type Address = PartAddress;
    type View<'a> = &'a str;
    type Edit = String;

    fn fingerprint(&self) -> SurfaceFingerprint {
        SurfaceFingerprint(Fingerprint::from_bytes([5; 32]))
    }

    fn parts<'a>(
        &self,
        artifact: &'a TestArtifact,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        Ok(vec![Part {
            id: "text",
            address: PartAddress("text".to_owned()),
            view: artifact.0.as_str(),
        }])
    }

    fn change_part(
        &self,
        _artifact: &TestArtifact,
        _id: Self::PartId,
        _edit: Self::Edit,
    ) -> Result<<TestArtifact as Artifact>::Change, SurfaceError> {
        Err(SurfaceError::UnknownPart)
    }
}

#[derive(Clone, Debug)]
struct FailingProjectionSurface;

impl EditSurface<TestArtifact> for FailingProjectionSurface {
    type PartId = &'static str;
    type Address = PartAddress;
    type View<'a> = &'a str;
    type Edit = String;

    fn fingerprint(&self) -> SurfaceFingerprint {
        SurfaceFingerprint(Fingerprint::from_bytes([6; 32]))
    }

    fn parts<'a>(
        &self,
        _artifact: &'a TestArtifact,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        Err(SurfaceError::UnknownPart)
    }

    fn change_part(
        &self,
        _artifact: &TestArtifact,
        _id: Self::PartId,
        _edit: Self::Edit,
    ) -> Result<<TestArtifact as Artifact>::Change, SurfaceError> {
        Err(SurfaceError::UnknownPart)
    }
}

fn candidate_suffix(_candidate: CandidateId) -> bool {
    false
}

fn content_id(text: &str) -> ContentId {
    let mut bytes = [0; 32];
    let raw = text.as_bytes();
    bytes[..raw.len().min(32)].copy_from_slice(&raw[..raw.len().min(32)]);
    ContentId::from_bytes(bytes)
}
