use futures::executor::block_on;
use leaven_agent::{AgentSession, FakeAgentAction, FakeAgentRuntime};
use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentTarget, CacheIdentity, Evidence,
    OptimizationProblem, Proposal, ProposalBatch, ProposalBatchSemantics,
    ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_engine::{
    CachePolicy, CaseSet, Engine, EvaluationContext, EvaluationError, Evaluator, RunContext,
    RunEvent,
};
use leaven_evidence::ScalarEvidence;
use leaven_gepa::{
    FixedSurfaceEdit, Gepa, GepaReflectionBootstrap, GepaReflector, LmBackedReflector,
    PlainTextEditParser, ReflectRequest, ReflectionError, ReflectionRenderInput,
    ReflectionRenderer, ReflectiveDatasetBuilder, ReflectiveExample, gepa_stage_proposer,
};
use leaven_kernel::{
    AssessmentId, Budget, BudgetSnapshot, CandidateId, ContentId, Cost, EvaluatorId, Fingerprint,
    MetadataBag, Metered, StageAttemptOutcome, StageCallId, StageRole,
};
use leaven_population::ParetoFrontier;
use leaven_stage::{
    AgentStageBootstrap, AgentStageCallContext, ProposerSlot, StageOutputParseError,
    StageOutputParser, StageQuery, StageQueryKind, receipt_store::StageReceiptStore,
};
use leaven_store_inline::InlineEvidenceStore;
use leaven_surface::{EditSurface, Part, PartAddress, SurfaceError, SurfaceFingerprint};
use leaven_workspace::{WorkspacePath, WorkspaceView};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn gepa_reflection_bootstrap_prewarms_parent_and_feedback_queries() {
    block_on(async {
        let parent = CandidateId::new();
        let sibling = CandidateId::new();
        let feedback = AssessmentId::new();
        // The parent ref is deduplicated; a sibling candidate ref and an
        // assessment ref each become their own prewarm query.
        let request = ReflectRequest::new(parent, "answer").with_source_refs([
            leaven_core::InfoRef::Candidate(parent),
            leaven_core::InfoRef::Candidate(sibling),
            leaven_core::InfoRef::Assessment(feedback),
        ]);
        let ctx = AgentStageCallContext::new(
            StageCallId::new(),
            leaven_engine::ReadScope::default(),
            BudgetSnapshot::default(),
        );

        let plan = <GepaReflectionBootstrap as AgentStageBootstrap<
            TestProblem,
            ProposerSlot<ReflectRequest>,
        >>::plan(&GepaReflectionBootstrap::default(), request, ctx)
        .await
        .unwrap();

        assert_eq!(plan.role, StageRole::reflect());
        assert!(plan.query.allowed.contains(StageQueryKind::Candidate));
        assert_eq!(plan.query.prewarm[0], StageQuery::Candidate { id: parent });
        assert_eq!(plan.query.prewarm[1], StageQuery::Candidate { id: sibling });
        assert_eq!(
            plan.query.prewarm[2],
            StageQuery::Assessment { id: feedback }
        );
        assert_eq!(
            plan.query
                .prewarm
                .iter()
                .filter(|query| matches!(query, StageQuery::Candidate { id } if *id == parent))
                .count(),
            1,
            "the parent candidate is prewarmed once even when also a source ref",
        );
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

#[derive(Clone, Debug)]
struct TestEvidence;

impl Evidence for TestEvidence {}

impl leaven_gepa::GepaCaseEvidence for TestEvidence {
    fn scalar_score(&self) -> Option<ScalarEvidence> {
        Some(ScalarEvidence::new(1.0).unwrap())
    }
}

struct TestProblem;

impl OptimizationProblem for TestProblem {
    type Artifact = TestArtifact;
    type Case = ();
    type Evidence = TestEvidence;
    type ProposalAnnotations = ();
}

/// Reflective-dataset builder fixture: `TestEvidence` has no GEPA-parity
/// projection, so the agent routing test scripts one fixed example.
#[derive(Clone, Copy, Debug)]
struct ScriptedDataset;

fn scripted_examples() -> Vec<ReflectiveExample> {
    vec![ReflectiveExample {
        case: Some(leaven_kernel::CaseId::new(0)),
        input: "scripted reflection input".to_owned(),
        output: Some("scripted output".to_owned()),
        score: Some(0.5),
        feedback: "scripted feedback".to_owned(),
        source_refs: Vec::new(),
    }]
}

impl ReflectiveDatasetBuilder<TestProblem, WholeTextSurface> for ScriptedDataset {
    async fn build(
        &self,
        _ctx: &mut RunContext<'_, TestProblem>,
        _parent: CandidateId,
        _parent_assessments: &[AssessmentId],
        _part: &&'static str,
    ) -> Result<Vec<ReflectiveExample>, ReflectionError> {
        Ok(scripted_examples())
    }
}

#[test]
fn gepa_stage_proposer_routes_fake_runtime_through_run_context() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let parent = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TestArtifact("seed".to_owned()), 0).unwrap()
        };
        let proposer = gepa_stage_proposer(
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/proposal.json").unwrap(),
                bytes: br#"{"suffix":"-gepa"}"#.to_vec(),
            }]),
            JsonProposalParser,
            Default::default(),
        );

        let report = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.propose(
                &proposer,
                ReflectRequest::new(parent, "root").with_source_refs([
                    leaven_core::InfoRef::Candidate(parent),
                    leaven_core::InfoRef::Assessment(AssessmentId::new()),
                ]),
            )
            .await
            .unwrap()
        };
        let candidate = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.apply_batch(report.batch_id)
                .unwrap()
                .successful_candidates()
                .next()
                .unwrap()
        };
        let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);

        assert_eq!(ctx.graph().artifact(candidate).unwrap().0, "seed-gepa");
        let receipt_ref = ctx
            .graph()
            .events()
            .find_map(|event| match event {
                RunEvent::StageAttemptRecorded {
                    role,
                    receipt,
                    outcome: StageAttemptOutcome::Completed,
                    ..
                } if role == &StageRole::reflect() => Some(receipt.clone()),
                _ => None,
            })
            .expect("stage attempt event recorded");
        let receipt = proposer
            .receipt_store()
            .read(receipt_ref.id)
            .await
            .unwrap()
            .expect("stage receipt is persisted");
        assert!(receipt.queries.iter().any(|query| matches!(
            query.query,
            StageQuery::Candidate { id } if id == parent
        )));
        let proposal = ctx
            .graph()
            .proposal_that_created(candidate)
            .expect("applied candidate has proposal");
        assert!(
            proposal
                .provenance()
                .informed_by_refs()
                .contains(&leaven_core::InfoRef::Candidate(parent))
        );
    });
}

#[test]
fn gepa_optimizer_uses_agent_backed_reflection_path() {
    block_on(async {
        let case_set = CaseSet::new(vec![()])
            .with_partition(
                leaven_core::PartitionId::from("TRAIN"),
                vec![leaven_kernel::CaseId::new(0)],
            )
            .with_partition(
                leaven_core::PartitionId::from("VALIDATION"),
                vec![leaven_kernel::CaseId::new(0)],
            );
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut engine = Engine::<TestProblem>::builder()
            .evaluator(ConstantEvaluator)
            .build();
        let seed = engine
            .insert_seed(TestArtifact("seed".to_owned()), 0)
            .unwrap();
        let proposer = gepa_stage_proposer(
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/proposal.json").unwrap(),
                bytes: br#"{"suffix":"-optimizer"}"#.to_vec(),
            }]),
            JsonProposalParser,
            Default::default(),
        );
        let mut gepa = Gepa::new(
            WholeTextSurface,
            ParetoFrontier::by_case().build(),
            proposer,
        )
        .reflective_dataset(ScriptedDataset);

        engine.run(&mut gepa, &case_set, &store).await.unwrap();
        let view = engine.view();
        let candidate = view
            .events()
            .find_map(|event| match event {
                RunEvent::ApplySucceeded { candidate_id, .. } => Some(*candidate_id),
                _ => None,
            })
            .expect("GEPA applied the agent-backed proposal");

        assert_ne!(candidate, seed);
        assert_eq!(
            view.artifact(candidate).expect("candidate artifact").0,
            "seed-optimizer"
        );
        assert!(view.events().any(|event| matches!(
            event,
            RunEvent::StageAttemptRecorded {
                role,
                outcome: StageAttemptOutcome::Completed,
                ..
            } if role == &StageRole::reflect()
        )));
        let proposal = view
            .proposal_that_created(candidate)
            .expect("agent proposal created candidate");
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
    });
}

#[test]
fn fixed_surface_reflector_records_and_applies_through_run_context() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let parent = ctx.insert_seed(TestArtifact("seed".to_owned()), 0).unwrap();
        let parent_assessment = AssessmentId::new();
        let mut reflector = FixedSurfaceEdit::new("fixed".to_owned());

        let candidate = reflector
            .reflect_candidate(
                &mut ctx,
                &WholeTextSurface,
                ReflectRequest::for_part(parent, "text", "text").with_source_refs([
                    leaven_core::InfoRef::Candidate(parent),
                    leaven_core::InfoRef::Assessment(parent_assessment),
                ]),
            )
            .await
            .unwrap()
            .expect("fixed reflection applies a candidate");

        assert_eq!(ctx.graph().artifact(candidate).unwrap().0, "seedfixed");
        let proposal = ctx
            .graph()
            .proposal_that_created(candidate)
            .expect("fixed reflection proposal created candidate");
        assert!(
            proposal
                .provenance()
                .informed_by_refs()
                .contains(&leaven_core::InfoRef::Candidate(parent))
        );
        assert!(
            proposal
                .provenance()
                .informed_by_refs()
                .contains(&leaven_core::InfoRef::Assessment(parent_assessment))
        );
    });
}

#[test]
fn fixed_surface_reflector_reports_graph_surface_and_budget_failures() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let mut reflector = FixedSurfaceEdit::new("fixed".to_owned());
        let missing_parent = CandidateId::new();

        let error = reflector
            .reflect_candidate(
                &mut ctx,
                &WholeTextSurface,
                ReflectRequest::for_part(missing_parent, "text", "text"),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains(&format!(
            "selected parent {missing_parent} is missing from graph"
        )));

        let parent = ctx.insert_seed(TestArtifact("seed".to_owned()), 0).unwrap();
        let error = reflector
            .reflect_candidate(
                &mut ctx,
                &WholeTextSurface,
                ReflectRequest::for_part(parent, "missing", "missing"),
            )
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("GEPA surface edit lowering failed")
        );

        let mut capped_graph =
            leaven_engine::RunGraph::<TestProblem>::new(leaven_kernel::RunId::new());
        let mut capped_budget = leaven_engine::BudgetLedger::new(Budget::metric_calls(0));
        let mut capped_ctx = RunContext::<TestProblem>::new(&mut capped_graph, &mut capped_budget);
        let capped_parent = capped_ctx
            .insert_seed(TestArtifact("seed".to_owned()), 0)
            .unwrap();
        let error = reflector
            .reflect_candidate(
                &mut capped_ctx,
                &WholeTextSurface,
                ReflectRequest::for_part(capped_parent, "text", "text"),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("GEPA proposal recording failed"));
    });
}

/// Regression test for the GEPA reflection divergence bug.
///
/// Before the build-once-pass-down cutover, the LM-backed reflector projected
/// real per-case feedback while the agent-backed reflector hard-coded an empty
/// record list. This test proves that for one `(parent, part, examples)`
/// input, the LM reflector and the agent reflector receive byte-identical
/// `ReflectRequest.examples`.
#[test]
fn lm_and_agent_reflectors_receive_byte_identical_examples() {
    use std::sync::{Arc, Mutex};

    block_on(async {
        let examples = vec![
            ReflectiveExample {
                case: Some(leaven_kernel::CaseId::new(0)),
                input: "find the remainder when 2^10 is divided by 7".to_owned(),
                output: Some("3".to_owned()),
                score: Some(0.0),
                feedback: "incorrect; expected 2".to_owned(),
                source_refs: Vec::new(),
            },
            ReflectiveExample {
                case: Some(leaven_kernel::CaseId::new(1)),
                input: "what is 19 + 23".to_owned(),
                output: Some("42".to_owned()),
                score: Some(1.0),
                feedback: "correct".to_owned(),
                source_refs: Vec::new(),
            },
        ];
        let canonical = serde_json::to_vec(&examples).unwrap();

        // LM-backed reflection path: capture the examples the renderer sees.
        let lm_seen: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
        let (mut graph, mut budget) = graph_and_budget();
        let lm_parent = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TestArtifact("seed".to_owned()), 0).unwrap()
        };
        let mut lm_reflector = LmBackedReflector::new(
            FencedLm,
            "mock-reflector",
            RecordingExamplesRenderer {
                seen: lm_seen.clone(),
            },
            PlainTextEditParser,
        );
        let lm_request = ReflectRequest::for_part(lm_parent, "text", "text")
            .with_examples(examples.clone())
            .with_source_refs([leaven_core::InfoRef::Candidate(lm_parent)]);
        {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            lm_reflector
                .reflect_candidate(&mut ctx, &WholeTextSurface, lm_request)
                .await
                .unwrap();
        }

        // Agent-backed reflection path: capture the examples the parser sees.
        let agent_seen: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
        let (mut agent_graph, mut agent_budget) = graph_and_budget();
        let agent_parent = {
            let mut ctx = RunContext::<TestProblem>::new(&mut agent_graph, &mut agent_budget);
            ctx.insert_seed(TestArtifact("seed".to_owned()), 0).unwrap()
        };
        let mut agent_reflector = gepa_stage_proposer(
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/proposal.json").unwrap(),
                bytes: br#"{"suffix":"-agent"}"#.to_vec(),
            }]),
            RecordingExamplesParser {
                seen: agent_seen.clone(),
            },
            Default::default(),
        );
        let agent_request = ReflectRequest::for_part(agent_parent, "text", "text")
            .with_examples(examples.clone())
            .with_source_refs([leaven_core::InfoRef::Candidate(agent_parent)]);
        {
            let mut ctx = RunContext::<TestProblem>::new(&mut agent_graph, &mut agent_budget);
            agent_reflector
                .reflect_candidate(&mut ctx, &WholeTextSurface, agent_request)
                .await
                .unwrap();
        }

        let lm_bytes = lm_seen.lock().unwrap().clone().expect("LM renderer ran");
        let agent_bytes = agent_seen
            .lock()
            .unwrap()
            .clone()
            .expect("agent parser ran");
        assert_eq!(
            lm_bytes, agent_bytes,
            "LM and agent reflectors must receive byte-identical reflective examples",
        );
        assert_eq!(
            lm_bytes, canonical,
            "the examples both backends receive must be exactly the optimizer-built examples",
        );
    });
}

#[derive(Clone)]
struct RecordingExamplesRenderer {
    seen: std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>>,
}

impl ReflectionRenderer<TestProblem, WholeTextSurface> for RecordingExamplesRenderer {
    fn render(
        &self,
        input: ReflectionRenderInput<'_, TestProblem, WholeTextSurface>,
    ) -> Result<leaven_lm::LmRequest, leaven_engine::ProposalError> {
        *self.seen.lock().unwrap() =
            Some(serde_json::to_vec(&input.request.examples).expect("examples serialize"));
        Ok(leaven_lm::LmRequest::new(
            input.model,
            leaven_lm::Messages::from_user("reflect"),
        ))
    }
}

struct RecordingExamplesParser {
    seen: std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>>,
}

impl StageOutputParser<TestProblem, ProposerSlot<ReflectRequest>> for RecordingExamplesParser {
    async fn parse(
        &self,
        workspace: &mut WorkspaceView<'_>,
        _session: &AgentSession,
        plan: &leaven_stage::parser::ErasedStagePlan,
        _ctx: AgentStageCallContext,
    ) -> Result<Metered<ProposalBatch<TestProblem>>, StageOutputParseError> {
        let request: ReflectRequest = serde_json::from_value(plan.request_json.clone())?;
        *self.seen.lock().unwrap() =
            Some(serde_json::to_vec(&request.examples).expect("examples serialize"));
        let bytes = workspace.read_file(&WorkspacePath::new("output/proposal.json").unwrap())?;
        let parsed: ParsedProposal = serde_json::from_slice(&bytes)?;
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![
                    Proposal::mutate(request.parent, parsed.suffix)
                        .informed_by(request.informed_by())
                        .build(),
                ],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        ))
    }
}

#[derive(Clone)]
struct FencedLm;

impl leaven_lm::Lm for FencedLm {
    fn id(&self) -> leaven_lm::LmId {
        leaven_lm::LmId::from("fenced")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([12; 32])
    }

    async fn complete(
        &self,
        _request: leaven_lm::LmRequest,
    ) -> Result<Metered<leaven_lm::LmResponse>, leaven_lm::LmError> {
        let usage = leaven_lm::TokenUsage {
            input_tokens: 1,
            cached_input_tokens: 0,
            output_tokens: 1,
            reasoning_tokens: 0,
        };
        let response = leaven_lm::LmResponse::new(
            leaven_lm::Message::assistant("```\n-reflected\n```"),
            usage.clone(),
        )
        .map_err(|error| leaven_lm::LmError::invalid_response("fenced", error.to_string()))?;
        Ok(Metered::new(response, usage.to_cost()))
    }
}

struct JsonProposalParser;

impl StageOutputParser<TestProblem, ProposerSlot<ReflectRequest>> for JsonProposalParser {
    async fn parse(
        &self,
        workspace: &mut WorkspaceView<'_>,
        _session: &AgentSession,
        plan: &leaven_stage::parser::ErasedStagePlan,
        _ctx: AgentStageCallContext,
    ) -> Result<Metered<ProposalBatch<TestProblem>>, StageOutputParseError> {
        let bytes = workspace.read_file(&WorkspacePath::new("output/proposal.json").unwrap())?;
        let parsed: ParsedProposal = serde_json::from_slice(&bytes)?;
        let request: ReflectRequest = serde_json::from_value(plan.request_json.clone())?;
        let informed_by = request.informed_by();
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![
                    Proposal::mutate(request.parent, parsed.suffix)
                        .informed_by(informed_by)
                        .build(),
                ],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        ))
    }
}

#[derive(serde::Deserialize)]
struct ParsedProposal {
    suffix: String,
}

struct ConstantEvaluator;

impl Evaluator<TestProblem> for ConstantEvaluator {
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
                    evidence: TestEvidence,
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

fn graph_and_budget() -> (
    leaven_engine::RunGraph<TestProblem>,
    leaven_engine::BudgetLedger,
) {
    (
        leaven_engine::RunGraph::new(leaven_kernel::RunId::new()),
        leaven_engine::BudgetLedger::new(Budget::unlimited()),
    )
}

fn content_id(text: &str) -> ContentId {
    let mut bytes = [0; 32];
    let raw = text.as_bytes();
    bytes[..raw.len().min(32)].copy_from_slice(&raw[..raw.len().min(32)]);
    ContentId::from_bytes(bytes)
}
