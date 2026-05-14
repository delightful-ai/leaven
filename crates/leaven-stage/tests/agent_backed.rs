use futures::executor::block_on;
use leaven_agent::{AgentSession, FakeAgentAction, FakeAgentRuntime};
use leaven_core::{
    Artifact, ArtifactIdentity, CacheIdentity, Evidence, OptimizationProblem, Proposal,
    ProposalBatch, ProposalBatchSemantics,
};
use leaven_engine::{RunContext, RunEvent};
use leaven_kernel::{Budget, CandidateId, ContentId, Cost, MetadataBag, Metered, StageRole};
use leaven_stage::{
    AgentBacked, AgentStageBootstrap, AgentStageCallContext, AgentStagePlan, ProposerSlot,
    StageDirective, StageOutputContract, StageOutputParseError, StageOutputParser,
};
use leaven_workspace::{WorkspacePath, WorkspaceView};
use leaven_workspace_local::LocalWorkspaceFactory;
use serde::Deserialize;

#[test]
fn agent_backed_fake_runtime_records_receipt_and_applies_candidate() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let parent = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap()
        };
        let proposer = AgentBacked::<ProposerSlot<ReflectRequest>, _, _, _>::from_factory(
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/proposal.json").unwrap(),
                bytes: br#"{"suffix":"-agent"}"#.to_vec(),
            }]),
            ReflectBootstrap,
            JsonProposalParser,
            Default::default(),
        );

        let report = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.propose(&proposer, ReflectRequest { parent })
                .await
                .unwrap()
        };
        let apply = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.apply_batch(report.batch_id).unwrap()
        };
        let candidate = apply.outcomes[0].outcome.candidate_id().unwrap();
        let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);

        assert_eq!(ctx.graph().artifact(candidate).unwrap().0, "seed-agent");
        assert!(ctx.graph().events().any(|event| {
            matches!(
                event,
                RunEvent::StageAttemptRecorded {
                    role,
                    outcome: leaven_kernel::StageAttemptOutcome::Completed,
                    ..
                } if role == &StageRole::reflect()
            )
        }));
    });
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
struct ReflectRequest {
    parent: CandidateId,
}

struct ReflectBootstrap;

impl AgentStageBootstrap<TestProblem, ProposerSlot<ReflectRequest>> for ReflectBootstrap {
    async fn plan(
        &self,
        request: ReflectRequest,
        _ctx: AgentStageCallContext,
    ) -> Result<AgentStagePlan<ReflectRequest>, leaven_stage::StageBootstrapError> {
        Ok(AgentStagePlan::new(
            StageRole::reflect(),
            request,
            StageDirective::new("Reflect", "Write a proposal JSON file."),
            StageOutputContract::proposal_json(WorkspacePath::new("output/proposal.json").unwrap()),
        ))
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

#[derive(Deserialize)]
struct ParsedProposal {
    suffix: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextArtifact(String);

impl Artifact for TextArtifact {
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
    type Artifact = TextArtifact;
    type Case = ();
    type Evidence = TestEvidence;
    type ProposalAnnotations = ();
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

trait CandidateOutcome {
    fn candidate_id(&self) -> Option<CandidateId>;
}

impl CandidateOutcome for leaven_engine::ApplyOutcome {
    fn candidate_id(&self) -> Option<CandidateId> {
        match self {
            leaven_engine::ApplyOutcome::Success { candidate_id } => Some(*candidate_id),
            leaven_engine::ApplyOutcome::Failure { .. } => None,
        }
    }
}
