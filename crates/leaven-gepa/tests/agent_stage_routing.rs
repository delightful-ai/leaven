use futures::executor::block_on;
use leaven_agent::{AgentSession, FakeAgentAction, FakeAgentRuntime};
use leaven_core::{
    Artifact, ArtifactIdentity, CacheIdentity, Evidence, OptimizationProblem, Proposal,
    ProposalBatch, ProposalBatchSemantics,
};
use leaven_engine::{RunContext, RunEvent};
use leaven_gepa::{GepaReflectionBootstrap, GepaReflectionRequest, gepa_stage_proposer};
use leaven_kernel::{
    AssessmentId, Budget, BudgetSnapshot, CandidateId, ContentId, Cost, MetadataBag, Metered,
    StageAttemptOutcome, StageCallId, StageRole,
};
use leaven_stage::{
    AgentStageBootstrap, AgentStageCallContext, ProposerSlot, StageOutputParseError,
    StageOutputParser, StageQuery, StageQueryKind, receipt_store::StageReceiptStore,
};
use leaven_workspace::{WorkspacePath, WorkspaceView};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn gepa_reflection_bootstrap_prewarms_parent_and_feedback_queries() {
    block_on(async {
        let parent = CandidateId::new();
        let feedback = AssessmentId::new();
        let request = GepaReflectionRequest::new(parent, "answer").with_feedback([feedback]);
        let ctx = AgentStageCallContext::new(
            StageCallId::new(),
            leaven_engine::ReadScope::default(),
            BudgetSnapshot::default(),
        );

        let plan = <GepaReflectionBootstrap as AgentStageBootstrap<
            TestProblem,
            ProposerSlot<GepaReflectionRequest>,
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
            ctx.propose(&proposer, GepaReflectionRequest::new(parent, "root"))
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
    });
}

struct JsonProposalParser;

impl StageOutputParser<TestProblem, ProposerSlot<GepaReflectionRequest>> for JsonProposalParser {
    async fn parse(
        &self,
        workspace: &mut WorkspaceView<'_>,
        _session: &AgentSession,
        plan: &leaven_stage::parser::ErasedStagePlan,
        _ctx: AgentStageCallContext,
    ) -> Result<Metered<ProposalBatch<TestProblem>>, StageOutputParseError> {
        let bytes = workspace.read_file(&WorkspacePath::new("output/proposal.json").unwrap())?;
        let parsed: ParsedProposal = serde_json::from_slice(&bytes)?;
        let request: GepaReflectionRequest = serde_json::from_value(plan.request_json.clone())?;
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![Proposal::mutate(request.parent, parsed.suffix).build()],
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
