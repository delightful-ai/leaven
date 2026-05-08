use std::error::Error;
use std::num::NonZeroUsize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, AgentRuntimeCapabilities,
    AgentRuntimeError, AgentSession,
};
use leaven_agentic::{
    AgentPromptTarget, AgenticParseError, AgenticRepairError, AgenticRunInput, ProposalParser,
    ProposalRepairFeedback, ProposalRepairPromptBuilder, RepairingAgenticProposer,
    RepairingAgenticProposerConfig,
};
use leaven_core::{
    Artifact, ArtifactIdentity, Evidence, OptimizationProblem, Proposal, ProposalBatch,
    ProposalBatchSemantics,
};
use leaven_engine::{
    BudgetLedger, MaterializationReport, MaterializeContext, MaterializeError, Materializer,
    RenderContext, RenderError, Renderer, RunContext, RunGraph,
};
use leaven_kernel::{
    AgentRuntimeId, ContentId, Cost, Fingerprint, MetadataBag, Metered, ProposerId, RunId,
};
use leaven_workspace::{WorkspacePath, WorkspaceView};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn repairing_proposer_routes_parse_failure_back_to_same_runtime_loop() {
    futures::executor::block_on(async {
        let runtime_calls = Arc::new(AtomicUsize::new(0));
        let runtime_tasks = Arc::new(Mutex::new(Vec::new()));
        let repair_errors = Arc::new(Mutex::new(Vec::new()));
        let proposer = RepairingAgenticProposer::new(
            RepairingAgenticProposerConfig::new(
                ProposerId::from("agentic/repair"),
                NonZeroUsize::new(2).unwrap(),
            ),
            LocalWorkspaceFactory::temp(),
            TwoAttemptRuntime {
                calls: runtime_calls.clone(),
                tasks: runtime_tasks.clone(),
            },
            TestMaterializer,
            TestRenderer,
            RecordingRepairPrompt {
                errors: repair_errors.clone(),
            },
            ProposalFileParser,
        );
        let mut graph = RunGraph::<TestProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);

        let report = ctx
            .propose(
                &proposer,
                AgenticRunInput::new(
                    ProposalInput,
                    leaven_agent::OutputContract::Files {
                        paths: vec![WorkspacePath::new("output/proposal.txt").unwrap()],
                    },
                ),
            )
            .await
            .unwrap();

        assert_eq!(runtime_calls.load(Ordering::SeqCst), 2);
        assert_eq!(
            runtime_tasks.lock().unwrap().as_slice(),
            ["write a valid proposal", "repair the proposal"]
        );
        assert_eq!(repair_errors.lock().unwrap().as_slice(), ["bad proposal"]);
        assert_eq!(report.cost.llm_calls, 2);
        let applied = ctx.apply_batch(report.batch_id).unwrap();
        let child = applied.successful_candidates().next().unwrap();
        assert_eq!(ctx.graph().artifact(child).unwrap().0, "fixed");
    });
}

#[test]
fn repairing_proposer_exhausts_after_bounded_attempts() {
    futures::executor::block_on(async {
        let runtime_calls = Arc::new(AtomicUsize::new(0));
        let proposer = RepairingAgenticProposer::new(
            RepairingAgenticProposerConfig::new(
                ProposerId::from("agentic/repair-exhausted"),
                NonZeroUsize::new(2).unwrap(),
            ),
            LocalWorkspaceFactory::temp(),
            AlwaysInvalidRuntime {
                calls: runtime_calls.clone(),
            },
            TestMaterializer,
            TestRenderer,
            StaticRepairPrompt,
            ProposalFileParser,
        );
        let mut graph = RunGraph::<TestProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);

        let error = ctx
            .propose(
                &proposer,
                AgenticRunInput::new(
                    ProposalInput,
                    leaven_agent::OutputContract::Files {
                        paths: vec![WorkspacePath::new("output/proposal.txt").unwrap()],
                    },
                ),
            )
            .await
            .unwrap_err();

        assert_eq!(runtime_calls.load(Ordering::SeqCst), 2);
        assert!(error_chain_contains(&error, "proposal repair exhausted"));
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TestArtifact(String);

impl Artifact for TestArtifact {
    type Change = String;
    type ApplyError = TestApplyError;

    fn identity(&self) -> ArtifactIdentity {
        let byte = u8::try_from(self.0.len()).unwrap_or(u8::MAX);
        ArtifactIdentity::Content(ContentId::from_bytes([byte; 32]))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(change.clone()))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("test apply failed")]
struct TestApplyError;

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

#[derive(Clone, Debug)]
struct ProposalInput;

struct TestMaterializer;

impl Materializer<TestProblem, ProposalInput> for TestMaterializer {
    async fn materialize_into(
        &self,
        _value: &ProposalInput,
        workspace: &mut WorkspaceView<'_>,
        _ctx: MaterializeContext<'_, TestProblem>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        workspace.write_file(&WorkspacePath::new("input.txt")?, b"input")?;
        Ok(Metered::new(
            MaterializationReport {
                files_written: 1,
                bytes_written: 5,
                truncations: Vec::new(),
            },
            Cost::zero(),
        ))
    }
}

struct TestRenderer;

impl Renderer<TestProblem, ProposalInput, AgentPromptTarget> for TestRenderer {
    type View = AgentInstructions;

    async fn render(
        &self,
        _value: &ProposalInput,
        _target: AgentPromptTarget,
        _ctx: RenderContext<'_, TestProblem>,
    ) -> Result<Metered<Self::View>, RenderError> {
        Ok(Metered::new(
            AgentInstructions::task("write a valid proposal"),
            Cost::zero(),
        ))
    }
}

struct ProposalFileParser;

impl ProposalParser<TestProblem, ProposalInput> for ProposalFileParser {
    async fn parse_proposals(
        &self,
        workspace: &mut WorkspaceView<'_>,
        _session: &AgentSession,
        _input: &ProposalInput,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Result<Metered<ProposalBatch<TestProblem>>, AgenticParseError> {
        let bytes = workspace.read_file(&WorkspacePath::new("output/proposal.txt").unwrap())?;
        let text = String::from_utf8(bytes).unwrap();
        if text != "fixed" {
            return Err(AgenticParseError::Message("bad proposal".to_owned()));
        }
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![Proposal::create(TestArtifact(text)).build()],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        ))
    }
}

struct RecordingRepairPrompt {
    errors: Arc<Mutex<Vec<String>>>,
}

impl ProposalRepairPromptBuilder<ProposalInput> for RecordingRepairPrompt {
    fn build_repair(
        &self,
        _input: &ProposalInput,
        feedback: ProposalRepairFeedback<'_>,
    ) -> Result<AgentInstructions, AgenticRepairError> {
        assert_eq!(feedback.failed_attempt.get(), 1);
        assert_eq!(feedback.max_attempts.get(), 2);
        assert!(!feedback.previous_session.transcript.events.is_empty());
        self.errors.lock().unwrap().push(
            feedback
                .parse_error
                .to_string()
                .replace("agentic parse failed: ", ""),
        );
        Ok(AgentInstructions::task("repair the proposal"))
    }
}

struct StaticRepairPrompt;

impl ProposalRepairPromptBuilder<ProposalInput> for StaticRepairPrompt {
    fn build_repair(
        &self,
        _input: &ProposalInput,
        _feedback: ProposalRepairFeedback<'_>,
    ) -> Result<AgentInstructions, AgenticRepairError> {
        Ok(AgentInstructions::task("try again"))
    }
}

struct TwoAttemptRuntime {
    calls: Arc<AtomicUsize>,
    tasks: Arc<Mutex<Vec<String>>>,
}

impl AgentRuntime for TwoAttemptRuntime {
    fn id(&self) -> AgentRuntimeId {
        AgentRuntimeId::new_const("two-attempt")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([2; 32])
    }

    fn capabilities(&self) -> AgentRuntimeCapabilities {
        AgentRuntimeCapabilities::default()
    }

    async fn run_session(
        &self,
        workspace: &mut WorkspaceView<'_>,
        request: AgentRunRequest,
        ctx: AgentRunContext<'_>,
    ) -> Result<Metered<AgentSession>, AgentRuntimeError> {
        let attempt = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let mut session = AgentSession::succeeded(ctx.session_id());
        self.tasks
            .lock()
            .unwrap()
            .push(request.instructions.task.clone());
        session.transcript.push_message(
            leaven_agent::TranscriptRole::User,
            request.instructions.task,
        );
        let bytes = if attempt == 1 {
            b"invalid".as_slice()
        } else {
            b"fixed".as_slice()
        };
        workspace.write_file(&WorkspacePath::new("output/proposal.txt").unwrap(), bytes)?;
        Ok(Metered::new(session, Cost::llm_calls(1)))
    }
}

fn error_chain_contains(error: &(dyn Error + 'static), needle: &str) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if error.to_string().contains(needle) {
            return true;
        }
        current = error.source();
    }
    false
}

struct AlwaysInvalidRuntime {
    calls: Arc<AtomicUsize>,
}

impl AgentRuntime for AlwaysInvalidRuntime {
    fn id(&self) -> AgentRuntimeId {
        AgentRuntimeId::new_const("always-invalid")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([3; 32])
    }

    async fn run_session(
        &self,
        workspace: &mut WorkspaceView<'_>,
        request: AgentRunRequest,
        ctx: AgentRunContext<'_>,
    ) -> Result<Metered<AgentSession>, AgentRuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut session = AgentSession::succeeded(ctx.session_id());
        session.transcript.push_message(
            leaven_agent::TranscriptRole::User,
            request.instructions.task,
        );
        workspace.write_file(
            &WorkspacePath::new("output/proposal.txt").unwrap(),
            b"still invalid",
        )?;
        Ok(Metered::new(session, Cost::llm_calls(1)))
    }
}
