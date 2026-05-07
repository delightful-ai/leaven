use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use futures::future::{BoxFuture, FutureExt};
use leaven_agent::{AgentInstructions, AgentSession, FakeAgentAction, FakeAgentRuntime};
use leaven_agentic::{
    AgentPromptTarget, AgenticEvaluator, AgenticEvaluatorConfig, AgenticProposer,
    AgenticProposerConfig, AgenticRunInput, EvaluationInputBuilder, EvidenceParser, ProposalParser,
};
use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, OptimizationProblem, Proposal,
    ProposalBatch, ProposalBatchSemantics, ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_engine::{
    BudgetLedger, CaseSet, MaterializationReport, MaterializeContext, MaterializeError,
    Materializer, RenderContext, RenderError, Renderer, RunContext, RunGraph,
};
use leaven_kernel::{
    CandidateId, ContentId, Cost, EvaluationSetId, EvaluatorId, Fingerprint, MetadataBag, Metered,
    ProposerId, RunId,
};
use leaven_store_inline::InlineEvidenceStore;
use leaven_workspace::{
    Command, CommandOutput, ExitStatus, FactoryError, Workspace, WorkspaceBackend, WorkspaceConfig,
    WorkspaceError, WorkspaceFactory, WorkspacePath,
};

#[test]
fn agentic_proposer_runs_runtime_parses_proposals_and_cleans_workspace() {
    futures::executor::block_on(async {
        let cleanup_count = Arc::new(AtomicUsize::new(0));
        let proposer = AgenticProposer::new(
            AgenticProposerConfig::new(ProposerId::from("agentic/test")),
            TestWorkspaceFactory {
                cleanup_count: cleanup_count.clone(),
            },
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/proposal.txt").unwrap(),
                bytes: b"agent-authored".to_vec(),
            }])
            .with_cost(Cost::llm_calls(1)),
            TestMaterializer,
            TestRenderer,
            ProposalFileParser,
        );
        let mut graph = RunGraph::<TestProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);

        let report = ctx
            .propose(
                &proposer,
                AgenticRunInput::new(
                    ProposalInput {
                        prompt: "author proposal".to_owned(),
                    },
                    OutputContract::file("output/proposal.txt"),
                ),
            )
            .await
            .unwrap();

        assert_eq!(report.cost.llm_calls, 1);
        assert_eq!(report.cost.metric_calls, 1);
        assert_eq!(report.cost.prompt_tokens, 3);
        assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
        let apply = ctx.apply_batch(report.batch_id).unwrap();
        let candidate = apply.successful_candidates().next().unwrap();
        assert_eq!(ctx.graph().artifact(candidate).unwrap().0, "agent-authored");
    });
}

#[test]
fn agentic_evaluator_runs_runtime_parses_evidence_and_cleans_workspace() {
    futures::executor::block_on(async {
        let cleanup_count = Arc::new(AtomicUsize::new(0));
        let evaluator = AgenticEvaluator::new(
            AgenticEvaluatorConfig::new(
                EvaluatorId::from("agentic/eval"),
                Fingerprint::from_bytes([7; 32]),
            ),
            TestWorkspaceFactory {
                cleanup_count: cleanup_count.clone(),
            },
            FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
                path: WorkspacePath::new("output/evidence.txt").unwrap(),
                bytes: b"0.75".to_vec(),
            }])
            .with_cost(Cost::llm_calls(2)),
            IndependentInputBuilder,
            TestMaterializer,
            TestRenderer,
            EvidenceFileParser,
        );
        let mut graph = RunGraph::<TestProblem>::new(RunId::new());
        let mut budget = BudgetLedger::default();
        let mut cache = leaven_engine::EvaluationCache::default();
        let case_set = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let candidate = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(TestArtifact("seed".to_owned()), 0).unwrap()
        };
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_cache(&mut cache)
            .with_evidence_store(&store);

        let report = ctx
            .evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .unwrap();

        assert_eq!(report.cost.llm_calls, 2);
        assert_eq!(report.cost.metric_calls, 1);
        assert_eq!(report.assessment_ids.len(), 1);
        assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
        let assessment = ctx.graph().assessment(report.assessment_ids[0]).unwrap();
        assert_eq!(assessment.evaluator(), &EvaluatorId::from("agentic/eval"));
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TestArtifact(String);

impl Artifact for TestArtifact {
    type Change = String;
    type ApplyError = TestApplyError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::from_bytes([self.0.len() as u8; 32]))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(change.clone()))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("test apply failed")]
struct TestApplyError;

#[derive(Clone, Debug)]
struct TestEvidence {
    score: f64,
}

impl leaven_core::Evidence for TestEvidence {}

struct TestProblem;

impl OptimizationProblem for TestProblem {
    type Artifact = TestArtifact;
    type Case = &'static str;
    type Evidence = TestEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug)]
struct ProposalInput {
    prompt: String,
}

#[derive(Clone, Debug)]
struct EvaluationInput {
    candidate: CandidateId,
    artifact: TestArtifact,
}

struct TestMaterializer;

impl<I: std::fmt::Debug + Send + Sync> Materializer<TestProblem, I> for TestMaterializer {
    async fn materialize_into(
        &self,
        value: &I,
        workspace: &mut leaven_workspace::WorkspaceView<'_>,
        _ctx: MaterializeContext<'_, TestProblem>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        let text = format!("{value:?}");
        workspace.write_file(&WorkspacePath::new("input.txt")?, text.as_bytes())?;
        Ok(Metered::new(
            MaterializationReport {
                files_written: 1,
                bytes_written: text.len() as u64,
                truncations: Vec::new(),
            },
            Cost::zero(),
        ))
    }
}

struct TestRenderer;

impl<I: std::fmt::Debug + Send + Sync> Renderer<TestProblem, I, AgentPromptTarget>
    for TestRenderer
{
    type View = AgentInstructions;

    async fn render(
        &self,
        value: &I,
        _target: AgentPromptTarget,
        _ctx: RenderContext<'_, TestProblem>,
    ) -> Result<Metered<Self::View>, RenderError> {
        Ok(Metered::new(
            AgentInstructions::task(format!("run on {value:?}")),
            Cost::tokens(3, 0),
        ))
    }
}

struct ProposalFileParser;

impl ProposalParser<TestProblem, ProposalInput> for ProposalFileParser {
    async fn parse_proposals(
        &self,
        workspace: &mut leaven_workspace::WorkspaceView<'_>,
        _session: &AgentSession,
        input: &ProposalInput,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Result<Metered<ProposalBatch<TestProblem>>, leaven_agentic::AgenticParseError> {
        assert_eq!(input.prompt, "author proposal");
        let bytes = workspace.read_file(&WorkspacePath::new("output/proposal.txt").unwrap())?;
        let artifact = TestArtifact(String::from_utf8(bytes).unwrap());
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![Proposal::create(artifact).build()],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::metric_calls(1),
        ))
    }
}

struct IndependentInputBuilder;

impl EvaluationInputBuilder<TestProblem, EvaluationInput> for IndependentInputBuilder {
    fn build_inputs(
        &self,
        request: &ResolvedEvaluationRequest,
        graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Result<Vec<AgenticRunInput<EvaluationInput>>, leaven_agentic::AgenticAdapterError> {
        let ResolvedRequestKind::Independent { candidates } = &request.kind else {
            return Err(leaven_agentic::AgenticAdapterError::Input(
                "expected independent request".to_owned(),
            ));
        };
        Ok(candidates
            .iter()
            .map(|candidate| {
                AgenticRunInput::new(
                    EvaluationInput {
                        candidate: *candidate,
                        artifact: graph.artifact(*candidate).unwrap().clone(),
                    },
                    OutputContract::file("output/evidence.txt"),
                )
            })
            .collect())
    }
}

struct EvidenceFileParser;

impl EvidenceParser<TestProblem, EvaluationInput> for EvidenceFileParser {
    async fn parse_evidence(
        &self,
        workspace: &mut leaven_workspace::WorkspaceView<'_>,
        _session: &AgentSession,
        input: &EvaluationInput,
        _request: &ResolvedEvaluationRequest,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, leaven_agentic::AgenticParseError> {
        assert_eq!(input.artifact.0, "seed");
        let bytes = workspace.read_file(&WorkspacePath::new("output/evidence.txt").unwrap())?;
        let score: f64 = String::from_utf8(bytes).unwrap().parse().unwrap();
        let evidence = TestEvidence { score };
        assert!(evidence.score > 0.0);
        Ok(Metered::new(
            vec![Assessment::Independent {
                candidate: input.candidate,
                target: AssessmentTarget::EvaluationSet(EvaluationSetId::new()),
                evidence,
                cost: Cost::metric_calls(1),
                metadata: MetadataBag::new(),
            }],
            Cost::metric_calls(1),
        ))
    }
}

struct TestWorkspaceFactory {
    cleanup_count: Arc<AtomicUsize>,
}

impl WorkspaceFactory for TestWorkspaceFactory {
    async fn allocate(&self, _config: WorkspaceConfig) -> Result<Workspace, FactoryError> {
        let root = temp_root("agentic-adapter");
        Ok(Workspace::new(
            root.clone(),
            Box::new(TestWorkspaceBackend {
                root,
                cleanup_count: self.cleanup_count.clone(),
            }),
        ))
    }
}

struct TestWorkspaceBackend {
    root: PathBuf,
    cleanup_count: Arc<AtomicUsize>,
}

impl WorkspaceBackend for TestWorkspaceBackend {
    fn write_file(&mut self, path: &WorkspacePath, bytes: &[u8]) -> Result<(), WorkspaceError> {
        let path = self.root.join(path.to_host_relative());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| WorkspaceError::Io(err.to_string()))?;
        }
        std::fs::write(path, bytes).map_err(|err| WorkspaceError::Io(err.to_string()))
    }

    fn read_file(&mut self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        std::fs::read(self.root.join(path.to_host_relative()))
            .map_err(|err| WorkspaceError::Io(err.to_string()))
    }

    fn run_command(&mut self, command: Command) -> Result<CommandOutput, WorkspaceError> {
        let _ = command;
        Ok(CommandOutput {
            status: ExitStatus { code: Some(0) },
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }

    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>> {
        async move {
            self.cleanup_count.fetch_add(1, Ordering::SeqCst);
            remove_dir(&self.root);
            Ok(())
        }
        .boxed()
    }

    fn local_mount(&self) -> Option<&Path> {
        Some(&self.root)
    }
}

struct OutputContract;

impl OutputContract {
    fn file(path: &str) -> leaven_agent::OutputContract {
        leaven_agent::OutputContract::Files {
            paths: vec![WorkspacePath::new(path).unwrap()],
        }
    }
}

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("leaven-{label}-{}", RunId::new()));
    remove_dir(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn remove_dir(path: &Path) {
    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}
