use leaven_core::{Artifact, ArtifactIdentity, Proposal, ProposalBatch, ProposalBatchSemantics};
use leaven_engine::{ApplyOutcome, BudgetLedger, ProposalBatchReport, RunContext, RunGraph};
use leaven_kernel::{
    Budget, CandidateId, ContentId, Cost, MetadataBag, ProposalBatchId, RunId, StageId,
};
use leaven_run::{
    PublicProposalWriteReceiptContext, PublicProposalWriteReceiptProjectionError, RunProblem,
};

#[test]
fn runcontext_proposal_writes_project_to_public_seam_receipts() {
    let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
        .expect("public seam package loads from workspace");
    let (mut graph, mut budget, batch_report, apply_report) = graph_with_applied_proposal();
    let ctx = RunContext::<RunProblem<TextArtifact, ()>>::new(&mut graph, &mut budget);
    let graph_view = ctx.graph();
    let result = proposal_receipt_context()
        .proposal_apply_plan_result(&graph_view, &batch_report, &apply_report)
        .expect("RunContext-backed proposal writes project");

    let document = package
        .validate_plan_result_document(&result)
        .expect("projected proposal write receipts validate through public seam owner");
    assert!(
        document
            .value_kinds()
            .contains(&"proposal_batch_receipt".to_owned())
    );
    assert!(document.value_kinds().contains(&"apply_receipt".to_owned()));
    assert_eq!(result["receipts"][0]["write_kind"], "submit_proposal_batch");
    assert_eq!(result["receipts"][1]["write_kind"], "apply_proposal_batch");
}

#[test]
fn proposal_write_projection_rejects_receipts_not_backed_by_runcontext_graph_truth() {
    let (mut graph, mut budget, batch_report, apply_report) = graph_with_applied_proposal();
    let ctx = RunContext::<RunProblem<TextArtifact, ()>>::new(&mut graph, &mut budget);
    let graph_view = ctx.graph();
    let context = proposal_receipt_context();

    let mut forged_batch = batch_report.clone();
    forged_batch.batch_id = ProposalBatchId::new();
    let mut forged_batch_apply = apply_report.clone();
    forged_batch_apply.batch_id = forged_batch.batch_id;
    let missing_batch = context
        .proposal_apply_plan_result(&graph_view, &forged_batch, &forged_batch_apply)
        .unwrap_err();
    assert!(matches!(
        missing_batch,
        PublicProposalWriteReceiptProjectionError::BatchNotInGraph
    ));

    let mut forged_apply = apply_report.clone();
    forged_apply.outcomes[0].outcome = ApplyOutcome::Success {
        candidate_id: CandidateId::new(),
    };
    let forged_candidate = context
        .proposal_apply_plan_result(&graph_view, &batch_report, &forged_apply)
        .unwrap_err();
    assert!(matches!(
        forged_candidate,
        PublicProposalWriteReceiptProjectionError::CreatedCandidateNotGraphBacked
    ));

    let missing_timing = PublicProposalWriteReceiptContext::new(
        "plan_proposal_apply",
        "rev_proposal_base",
        "rev_proposal_final",
        "fp_cap_sha256_proposal",
        "fp_policy_sha256_proposal",
    )
    .proposal_apply_plan_result(&graph_view, &batch_report, &apply_report)
    .unwrap_err();
    assert!(matches!(
        missing_timing,
        PublicProposalWriteReceiptProjectionError::MissingTiming
    ));
}

fn graph_with_applied_proposal() -> (
    RunGraph<RunProblem<TextArtifact, ()>>,
    BudgetLedger,
    ProposalBatchReport,
    leaven_engine::ApplyReport,
) {
    let mut graph = RunGraph::<RunProblem<TextArtifact, ()>>::new(RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let mut ctx = RunContext::<RunProblem<TextArtifact, ()>>::new(&mut graph, &mut budget);
    let seed = ctx.insert_seed(TextArtifact(1), 0).unwrap();
    let proposal = Proposal::mutate(seed, 2).build();
    let batch_report = ctx
        .record_proposal_batch(
            StageId::custom("public-seam-test"),
            ProposalBatch {
                proposals: vec![proposal],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        )
        .unwrap();
    let apply_report = ctx.apply_batch(batch_report.batch_id).unwrap();
    drop(ctx);
    (graph, budget, batch_report, apply_report)
}

fn proposal_receipt_context() -> PublicProposalWriteReceiptContext {
    PublicProposalWriteReceiptContext::new(
        "plan_proposal_apply",
        "rev_proposal_base",
        "rev_proposal_final",
        "fp_cap_sha256_proposal",
        "fp_policy_sha256_proposal",
    )
    .with_submit_timing("2026-05-23T12:00:00Z", "2026-05-23T12:00:01Z")
    .with_apply_timing("2026-05-23T12:00:01Z", "2026-05-23T12:00:02Z")
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextArtifact(i32);

#[derive(Debug)]
struct TextArtifactError;

impl std::fmt::Display for TextArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("text artifact error")
    }
}

impl std::error::Error for TextArtifactError {}

impl Artifact for TextArtifact {
    type Change = i32;
    type ApplyError = TextArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        let mut bytes = [0; ContentId::BYTES];
        bytes[..std::mem::size_of::<i32>()].copy_from_slice(&self.0.to_le_bytes());
        ArtifactIdentity::Content(ContentId::from_bytes(bytes))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(*change))
    }
}
