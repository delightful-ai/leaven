use futures::executor::block_on;
use leaven_agent::{AgentSession, FakeAgentAction, FakeAgentRuntime};
use leaven_core::{
    Artifact, ArtifactIdentity, CacheIdentity, Evidence, OptimizationProblem, Proposal,
    ProposalBatch, ProposalBatchSemantics,
};
use leaven_engine::{Arity, Proposer, RunContext, RunEvent};
use leaven_kernel::{Budget, CandidateId, ContentId, Cost, MetadataBag, Metered, StageRole};
use leaven_stage::{
    AgentBacked, AgentStageBootstrap, AgentStageCallContext, AgentStagePlan, AllowedQuerySet,
    ProposerSlot, StageDirective, StageOutputContract, StageOutputParseError, StageOutputParser,
    StageQuery, StageQueryKind, StageQueryPolicy,
};
use leaven_workspace::{WorkspaceBackend, WorkspaceError, WorkspacePath, WorkspaceView};
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

#[test]
fn agent_backed_exposes_stage_identity_and_surfaces_runtime_errors() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let parent = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap()
        };
        let proposer = AgentBacked::<ProposerSlot<ReflectRequest>, _, _, _>::from_factory(
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(Vec::new()),
            ReflectBootstrap,
            JsonProposalParser,
            leaven_stage::AgentBackedPolicy {
                runtime_timeout: Some(std::time::Duration::from_millis(10)),
                ..Default::default()
            },
        );

        assert_eq!(proposer.id().as_str(), "agent-backed");
        assert_eq!(proposer.arity(), Arity::Single);

        let err = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.propose(&proposer, ReflectRequest { parent })
                .await
                .unwrap_err()
        };

        assert!(err.to_string().contains("agent stage runtime failed"));
    });
}

#[test]
fn agent_backed_rejects_invalid_output_contract_before_workspace_allocation() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let parent = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap()
        };
        let proposer = AgentBacked::<ProposerSlot<ReflectRequest>, _, _, _>::from_factory(
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(Vec::new()),
            InvalidOutputBootstrap,
            JsonProposalParser,
            Default::default(),
        );

        let err = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.propose(&proposer, ReflectRequest { parent })
                .await
                .unwrap_err()
        };

        assert!(
            err.to_string()
                .contains("agent stage output contract invalid")
        );
    });
}

#[test]
fn agent_backed_surfaces_serialization_allocation_and_parse_failures() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let parent = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap()
        };
        let serialization = AgentBacked::<ProposerSlot<BadRequest>, _, _, _>::from_factory(
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(Vec::new()),
            BadRequestBootstrap,
            BadRequestParser,
            Default::default(),
        );
        let err = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.propose(&serialization, BadRequest).await.unwrap_err()
        };
        assert!(
            err.to_string()
                .contains("agent stage plan serialization failed")
        );

        let allocation = AgentBacked::<ProposerSlot<ReflectRequest>, _, _, _>::from_factory(
            FailingFactory,
            FakeAgentRuntime::new(Vec::new()),
            ReflectBootstrap,
            JsonProposalParser,
            Default::default(),
        );
        let err = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.propose(&allocation, ReflectRequest { parent })
                .await
                .unwrap_err()
        };
        assert!(
            err.to_string()
                .contains("agent stage workspace allocation failed")
        );

        let setup = AgentBacked::<ProposerSlot<ReflectRequest>, _, _, _>::from_factory(
            SetupFailingFactory,
            FakeAgentRuntime::new(Vec::new()),
            ReflectBootstrap,
            JsonProposalParser,
            Default::default(),
        );
        let err = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.propose(&setup, ReflectRequest { parent })
                .await
                .unwrap_err()
        };
        assert!(
            err.to_string()
                .contains("agent stage workspace setup failed")
        );

        let parse = AgentBacked::<ProposerSlot<ReflectRequest>, _, _, _>::from_factory(
            LocalWorkspaceFactory::temp(),
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/proposal.json").unwrap(),
                bytes: br#"{"suffix": 1}"#.to_vec(),
            }]),
            ReflectBootstrap,
            JsonProposalParser,
            Default::default(),
        );
        let err = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.propose(&parse, ReflectRequest { parent })
                .await
                .unwrap_err()
        };
        assert!(err.to_string().contains("agent stage output parse failed"));
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
        let mut directive = StageDirective::new("Reflect", "Write a proposal JSON file.");
        directive
            .success_criteria
            .push("output/proposal.json contains a string suffix".to_owned());
        let plan = AgentStagePlan::new(
            StageRole::reflect(),
            request,
            directive,
            StageOutputContract::proposal_json(WorkspacePath::new("output/proposal.json").unwrap()),
        )
        .with_query_policy(StageQueryPolicy::bounded(
            AllowedQuerySet::only([StageQueryKind::Help]),
            vec![StageQuery::Help],
            Some(1),
            Some(4096),
        ));
        Ok(plan)
    }
}

struct InvalidOutputBootstrap;

impl AgentStageBootstrap<TestProblem, ProposerSlot<ReflectRequest>> for InvalidOutputBootstrap {
    async fn plan(
        &self,
        request: ReflectRequest,
        _ctx: AgentStageCallContext,
    ) -> Result<AgentStagePlan<ReflectRequest>, leaven_stage::StageBootstrapError> {
        Ok(AgentStagePlan::new(
            StageRole::reflect(),
            request,
            StageDirective::new("Reflect", "Write a proposal JSON file."),
            StageOutputContract::proposal_json(
                WorkspacePath::new("scratch/proposal.json").unwrap(),
            ),
        ))
    }
}

struct BadRequest;

impl serde::Serialize for BadRequest {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom("bad request"))
    }
}

struct BadRequestBootstrap;

impl AgentStageBootstrap<TestProblem, ProposerSlot<BadRequest>> for BadRequestBootstrap {
    async fn plan(
        &self,
        request: BadRequest,
        _ctx: AgentStageCallContext,
    ) -> Result<AgentStagePlan<BadRequest>, leaven_stage::StageBootstrapError> {
        Ok(AgentStagePlan::new(
            StageRole::reflect(),
            request,
            StageDirective::new("Reflect", "Write a proposal JSON file."),
            StageOutputContract::proposal_json(WorkspacePath::new("output/proposal.json").unwrap()),
        ))
    }
}

struct BadRequestParser;

impl StageOutputParser<TestProblem, ProposerSlot<BadRequest>> for BadRequestParser {
    async fn parse(
        &self,
        _workspace: &mut WorkspaceView<'_>,
        _session: &AgentSession,
        _plan: &leaven_stage::parser::ErasedStagePlan,
        _ctx: AgentStageCallContext,
    ) -> Result<Metered<ProposalBatch<TestProblem>>, StageOutputParseError> {
        unreachable!("bad request serialization fails before parsing")
    }
}

struct FailingFactory;

impl leaven_workspace::WorkspaceFactory for FailingFactory {
    async fn allocate(
        &self,
        _config: leaven_workspace::WorkspaceConfig,
    ) -> Result<leaven_workspace::Workspace, leaven_workspace::FactoryError> {
        Err(leaven_workspace::FactoryError::Allocate(
            "no workspace".to_owned(),
        ))
    }
}

struct SetupFailingFactory;

impl leaven_workspace::WorkspaceFactory for SetupFailingFactory {
    async fn allocate(
        &self,
        _config: leaven_workspace::WorkspaceConfig,
    ) -> Result<leaven_workspace::Workspace, leaven_workspace::FactoryError> {
        Ok(leaven_workspace::Workspace::new(
            std::env::temp_dir().join("leaven-stage-setup-failing-workspace"),
            Box::new(UnsupportedBackend),
        ))
    }
}

struct UnsupportedBackend;

impl WorkspaceBackend for UnsupportedBackend {
    fn cleanup(self: Box<Self>) -> futures::future::BoxFuture<'static, Result<(), WorkspaceError>> {
        Box::pin(async { Ok(()) })
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
            Self::Success { candidate_id } => Some(*candidate_id),
            Self::Failure { .. } => None,
        }
    }
}
