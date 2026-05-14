use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentTarget, CacheIdentity, ExternalRef, InfoRef,
    OptimizationProblem, ProposalBatch, ProposalBatchSemantics, ResolvedEvaluationRequest,
    ResolvedRequestKind,
};
use leaven_engine::{CachePolicy, CaseSet, Engine, EvaluationContext, EvaluationError, Evaluator};
use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence, ScoredFeedbackEvidence};
use leaven_gepa::{
    DEFAULT_REFLECTION_PROMPT_TEMPLATE, DefaultReflectionRenderer, Gepa, GepaReflectionEvidence,
    LmBackedReflector, LmBackedReflectorConfig, PlainTextEditParser, ReflectRequest,
    ReflectionOutputParser, ReflectionRenderInput, ReflectionRenderer, ReflectiveFeedbackRecord,
    SelectedFeedback,
};
use leaven_kernel::{
    AssessmentId, Budget, CandidateId, CaseId, ContentId, Cost, EvaluatorId, Fingerprint,
    MetadataBag, Metered, ProposerId, StageId,
};
use leaven_lm::{Lm, LmError, LmId, LmRequest, LmResponse, Message, Role, TokenUsage};
use leaven_population::ParetoFrontier;
use leaven_store_inline::InlineEvidenceStore;
use leaven_surface::{EditSurface, Part, PartAddress, SurfaceError, SurfaceFingerprint};

#[test]
fn lm_backed_reflector_renders_feedback_records_and_applies_candidate() {
    block_on(async {
        let case_set = CaseSet::new(vec![()]).with_partition(
            leaven_core::PartitionId::from("TRAIN"),
            vec![leaven_kernel::CaseId::new(0)],
        );
        let store = InlineEvidenceStore::<CasewiseEvidence<ScoredFeedbackEvidence>>::new("inline");
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
        );

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
        assert!(rendered.contains("runner trace: saw seed"));

        let proposal = view
            .proposal_that_created(candidate)
            .expect("applied candidate has proposal");
        assert!(
            proposal
                .provenance()
                .informed_by
                .contains(&leaven_core::InfoRef::Candidate(seed))
        );
        assert!(
            proposal
                .provenance()
                .informed_by
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
fn reflection_feedback_records_preserve_all_source_refs() {
    let candidate = CandidateId::new();
    let assessment = AssessmentId::new();
    let external = InfoRef::External(ExternalRef {
        kind: "fixture".to_owned(),
        id: "feedback-row".to_owned(),
    });
    let record_source = InfoRef::External(ExternalRef {
        kind: "fixture".to_owned(),
        id: "trace-row".to_owned(),
    });

    let selected = SelectedFeedback {
        candidate_refs: vec![candidate],
        evidence_refs: vec![external.clone()],
        ..SelectedFeedback::default()
    }
    .with_assessments([assessment])
    .with_records([ReflectiveFeedbackRecord {
        case: Some(CaseId::new(7)),
        score: Some(0.25),
        feedback: "needs modular arithmetic".to_owned(),
        trace: vec!["missed 32nds conversion".to_owned()],
        source_refs: vec![record_source.clone()],
    }]);

    let refs = selected.source_refs();
    assert!(refs.contains(&InfoRef::Candidate(candidate)));
    assert!(refs.contains(&InfoRef::Assessment(assessment)));
    assert!(refs.contains(&external));
    assert!(refs.contains(&record_source));
}

#[test]
fn scalar_casewise_evidence_projects_reflection_records() {
    let evidence = CasewiseEvidence::new(vec![CaseOutcome::new(
        CaseId::new(3),
        ScalarEvidence::new(0.75).unwrap(),
    )]);

    let records = evidence.reflection_records();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].case, Some(CaseId::new(3)));
    assert_eq!(records[0].score, Some(0.75));
    assert!(records[0].feedback.is_empty());
    assert!(records[0].trace.is_empty());
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
    assert!(rendered_text.contains("(no textual feedback records were selected)"));

    let no_trace_request = ReflectRequest::for_part(parent, "text", "text").with_selected_feedback(
        SelectedFeedback::default().with_records([ReflectiveFeedbackRecord {
            case: Some(CaseId::new(9)),
            score: Some(1.0),
            feedback: "already correct".to_owned(),
            trace: Vec::new(),
            source_refs: Vec::new(),
        }]),
    );
    let no_trace_rendered = DefaultReflectionRenderer
        .render(ReflectionRenderInput::<TestProblem, WholeTextSurface> {
            request: &no_trace_request,
            artifact: &artifact,
            surface: &surface,
            model: "mock-renderer".into(),
            config: &config,
        })
        .unwrap();
    let no_trace_text = no_trace_rendered
        .messages
        .iter()
        .map(Message::content)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(no_trace_text.contains("already correct"));
    assert!(!no_trace_text.contains("trace:"));

    let selected = SelectedFeedback {
        candidate_refs: vec![parent],
        assessment_refs: vec![AssessmentId::new()],
        ..SelectedFeedback::default()
    };
    let request = request.with_selected_feedback(selected);
    let batch: ProposalBatch<TestProblem> = PlainTextEditParser
        .parse("-direct", &request, &artifact, &surface)
        .unwrap();
    assert_eq!(batch.semantics, ProposalBatchSemantics::Alternatives);
    assert_eq!(batch.proposals.len(), 1);
    assert!(!batch.proposals[0].provenance.informed_by.is_empty());

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
    let request = ReflectRequest::for_part(parent, "text", "text").with_selected_feedback(
        SelectedFeedback::default().with_records([ReflectiveFeedbackRecord {
            case: Some(CaseId::new(1)),
            score: Some(0.0),
            feedback: "needs a modular arithmetic strategy".to_owned(),
            trace: vec!["solver_answer: 42".to_owned()],
            source_refs: Vec::new(),
        }]),
    );

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
    assert!(default_text.contains("solver_answer: 42"));

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
}

#[test]
fn lm_backed_reflector_surfaces_lm_failures_without_candidate() {
    block_on(async {
        let case_set = CaseSet::new(vec![()]).with_partition(
            leaven_core::PartitionId::from("TRAIN"),
            vec![leaven_kernel::CaseId::new(0)],
        );
        let store = InlineEvidenceStore::<CasewiseEvidence<ScoredFeedbackEvidence>>::new("inline");
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
        );

        let error = engine.run(&mut gepa, &case_set, &store).await.unwrap_err();

        assert!(error.to_string().contains("GEPA reflection failed"));
        assert!(engine.view().candidate_tree().children(seed).is_empty());
    });
}

#[test]
fn lm_backed_reflector_surfaces_parser_failures_without_candidate() {
    block_on(async {
        let case_set = CaseSet::new(vec![()]).with_partition(
            leaven_core::PartitionId::from("TRAIN"),
            vec![leaven_kernel::CaseId::new(0)],
        );
        let store = InlineEvidenceStore::<CasewiseEvidence<ScoredFeedbackEvidence>>::new("inline");
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
        );

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
    type Case = ();
    type Evidence = CasewiseEvidence<ScoredFeedbackEvidence>;
    type ProposalAnnotations = ();
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
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        Ok(Metered::new(
            candidates
                .into_iter()
                .map(|candidate| Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::EvaluationSet(leaven_kernel::EvaluationSetId::new()),
                    evidence: CasewiseEvidence::new(vec![CaseOutcome::new(
                        leaven_kernel::CaseId::new(0),
                        ScoredFeedbackEvidence::new(
                            ScalarEvidence::new(if candidate_suffix(candidate) {
                                1.0
                            } else {
                                0.0
                            })
                            .unwrap(),
                            "candidate missed the target suffix",
                            vec!["runner trace: saw seed".to_owned()],
                        ),
                    )]),
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                })
                .collect(),
            Cost::metric_calls(1),
        ))
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
