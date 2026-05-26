use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use futures::executor::block_on;
use futures::future::{BoxFuture, FutureExt};
use leaven_core::{
    Assessment, AssessmentGranularity, AssessmentTarget, EvaluationPurpose, EvaluationRequest,
    EvaluationSet, InfoRef, Proposal, ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_engine::{
    CachePolicy, CaseSet, EvaluationContext, EvaluationError, Evaluator, MaterializationReport,
    MaterializeContext, MaterializeError, Materializer, RunContext, TrustPolicy,
};
use leaven_kernel::{Cost, EvaluatorId, Fingerprint, MetadataBag, Metered, RunId, StageId};
use leaven_store_inline::InlineEvidenceStore;
use leaven_workspace::{Workspace, WorkspaceBackend, WorkspaceError, WorkspacePath, WorkspaceView};

use super::support::{
    TestEvidence, TestProblem, TextArtifact, graph_and_budget, record_one, text_artifact,
};

#[test]
fn materializer_writes_are_deterministic_for_same_graph_view_and_input() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        ctx.insert_seed(text_artifact("seed"), 0).unwrap();
        let materializer = DeterministicMaterializer;
        let value = text_artifact("artifact");
        let left_root = temp_root("materializer-left");
        let right_root = temp_root("materializer-right");
        let mut left_workspace = fs_workspace(left_root.clone());
        let mut right_workspace = fs_workspace(right_root.clone());
        let mut left = left_workspace.view();
        let mut right = right_workspace.view();

        let left_report = materializer
            .materialize_into(&value, &mut left, ctx.materialize_context())
            .await
            .unwrap();
        let right_report = materializer
            .materialize_into(&value, &mut right, ctx.materialize_context())
            .await
            .unwrap();

        assert_eq!(left_report.value.files_written, 2);
        assert_eq!(left_report.value, right_report.value);
        assert_eq!(
            left.read_file(&WorkspacePath::new("artifact.txt").unwrap())
                .unwrap(),
            right
                .read_file(&WorkspacePath::new("artifact.txt").unwrap())
                .unwrap()
        );
        assert_eq!(
            left.read_file(&WorkspacePath::new("history/candidates.txt").unwrap())
                .unwrap(),
            right
                .read_file(&WorkspacePath::new("history/candidates.txt").unwrap())
                .unwrap()
        );
        drop(left);
        drop(right);
        left_workspace.cleanup().await.unwrap();
        right_workspace.cleanup().await.unwrap();
        remove_dir(&left_root);
        remove_dir(&right_root);
    });
}

#[test]
fn materializer_invoked_from_proposer_receives_proposer_read_scope() {
    block_on(async {
        let (mut graph, mut budget) = graph_and_budget();
        let mut cache = leaven_engine::EvaluationCache::default();
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let hidden = leaven_core::PartitionId::from("TEST");
        let case_set = CaseSet::new(vec!["hidden"])
            .with_partition(hidden.clone(), vec![leaven_kernel::CaseId::new(0)]);
        let candidate = {
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(text_artifact("seed"), 0).unwrap()
        };
        let assessment = {
            let evaluator = TestEvaluator;
            let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_cache(&mut cache)
                .with_evidence_store(&store);
            ctx.evaluate_with(
                &evaluator,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::Partition(hidden.clone()),
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::FinalTest,
                },
            )
            .await
            .unwrap()
            .assessment_ids[0]
        };
        let materializer = VisibilityMaterializer {
            hidden: hidden.clone(),
            hidden_assessment: assessment,
            candidate,
        };
        let root = temp_root("materializer-visibility");
        let mut workspace = fs_workspace(root.clone());
        let mut view = workspace.view();
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_trust_policy(TrustPolicy::default().hide_from_proposers([hidden]));
        let proposal_ctx = ctx.proposal_context(StageId::custom("p4/proposer"));

        let report = materializer
            .materialize_into(
                &text_artifact("artifact"),
                &mut view,
                proposal_ctx.materialize_context(),
            )
            .await
            .unwrap();

        assert_eq!(report.value.files_written, 1);
        assert_eq!(
            view.read_file(&WorkspacePath::new("visible.txt").unwrap())
                .unwrap(),
            b"hidden assessment not visible"
        );
        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&root);
    });
}

#[test]
fn workspace_cleanup_is_explicit_and_called_once() {
    block_on(async {
        let cleanup_count = Arc::new(AtomicUsize::new(0));
        let workspace = Workspace::new(
            temp_root("cleanup-once"),
            Box::new(CountingBackend {
                cleanup_count: cleanup_count.clone(),
            }),
        );

        workspace.cleanup().await.unwrap();

        assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn create_none_with_informed_history_has_no_causal_parent_edges() {
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(text_artifact("seed"), 0).unwrap();
    let proposal = Proposal::create(text_artifact("fresh"))
        .informed_by([InfoRef::Candidate(seed)])
        .build();
    let batch = record_one(&mut ctx, proposal);
    let created = ctx
        .apply_batch(batch)
        .unwrap()
        .successful_candidates()
        .next()
        .unwrap();

    assert!(ctx.graph().parents(created).is_empty());
    assert_eq!(ctx.graph().children(seed), []);
    assert_eq!(ctx.graph().informed_by(created), [InfoRef::Candidate(seed)]);
    assert_eq!(ctx.graph().informed(seed), [created]);
}

#[test]
fn workspace_renderer_names_are_not_public_source_exports() {
    let root = workspace_root();
    for path in [
        root.join("crates/leaven-engine/src"),
        root.join("crates/leaven/src"),
    ] {
        assert_no_old_workspace_renderer_names(&path);
    }
}

struct DeterministicMaterializer;

impl Materializer<TestProblem, TextArtifact> for DeterministicMaterializer {
    async fn materialize_into(
        &self,
        value: &TextArtifact,
        workspace: &mut WorkspaceView<'_>,
        ctx: MaterializeContext<'_, TestProblem>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        workspace.write_file(&WorkspacePath::new("artifact.txt")?, value.0.as_bytes())?;
        let history = format!("candidates={}", ctx.graph().candidate_count());
        workspace.write_file(
            &WorkspacePath::new("history/candidates.txt")?,
            history.as_bytes(),
        )?;
        Ok(Metered::new(
            MaterializationReport {
                files_written: 2,
                bytes_written: u64::try_from(value.0.len() + history.len())
                    .expect("fixture byte count fits u64"),
                truncations: Vec::new(),
            },
            Cost::zero(),
        ))
    }
}

struct VisibilityMaterializer {
    hidden: leaven_core::PartitionId,
    hidden_assessment: leaven_kernel::AssessmentId,
    candidate: leaven_kernel::CandidateId,
}

impl Materializer<TestProblem, TextArtifact> for VisibilityMaterializer {
    async fn materialize_into(
        &self,
        _value: &TextArtifact,
        workspace: &mut WorkspaceView<'_>,
        ctx: MaterializeContext<'_, TestProblem>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        assert!(ctx.read_scope().hidden_partitions.contains(&self.hidden));
        assert_eq!(ctx.budget().spent.metric_calls, 1);
        assert!(ctx.graph().assessment(self.hidden_assessment).is_none());
        assert!(ctx.graph().assessments(self.candidate).is_empty());
        workspace.write_file(
            &WorkspacePath::new("visible.txt")?,
            b"hidden assessment not visible",
        )?;
        Ok(Metered::new(
            MaterializationReport {
                files_written: 1,
                bytes_written: 29,
                truncations: Vec::new(),
            },
            Cost::zero(),
        ))
    }
}

struct TestEvaluator;

impl Evaluator<TestProblem> for TestEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([8; 32])
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
                    evidence: TestEvidence { score: 1.0 },
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                })
                .collect(),
            Cost::metric_calls(1),
        ))
    }
}

struct CountingBackend {
    cleanup_count: Arc<AtomicUsize>,
}

impl WorkspaceBackend for CountingBackend {
    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>> {
        async move {
            self.cleanup_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        .boxed()
    }
}

struct FsBackend {
    root: PathBuf,
}

impl WorkspaceBackend for FsBackend {
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

    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>> {
        async move {
            remove_dir(&self.root);
            Ok(())
        }
        .boxed()
    }

    fn local_mount(&self) -> Option<&Path> {
        Some(&self.root)
    }
}

fn fs_workspace(root: PathBuf) -> Workspace {
    Workspace::new(root.clone(), Box::new(FsBackend { root }))
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

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under workspace/crates/leaven-engine")
        .to_path_buf()
}

fn assert_no_old_workspace_renderer_names(path: &Path) {
    if path.is_file() {
        if path.extension().is_some_and(|ext| ext == "rs") {
            let text = std::fs::read_to_string(path).unwrap();
            for old_name in [
                "WorkspaceRenderer",
                "ArtifactWorkspaceRenderer",
                "HistoryWorkspaceRenderer",
                "SurfaceWorkspaceRenderer",
            ] {
                assert!(
                    !text.contains(old_name),
                    "{} contains old materializer name `{old_name}`",
                    path.display()
                );
            }
        }
        return;
    }
    for entry in std::fs::read_dir(path).unwrap() {
        assert_no_old_workspace_renderer_names(&entry.unwrap().path());
    }
}
