use futures::executor::block_on;
use leaven::{
    Artifact, ArtifactIdentity, Budget, ContentId, Cost, Evidence, MaterializationReport,
    MaterializeError, Materializer, MetadataBag, OptimizationProblem, Proposal, ProposalBatch,
    ProposalBatchSemantics,
};
use leaven_core::ExternalRef;
use leaven_engine::{BudgetLedger, RunContext, RunGraph};
use leaven_kernel::{Metered, RunId, StageId};
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory, WorkspacePath, WorkspaceView};
use leaven_workspace_local::LocalWorkspaceFactory;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    block_on(async {
        let factory = LocalWorkspaceFactory::default();
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await?;
        let result = run_meta_harness_lite(&mut workspace).await;
        let cleanup = workspace.cleanup().await;
        match (result, cleanup) {
            (Ok(candidate), Ok(())) => {
                println!(
                    "p4 meta-harness lite: candidate={candidate} materialized=true cleanup=true"
                );
                Ok(())
            }
            (Err(err), Ok(())) => Err(err),
            (Ok(_), Err(err)) => Err(Box::<dyn std::error::Error>::from(err)),
            (Err(stage), Err(cleanup)) => {
                Err(format!("stage failed: {stage}; cleanup failed: {cleanup}").into())
            }
        }
    })
}

async fn run_meta_harness_lite(
    workspace: &mut leaven_workspace::Workspace,
) -> Result<leaven_kernel::CandidateId, Box<dyn std::error::Error>> {
    let mut graph = RunGraph::<MetaHarnessProblem>::new(RunId::new());
    let mut budget = BudgetLedger::new(Budget::metric_calls(10));
    let mut ctx = RunContext::new(&mut graph, &mut budget);
    let materializer = HarnessMaterializer;
    let seed = HarnessArtifact {
        source: "def score(x):\n    return 0\n".to_owned(),
        notes: "baseline harness".to_owned(),
    };

    {
        let mut view = workspace.view();
        materializer
            .materialize_into(
                &seed,
                &mut view,
                ctx.render_context(StageId::custom("p4/materialize-seed")),
            )
            .await?;
        view.write_file(
            &WorkspacePath::new("output/harness_0.py")?,
            b"def score(x):\n    return 1\n",
        )?;
        view.write_file(
            &WorkspacePath::new("output/notes_0.md")?,
            b"fresh harness authored from materialized history\n",
        )?;
    }

    let view = workspace.view();
    let source = String::from_utf8(view.read_file(&WorkspacePath::new("output/harness_0.py")?)?)?;
    let notes = String::from_utf8(view.read_file(&WorkspacePath::new("output/notes_0.md")?)?)?;
    let report = ctx.record_proposal_batch(
        StageId::custom("p4/create-harness"),
        ProposalBatch {
            proposals: vec![
                Proposal::create(HarnessArtifact { source, notes })
                    .informed_by([leaven::InfoRef::External(ExternalRef {
                        kind: "workspace".to_owned(),
                        id: "meta-harness-lite".to_owned(),
                    })])
                    .build(),
            ],
            semantics: ProposalBatchSemantics::Alternatives,
            metadata: MetadataBag::new(),
        },
        Cost::metric_calls(1),
    )?;
    let applied = ctx.apply_batch(report.batch_id)?;
    let candidate = applied
        .successful_candidates()
        .next()
        .expect("fresh harness should apply");
    let artifact = ctx.graph().artifact(candidate).expect("candidate exists");
    assert!(artifact.source.contains("return 1"));
    assert!(artifact.notes.contains("fresh harness"));
    Ok(candidate)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HarnessArtifact {
    source: String,
    notes: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HarnessChange;

#[derive(Debug)]
struct HarnessError;

impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("harness artifact does not support in-place changes in this example")
    }
}

impl std::error::Error for HarnessError {}

impl Artifact for HarnessArtifact {
    type Change = HarnessChange;
    type ApplyError = HarnessError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(content_id(
            format!("{}\n{}", self.source, self.notes).as_bytes(),
        ))
    }

    fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Err(HarnessError)
    }
}

struct MetaHarnessProblem;

impl OptimizationProblem for MetaHarnessProblem {
    type Artifact = HarnessArtifact;
    type Case = ();
    type Evidence = HarnessEvidence;
    type ProposalAnnotations = ();
}

struct HarnessEvidence;

impl Evidence for HarnessEvidence {}

struct HarnessMaterializer;

impl Materializer<MetaHarnessProblem, HarnessArtifact> for HarnessMaterializer {
    async fn materialize_into(
        &self,
        value: &HarnessArtifact,
        workspace: &mut WorkspaceView<'_>,
        _ctx: leaven::RenderContext<'_, MetaHarnessProblem>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        workspace.write_file(
            &WorkspacePath::new("harnesses/seed/harness.py")?,
            value.source.as_bytes(),
        )?;
        workspace.write_file(
            &WorkspacePath::new("harnesses/seed/notes.md")?,
            value.notes.as_bytes(),
        )?;
        let bytes_written = u64::try_from(value.source.len() + value.notes.len())
            .expect("materialized fixture byte count fits u64");
        Ok(Metered::new(
            MaterializationReport {
                files_written: 2,
                bytes_written,
                truncations: Vec::new(),
            },
            Cost::metric_calls(1),
        ))
    }
}

fn content_id(bytes: &[u8]) -> ContentId {
    let mut id = [0; ContentId::BYTES];
    let len = bytes.len().min(ContentId::BYTES);
    id[..len].copy_from_slice(&bytes[..len]);
    ContentId::from_bytes(id)
}
