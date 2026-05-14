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
use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence};
use leaven_gepa::{
    FixedSurfaceEdit, Gepa, GepaReflectionBootstrap, GepaReflector, ReflectRequest,
    SelectedFeedback, gepa_stage_proposer,
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
        let feedback = AssessmentId::new();
        let request = ReflectRequest::new(parent, "answer").with_feedback([feedback]);
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
        assert_eq!(
            plan.query.prewarm[1],
            StageQuery::Assessment { id: feedback }
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

impl leaven_gepa::GepaScoreEvidence for TestEvidence {
    fn scalar_casewise(&self) -> CasewiseEvidence<ScalarEvidence> {
        CasewiseEvidence::new(vec![CaseOutcome::new(
            leaven_kernel::CaseId::new(0),
            ScalarEvidence::new(1.0).unwrap(),
        )])
    }
}

struct TestProblem;

impl OptimizationProblem for TestProblem {
    type Artifact = TestArtifact;
    type Case = ();
    type Evidence = TestEvidence;
    type ProposalAnnotations = ();
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
                ReflectRequest::new(parent, "root").with_selected_feedback(SelectedFeedback {
                    assessment_refs: vec![AssessmentId::new()],
                    evidence_refs: Vec::new(),
                    candidate_refs: vec![parent],
                }),
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
                .informed_by
                .contains(&leaven_core::InfoRef::Candidate(parent))
        );
    });
}

#[test]
fn gepa_optimizer_uses_agent_backed_reflection_path() {
    block_on(async {
        let case_set = CaseSet::new(vec![()]).with_partition(
            leaven_core::PartitionId::from("TRAIN"),
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
        );

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
                parent,
                parent_assessment,
                "text",
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
                .informed_by
                .contains(&leaven_core::InfoRef::Candidate(parent))
        );
        assert!(
            proposal
                .provenance()
                .informed_by
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
                missing_parent,
                AssessmentId::new(),
                "text",
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
                parent,
                AssessmentId::new(),
                "missing",
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
                capped_parent,
                AssessmentId::new(),
                "text",
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("GEPA proposal recording failed"));
    });
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
        let informed_by = request.selected_feedback.source_refs();
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
                    evidence: TestEvidence,
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
