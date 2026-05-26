use std::collections::BTreeMap;

use futures::executor::block_on;
use leaven_agent::{
    AgentRunContext, AgentRunRequest, AgentRuntime, AgentRuntimeError, AgentSession,
    FakeAgentAction, FakeAgentRuntime,
};
use leaven_core::{Proposal, ProposalBatch, ProposalBatchSemantics};
use leaven_engine::{Arity, Proposer, RunContext, RunEvent};
use leaven_kernel::{
    CandidateId, Cost, MetadataBag, Metered, StageAttemptFailure, StageAttemptOutcome,
    StageAttemptReceiptRef, StageRole, WorkspaceId,
};
use leaven_stage::{
    AgentBacked, AgentStageBootstrap, AgentStageCallContext, AgentStagePlan, AllowedQuerySet,
    OutputEntryStatus, ParseStatus, ProposerSlot, SlotMarker, StageAttemptReceiptBuilder,
    StageDirective, StageOutputContract, StageOutputParseError, StageOutputParser, StageQuery,
    StageQueryKind, StageQueryPolicy, receipt_store::StageReceiptStore,
};
use leaven_workspace::{WorkspaceBackend, WorkspaceError, WorkspacePath, WorkspaceView};
use leaven_workspace_local::LocalWorkspaceFactory;
use serde::Deserialize;

use crate::support::{TestProblem, TextArtifact, graph_and_budget};

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
        let receipt_ref = stage_receipt_ref(&ctx, &StageAttemptOutcome::Completed);
        let receipt = proposer
            .receipt_store()
            .read(receipt_ref.id)
            .await
            .unwrap()
            .expect("stage receipt is persisted");
        assert_eq!(receipt.outputs.len(), 1);
        assert_eq!(receipt.outputs[0].status, OutputEntryStatus::Present);
        assert_eq!(
            receipt.outputs[0].path,
            WorkspacePath::new("output/proposal.json").unwrap()
        );
        assert_eq!(
            receipt.parse.as_ref().unwrap().status,
            ParseStatus::Succeeded
        );
        assert_eq!(
            receipt.parse.as_ref().unwrap().files_read,
            vec![WorkspacePath::new("output/proposal.json").unwrap()]
        );
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

        assert!(
            err.to_string().contains("agent stage runtime failed"),
            "{err:?} / {err}"
        );
        let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let receipt_ref = stage_receipt_ref(
            &ctx,
            &StageAttemptOutcome::Failed(StageAttemptFailure::Runtime),
        );
        let receipt = proposer
            .receipt_store()
            .read(receipt_ref.id)
            .await
            .unwrap()
            .expect("runtime failure receipt is persisted");
        assert_eq!(
            receipt.outcome,
            StageAttemptOutcome::Failed(StageAttemptFailure::Runtime)
        );
        assert!(receipt.parse.is_none());
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
        assert!(
            err.to_string().contains("agent stage output parse failed"),
            "{err:?} / {err}"
        );
        let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let receipt_ref = stage_receipt_ref(
            &ctx,
            &StageAttemptOutcome::Failed(StageAttemptFailure::OutputParse),
        );
        let receipt = parse
            .receipt_store()
            .read(receipt_ref.id)
            .await
            .unwrap()
            .expect("parse failure receipt is persisted");
        assert_eq!(
            receipt.outcome,
            StageAttemptOutcome::Failed(StageAttemptFailure::OutputParse)
        );
        assert_eq!(receipt.outputs.len(), 1);
        assert_eq!(receipt.outputs[0].status, OutputEntryStatus::Present);
        assert_eq!(receipt.parse.as_ref().unwrap().status, ParseStatus::Failed);
    });
}

#[test]
fn agent_backed_records_missing_outputs_before_parse_failure() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let parent = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap()
        };
        let proposer = AgentBacked::<ProposerSlot<ReflectRequest>, _, _, _>::from_factory(
            LocalWorkspaceFactory::temp(),
            MissingOutputRuntime,
            ReflectBootstrap,
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
            err.to_string().contains("agent stage output parse failed"),
            "{err:?} / {err}"
        );

        let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let receipt_ref = stage_receipt_ref(
            &ctx,
            &StageAttemptOutcome::Failed(StageAttemptFailure::OutputParse),
        );
        let receipt = proposer
            .receipt_store()
            .read(receipt_ref.id)
            .await
            .unwrap()
            .expect("parse failure receipt is persisted");
        assert_eq!(receipt.outputs.len(), 1);
        assert_eq!(receipt.outputs[0].status, OutputEntryStatus::Missing);
        assert!(receipt.outputs[0].fingerprint.is_none());
        assert_eq!(receipt.parse.as_ref().unwrap().status, ParseStatus::Failed);
    });
}

#[test]
fn agent_backed_records_workspace_setup_failure_receipt() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let parent = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap()
        };
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
        let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let receipt_ref = stage_receipt_ref(
            &ctx,
            &StageAttemptOutcome::Failed(StageAttemptFailure::WorkspaceSetup),
        );
        let receipt = setup
            .receipt_store()
            .read(receipt_ref.id)
            .await
            .unwrap()
            .expect("setup failure receipt is persisted");
        assert_eq!(
            receipt.outcome,
            StageAttemptOutcome::Failed(StageAttemptFailure::WorkspaceSetup)
        );
    });
}

#[test]
fn agent_backed_records_prewarm_query_failure_receipt() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let parent = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap()
        };
        let proposer = AgentBacked::<ProposerSlot<ReflectRequest>, _, _, _>::from_factory(
            QueryWriteFailingFactory,
            FakeAgentRuntime::new(Vec::new()),
            ReflectBootstrap,
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
            err.to_string().contains("agent stage prewarm query failed"),
            "{err:?} / {err}"
        );
        let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        let receipt_ref = stage_receipt_ref(
            &ctx,
            &StageAttemptOutcome::Failed(StageAttemptFailure::Query),
        );
        let receipt = proposer
            .receipt_store()
            .read(receipt_ref.id)
            .await
            .unwrap()
            .expect("query failure receipt is persisted");
        assert_eq!(
            receipt.outcome,
            StageAttemptOutcome::Failed(StageAttemptFailure::Query)
        );
        assert_eq!(receipt.setup.plan_entries.len(), 9);
        assert!(receipt.queries.is_empty());
    });
}

#[test]
fn agent_backed_surfaces_cleanup_failure_after_success_and_parse_error() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let parent = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap()
        };
        let success = AgentBacked::<ProposerSlot<ReflectRequest>, _, _, _>::from_factory(
            CleanupFailingFactory,
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/proposal.json").unwrap(),
                bytes: br#"{"suffix":"-agent"}"#.to_vec(),
            }]),
            ReflectBootstrap,
            JsonProposalParser,
            Default::default(),
        );
        let err = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.propose(&success, ReflectRequest { parent })
                .await
                .unwrap_err()
        };
        assert!(
            err.to_string()
                .contains("agent stage workspace cleanup failed"),
            "{err:?} / {err}"
        );

        let parse = AgentBacked::<ProposerSlot<ReflectRequest>, _, _, _>::from_factory(
            CleanupFailingFactory,
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
        assert!(
            err.to_string()
                .contains("agent stage workspace cleanup failed"),
            "{err:?} / {err}"
        );
    });
}

#[test]
fn proposer_slot_role_and_inline_receipt_store_are_public_stage_contracts() {
    block_on(async {
        assert_eq!(
            <ProposerSlot<ReflectRequest> as SlotMarker<TestProblem>>::role(),
            StageRole::reflect()
        );
        let store = leaven_stage::receipt_store::InlineReceiptStore::default();
        let receipt = StageAttemptReceiptBuilder::new(
            WorkspaceId::new(),
            leaven_kernel::StageCallId::new(),
            StageRole::reflect(),
            leaven_kernel::Fingerprint::from_bytes([9; 32]),
        )
        .finish(StageAttemptOutcome::Completed);
        let receipt_id = receipt.receipt_id;
        let receipt_ref = store.write(receipt).await.unwrap();

        assert_eq!(receipt_ref.id, receipt_id);
        assert!(receipt_ref.fingerprint.is_some());
        assert_eq!(
            store.read(receipt_id).await.unwrap().unwrap().outcome,
            StageAttemptOutcome::Completed
        );
        assert!(
            store
                .read(leaven_kernel::StageAttemptReceiptId::new())
                .await
                .unwrap()
                .is_none()
        );
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

struct QueryWriteFailingFactory;

impl leaven_workspace::WorkspaceFactory for QueryWriteFailingFactory {
    async fn allocate(
        &self,
        _config: leaven_workspace::WorkspaceConfig,
    ) -> Result<leaven_workspace::Workspace, leaven_workspace::FactoryError> {
        Ok(leaven_workspace::Workspace::new(
            std::env::temp_dir().join("leaven-stage-query-write-failing-workspace"),
            Box::new(ControllableBackend {
                fail_query_writes: true,
                fail_cleanup: false,
                ..Default::default()
            }),
        ))
    }
}

#[derive(Default)]
struct ControllableBackend {
    fail_query_writes: bool,
    fail_cleanup: bool,
    files: BTreeMap<WorkspacePath, Vec<u8>>,
    executable: BTreeMap<WorkspacePath, bool>,
}

impl WorkspaceBackend for ControllableBackend {
    fn write_file(&mut self, path: &WorkspacePath, bytes: &[u8]) -> Result<(), WorkspaceError> {
        if self.fail_query_writes && path.starts_with_component("queries") {
            return Err(WorkspaceError::Io(format!(
                "query writes disabled for {}",
                path.as_str()
            )));
        }
        self.files.insert(path.clone(), bytes.to_vec());
        Ok(())
    }

    fn read_file(&mut self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| WorkspaceError::Io(format!("missing file {}", path.as_str())))
    }

    fn set_executable(
        &mut self,
        path: &WorkspacePath,
        executable: bool,
    ) -> Result<(), WorkspaceError> {
        self.executable.insert(path.clone(), executable);
        Ok(())
    }

    fn cleanup(self: Box<Self>) -> futures::future::BoxFuture<'static, Result<(), WorkspaceError>> {
        Box::pin(async move {
            if self.fail_cleanup {
                Err(WorkspaceError::Cleanup("cleanup disabled".to_owned()))
            } else {
                Ok(())
            }
        })
    }
}

struct CleanupFailingFactory;

impl leaven_workspace::WorkspaceFactory for CleanupFailingFactory {
    async fn allocate(
        &self,
        _config: leaven_workspace::WorkspaceConfig,
    ) -> Result<leaven_workspace::Workspace, leaven_workspace::FactoryError> {
        Ok(leaven_workspace::Workspace::new(
            std::env::temp_dir().join("leaven-stage-cleanup-failing-workspace"),
            Box::new(ControllableBackend {
                fail_cleanup: true,
                ..Default::default()
            }),
        ))
    }
}

struct MissingOutputRuntime;

impl AgentRuntime for MissingOutputRuntime {
    fn id(&self) -> leaven_kernel::AgentRuntimeId {
        leaven_kernel::AgentRuntimeId::new_const("missing-output")
    }

    fn fingerprint(&self) -> leaven_kernel::Fingerprint {
        leaven_kernel::Fingerprint::from_bytes([0x17; 32])
    }

    async fn run_session(
        &self,
        _workspace: &mut WorkspaceView<'_>,
        _request: AgentRunRequest,
        ctx: AgentRunContext<'_>,
    ) -> Result<Metered<AgentSession>, AgentRuntimeError> {
        Ok(Metered::new(
            AgentSession::succeeded(ctx.session_id()),
            Cost::zero(),
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

fn stage_receipt_ref(
    ctx: &RunContext<'_, TestProblem>,
    expected: &StageAttemptOutcome,
) -> StageAttemptReceiptRef {
    ctx.graph()
        .events()
        .find_map(|event| match event {
            RunEvent::StageAttemptRecorded {
                role,
                receipt,
                outcome,
                ..
            } if role == &StageRole::reflect() && outcome == expected => Some(receipt.clone()),
            _ => None,
        })
        .expect("stage attempt event recorded")
}
