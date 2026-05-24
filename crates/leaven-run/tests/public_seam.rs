use std::sync::Arc;

use futures::{FutureExt, executor::block_on};
use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, Evidence, OptimizationProblem, Proposal,
    ProposalBatch, ProposalBatchSemantics, ResolvedEvaluationRequest,
};
use leaven_engine::{
    ApplyOutcome, BudgetLedger, CachePolicy, CaseSet, EvaluationContext, EvaluationError,
    Evaluator, ProposalBatchReport, RunContext, RunGraph,
};
use leaven_evidence::{
    CandidateAssessmentOutput, CaseAssessmentEvidence, DataClass, DataClassSet, OutputBlobAudit,
    OutputMetadata, OutputRecord, OutputVisibility, ScalarEvidence,
};
use leaven_kernel::{
    AssessmentId, Budget, CandidateId, CaseId, ContentId, Cost, EvaluatorId, Fingerprint,
    MetadataBag, Metered, ProposalBatchId, ProposalId, RunId, StageId,
};
use leaven_run::{
    PublicAssessmentWriteReceiptContext, PublicAssessmentWriteReceiptProjectionError,
    PublicProposalWriteReceiptContext, PublicProposalWriteReceiptProjectionError, RunCase,
    RunOutput, RunProblem, RuntimeFingerprint, Score, ScoreContext, ScoringEvaluator,
    ScoringEvaluatorIdentity,
};
use leaven_store_inline::InlineEvidenceStore;

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

    let mut wrong_outcome_proposal = apply_report.clone();
    wrong_outcome_proposal.outcomes[0].proposal_id = ProposalId::new();
    let wrong_outcome = context
        .proposal_apply_plan_result(&graph_view, &batch_report, &wrong_outcome_proposal)
        .unwrap_err();
    assert!(matches!(
        wrong_outcome,
        PublicProposalWriteReceiptProjectionError::ApplyOutcomeMismatch
    ));

    let mut empty_apply = apply_report.clone();
    empty_apply.outcomes.clear();
    let empty_apply = context
        .proposal_apply_plan_result(&graph_view, &batch_report, &empty_apply)
        .unwrap_err();
    assert!(matches!(
        empty_apply,
        PublicProposalWriteReceiptProjectionError::EmptyApplyBatch
    ));

    let mut partial_apply = apply_report.clone();
    partial_apply.outcomes.pop();
    let partial_apply = context
        .proposal_apply_plan_result(&graph_view, &batch_report, &partial_apply)
        .unwrap_err();
    assert!(matches!(
        partial_apply,
        PublicProposalWriteReceiptProjectionError::ApplyOutcomeSetMismatch
    ));

    let mut duplicate_apply = apply_report.clone();
    duplicate_apply.outcomes[1] = duplicate_apply.outcomes[0].clone();
    let duplicate_apply = context
        .proposal_apply_plan_result(&graph_view, &batch_report, &duplicate_apply)
        .unwrap_err();
    assert!(matches!(
        duplicate_apply,
        PublicProposalWriteReceiptProjectionError::ApplyOutcomeSetMismatch
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

#[test]
fn runcontext_assessments_project_to_public_seam_submit_assessment_receipts() {
    block_on(async {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
            .expect("public seam package loads from workspace");
        let (mut graph, mut budget, candidates) = graph_with_eval_seeds();
        let case_set = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_evidence_store(&store);
        let mut report = ctx
            .evaluate_with(&OneAssessmentEvaluator, independent_request(candidates))
            .await
            .unwrap();
        report.assessment_ids.reverse();
        let graph_view = ctx.graph();
        let result = assessment_receipt_context()
            .submit_assessments_plan_result(&graph_view, &report)
            .expect("RunContext-backed assessments project");

        let document = package
            .validate_plan_result_document(&result)
            .expect("projected assessment write receipt validates through public seam owner");
        assert!(
            document
                .value_kinds()
                .contains(&"assessment_batch_receipt".to_owned())
        );
        assert_eq!(result["receipts"][0]["write_kind"], "submit_assessments");
        assert_eq!(
            result["values"]["assessment_batch"]["per_assessment"][0]["replayability"],
            "fully_managed"
        );
        assert_eq!(
            result["values"]["assessment_batch"]["assessment_ids"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    });
}

#[test]
fn runcontext_assessment_score_outputs_project_to_public_seam_submit_assessments_plan() {
    block_on(async {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
            .expect("public seam package loads from workspace");
        let (mut graph, mut budget, candidate) = graph_with_run_eval_seed();
        let case_set = CaseSet::new(vec![leaven_eval::Case::input(CaseId::new(0), "case")]);
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut ctx =
            RunContext::<RunProblem<TextArtifact, &'static str>>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
        let report = ctx
            .evaluate_with(&ScoreOutputEvaluator, independent_request(vec![candidate]))
            .await
            .unwrap();
        let graph_view = ctx.graph();
        let plan = assessment_receipt_context()
            .submit_assessments_plan_document(&graph_view, &store, &report)
            .expect("RunContext-backed assessment evidence projects to Plan IR");

        let document = package
            .validate_plan_document(&plan)
            .expect("projected submit_assessments Plan IR validates through public seam owner");
        assert_eq!(document.assessment_score_output_count(), 1);
        assert_eq!(
            plan["ops"][0]["write"]["assessments"][0]["score"]["output"]["value"]["candidate"],
            json_candidate_ref(candidate)
        );
        assert_eq!(
            plan["ops"][0]["write"]["assessments"][0]["score"]["output"]["value"]["output"],
            "candidate-output"
        );
    });
}

#[test]
fn runcontext_assessment_evidence_visibility_projects_to_public_seam_plan_result() {
    block_on(async {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
            .expect("public seam package loads from workspace");
        let (mut graph, mut budget, candidate) = graph_with_target_eval_seed();
        let case = leaven_eval::Case::targeted(
            CaseId::new(0),
            PromptInput { addend: 2 },
            AnswerTarget { answer: 3 },
        );
        let case_set = CaseSet::new(vec![case.clone()]);
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut ctx = RunContext::<RunProblem<TextArtifact, PromptInput, AnswerTarget>>::new(
            &mut graph,
            &mut budget,
        )
        .with_case_set(&case_set)
        .with_evidence_store(&store);
        let evaluator = ScoringEvaluator::new(
            Arc::new(vec![case]),
            Arc::new(|artifact: TextArtifact, case: RunCase<PromptInput>| {
                async move {
                    Ok(RunOutput::new(
                        (artifact.0 + case.input().addend).to_string(),
                    ))
                }
                .boxed()
            }),
            Arc::new(
                |ctx: ScoreContext<TextArtifact, PromptInput, AnswerTarget, String>| {
                    async move {
                        let target = ctx.load_target().expect("target is visible to scorer");
                        let output = ctx.output.output.clone();
                        Ok(Score::new(
                            f64::from(u8::from(output == target.answer.to_string())),
                            "target checked",
                        )
                        .with_output(ctx.report_text_output(output)))
                    }
                    .boxed()
                },
            ),
            &scoring_identity("assessment-evidence-visibility"),
        );
        let report = ctx
            .evaluate_with(&evaluator, independent_request(vec![candidate]))
            .await
            .unwrap();
        let graph_view = ctx.graph();
        let result = assessment_receipt_context()
            .submit_assessments_plan_result_with_evidence(&graph_view, &store, &report)
            .expect("RunContext-backed assessment evidence projects to Plan Result");

        let document = package
            .validate_plan_result_document(&result)
            .expect("projected evidence Plan Result validates through public seam owner");
        assert!(document.value_kinds().contains(&"graph_set".to_owned()));
        assert_eq!(
            result["values"]["assessment_rows"]["items"][0]["evidence"]["source_receipts"]["read"],
            serde_json::json!(["qrec_case_0_target"])
        );
        assert_eq!(
            result["receipts"][0]["trace_refs"][0]["data_classes"],
            serde_json::json!(["case.target", "public"])
        );
        assert_eq!(
            result["values"]["assessment_rows"]["items"][0]["evidence"]["target_derived"],
            serde_json::json!(true)
        );

        let mut missing_receipt_visibility = result;
        missing_receipt_visibility["receipts"][0]
            .as_object_mut()
            .unwrap()
            .remove("trace_refs");
        let error = package
            .validate_plan_result_document(&missing_receipt_visibility)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("must carry receipt trace data classes"),
            "{error}"
        );
    });
}

#[test]
fn runcontext_assessment_artifact_score_outputs_project_to_public_seam_submit_assessments_plan() {
    block_on(async {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
            .expect("public seam package loads from workspace");
        let (mut graph, mut budget, candidate) = graph_with_run_eval_seed();
        let case_set = CaseSet::new(vec![leaven_eval::Case::input(CaseId::new(0), "case")]);
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut ctx =
            RunContext::<RunProblem<TextArtifact, &'static str>>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
        let report = ctx
            .evaluate_with(
                &ArtifactScoreOutputEvaluator,
                independent_request(vec![candidate]),
            )
            .await
            .unwrap();
        let graph_view = ctx.graph();
        let plan = assessment_receipt_context()
            .submit_assessments_plan_document(&graph_view, &store, &report)
            .expect("RunContext-backed artifact evidence projects to Plan IR");

        let document = package
            .validate_plan_document(&plan)
            .expect("projected artifact submit_assessments Plan IR validates");
        assert_eq!(document.assessment_score_output_count(), 1);
        assert_eq!(
            plan["ops"][0]["write"]["assessments"][0]["score"]["output"]["value"]["candidate"],
            json_candidate_ref(candidate)
        );
        assert_eq!(
            plan["ops"][0]["write"]["assessments"][0]["score"]["output"]["value"]["artifact"],
            "candidate-artifact"
        );
    });
}

#[test]
fn runcontext_blob_score_outputs_project_to_public_seam_submit_assessments_plan() {
    block_on(async {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
            .expect("public seam package loads from workspace");
        let (mut graph, mut budget, candidate) = graph_with_run_eval_seed();
        let case_set = CaseSet::new(vec![leaven_eval::Case::input(CaseId::new(0), "case")]);
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut ctx =
            RunContext::<RunProblem<TextArtifact, &'static str>>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
        let report = ctx
            .evaluate_with(
                &AuditedBlobScoreOutputEvaluator,
                independent_request(vec![candidate]),
            )
            .await
            .unwrap();
        let graph_view = ctx.graph();
        let plan = assessment_receipt_context()
            .submit_assessments_plan_document(&graph_view, &store, &report)
            .expect("audited blob-backed score output projects to Plan IR");

        let document = package
            .validate_plan_document(&plan)
            .expect("projected blob-backed submit_assessments Plan IR validates");
        assert_eq!(document.assessment_score_output_count(), 1);
        let output = &plan["ops"][0]["write"]["assessments"][0]["score"]["output"];
        assert_eq!(
            output["summary"],
            "blob inline:score-output sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa bytes=24"
        );
        assert_eq!(
            output["blob_ref"]["id"],
            "blob_395a63f4cf693eacb1f4d465e871bd77adda4dccd7c2c446550c9d11d04a8f64"
        );
        assert_eq!(
            output["blob_ref"]["data_classes"],
            serde_json::json!(["candidate.output", "public"])
        );
        assert_eq!(output["value"]["candidate"], json_candidate_ref(candidate));
        assert_eq!(output["value"]["output"], output["blob_ref"]);
    });
}

#[test]
fn runcontext_pairwise_and_listwise_score_outputs_project_to_public_seam_submit_assessments_plan() {
    block_on(async {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
            .expect("public seam package loads from workspace");
        let (mut graph, mut budget, candidates) = graph_with_run_eval_seeds();
        let case_set = CaseSet::new(vec![leaven_eval::Case::input(CaseId::new(0), "case")]);
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut ctx =
            RunContext::<RunProblem<TextArtifact, &'static str>>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
        let pairwise = ctx
            .evaluate_with(
                &PairwiseScoreOutputEvaluator,
                independent_request(candidates[..2].to_vec()),
            )
            .await
            .unwrap();
        let listwise = ctx
            .evaluate_with(
                &ListwiseScoreOutputEvaluator,
                independent_request(candidates.clone()),
            )
            .await
            .unwrap();
        let graph_view = ctx.graph();

        let pairwise_plan = assessment_receipt_context()
            .submit_assessments_plan_document(&graph_view, &store, &pairwise)
            .expect("pairwise RunContext assessment evidence projects to Plan IR");
        let pairwise_document = package
            .validate_plan_document(&pairwise_plan)
            .expect("projected pairwise submit_assessments Plan IR validates");
        assert_eq!(
            pairwise_document.pairwise_assessment_score_output_count(),
            1
        );
        assert_eq!(
            pairwise_plan["ops"][0]["write"]["assessments"][0]["score"]["output"]["value"][0]["candidate"],
            json_candidate_ref(candidates[0])
        );
        assert_eq!(
            pairwise_plan["ops"][0]["write"]["assessments"][0]["score"]["output"]["value"][0]["output"],
            "left-output"
        );

        let listwise_plan = assessment_receipt_context()
            .submit_assessments_plan_document(&graph_view, &store, &listwise)
            .expect("listwise RunContext assessment evidence projects to Plan IR");
        let listwise_document = package
            .validate_plan_document(&listwise_plan)
            .expect("projected listwise submit_assessments Plan IR validates");
        assert_eq!(
            listwise_document.listwise_assessment_score_output_count(),
            1
        );
        assert_eq!(
            listwise_plan["ops"][0]["write"]["assessments"][0]["score"]["output"]["value"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            listwise_plan["ops"][0]["write"]["assessments"][0]["score"]["output"]["value"][2]["output"],
            "third-output"
        );
    });
}

#[test]
fn assessment_score_output_plan_projection_rejects_missing_stored_evidence() {
    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_run_eval_seed();
        let case_set = CaseSet::new(vec![leaven_eval::Case::input(CaseId::new(0), "case")]);
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut ctx =
            RunContext::<RunProblem<TextArtifact, &'static str>>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
        let report = ctx
            .evaluate_with(&ScoreOutputEvaluator, independent_request(vec![candidate]))
            .await
            .unwrap();
        let empty_store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("empty");
        let graph_view = ctx.graph();

        let missing_evidence = assessment_receipt_context()
            .submit_assessments_plan_document(&graph_view, &empty_store, &report)
            .unwrap_err();
        assert!(matches!(
            missing_evidence,
            PublicAssessmentWriteReceiptProjectionError::EvidenceLoad { .. }
        ));
    });
}

#[test]
fn assessment_score_output_plan_projection_rejects_unsupported_shapes_and_outputs() {
    block_on(async {
        let (mut graph, mut budget, candidates) = graph_with_run_eval_seeds();
        let case_set = CaseSet::new(vec![leaven_eval::Case::input(CaseId::new(0), "case")]);
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut ctx =
            RunContext::<RunProblem<TextArtifact, &'static str>>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
        let pairwise = ctx
            .evaluate_with(
                &PairwiseMissingCandidateOutputEvaluator,
                independent_request(candidates[..2].to_vec()),
            )
            .await
            .unwrap();
        let graph_view = ctx.graph();
        let unsupported_shape = assessment_receipt_context()
            .submit_assessments_plan_document(&graph_view, &store, &pairwise)
            .unwrap_err();
        assert!(matches!(
            unsupported_shape,
            PublicAssessmentWriteReceiptProjectionError::UnsupportedScoreOutput
        ));
    });

    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_run_eval_seed();
        let case_set = CaseSet::new(vec![leaven_eval::Case::input(CaseId::new(0), "case")]);
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut ctx =
            RunContext::<RunProblem<TextArtifact, &'static str>>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
        let blob_backed = ctx
            .evaluate_with(
                &BlobScoreOutputEvaluator,
                independent_request(vec![candidate]),
            )
            .await
            .unwrap();
        let graph_view = ctx.graph();
        let unsupported_output = assessment_receipt_context()
            .submit_assessments_plan_document(&graph_view, &store, &blob_backed)
            .unwrap_err();
        assert!(matches!(
            unsupported_output,
            PublicAssessmentWriteReceiptProjectionError::UnsupportedScoreOutput
        ));
    });

    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_run_eval_seed();
        let case_set = CaseSet::new(vec![leaven_eval::Case::input(CaseId::new(0), "case")]);
        let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("inline");
        let mut ctx =
            RunContext::<RunProblem<TextArtifact, &'static str>>::new(&mut graph, &mut budget)
                .with_case_set(&case_set)
                .with_evidence_store(&store);
        let public_only = ctx
            .evaluate_with(
                &PublicOnlyScoreOutputEvaluator,
                independent_request(vec![candidate]),
            )
            .await
            .unwrap();
        let graph_view = ctx.graph();
        let unsupported_data_class = assessment_receipt_context()
            .submit_assessments_plan_document(&graph_view, &store, &public_only)
            .unwrap_err();
        assert!(matches!(
            unsupported_data_class,
            PublicAssessmentWriteReceiptProjectionError::UnsupportedScoreOutput
        ));
    });
}

#[test]
fn assessment_write_projection_rejects_global_bucket_assessment_ids() {
    block_on(async {
        let (mut graph, mut budget, candidate) = graph_with_eval_seed();
        let case_set = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
            .with_case_set(&case_set)
            .with_evidence_store(&store);
        let first = ctx
            .evaluate_with(
                &OneAssessmentEvaluator,
                independent_request(vec![candidate]),
            )
            .await
            .unwrap();
        let second = ctx
            .evaluate_with(
                &OneAssessmentEvaluator,
                independent_request(vec![candidate]),
            )
            .await
            .unwrap();
        let graph_view = ctx.graph();
        let context = assessment_receipt_context();

        let mut forged = first.clone();
        forged.assessment_ids[0] = AssessmentId::new();
        let missing_assessment = context
            .submit_assessments_plan_result(&graph_view, &forged)
            .unwrap_err();
        assert!(matches!(
            missing_assessment,
            PublicAssessmentWriteReceiptProjectionError::AssessmentNotInGraph
        ));

        let mut mismatched = second;
        mismatched.assessment_ids = first.assessment_ids;
        let request_mismatch = context
            .submit_assessments_plan_result(&graph_view, &mismatched)
            .unwrap_err();
        assert!(matches!(
            request_mismatch,
            PublicAssessmentWriteReceiptProjectionError::AssessmentRequestMismatch
        ));

        let mut empty = mismatched;
        empty.assessment_ids.clear();
        let empty_batch = context
            .submit_assessments_plan_result(&graph_view, &empty)
            .unwrap_err();
        assert!(matches!(
            empty_batch,
            PublicAssessmentWriteReceiptProjectionError::EmptyAssessmentBatch
        ));
    });
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
    let batch_report = ctx
        .record_proposal_batch(
            StageId::custom("public-seam-test"),
            ProposalBatch {
                proposals: vec![
                    Proposal::mutate(seed, 2).build(),
                    Proposal::mutate(seed, 3).build(),
                ],
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

fn assessment_receipt_context() -> PublicAssessmentWriteReceiptContext {
    PublicAssessmentWriteReceiptContext::new(
        "plan_assessment_submit",
        "rev_assessment_base",
        "rev_assessment_final",
        "fp_cap_sha256_assessment",
        "fp_policy_sha256_assessment",
    )
    .with_timing("2026-05-23T12:00:00Z", "2026-05-23T12:00:01Z")
}

fn graph_with_eval_seed() -> (RunGraph<TestProblem>, BudgetLedger, CandidateId) {
    let mut graph = RunGraph::<TestProblem>::new(RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let candidate = {
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        ctx.insert_seed(TextArtifact(1), 0).unwrap()
    };
    (graph, budget, candidate)
}

fn graph_with_eval_seeds() -> (RunGraph<TestProblem>, BudgetLedger, Vec<CandidateId>) {
    let mut graph = RunGraph::<TestProblem>::new(RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let candidates = {
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        vec![
            ctx.insert_seed(TextArtifact(1), 0).unwrap(),
            ctx.insert_seed(TextArtifact(2), 1).unwrap(),
        ]
    };
    (graph, budget, candidates)
}

fn graph_with_run_eval_seed() -> (
    RunGraph<RunProblem<TextArtifact, &'static str>>,
    BudgetLedger,
    CandidateId,
) {
    let mut graph = RunGraph::<RunProblem<TextArtifact, &'static str>>::new(RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let candidate = {
        let mut ctx =
            RunContext::<RunProblem<TextArtifact, &'static str>>::new(&mut graph, &mut budget);
        ctx.insert_seed(TextArtifact(1), 0).unwrap()
    };
    (graph, budget, candidate)
}

fn graph_with_run_eval_seeds() -> (
    RunGraph<RunProblem<TextArtifact, &'static str>>,
    BudgetLedger,
    Vec<CandidateId>,
) {
    let mut graph = RunGraph::<RunProblem<TextArtifact, &'static str>>::new(RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let candidates = {
        let mut ctx =
            RunContext::<RunProblem<TextArtifact, &'static str>>::new(&mut graph, &mut budget);
        vec![
            ctx.insert_seed(TextArtifact(1), 0).unwrap(),
            ctx.insert_seed(TextArtifact(2), 1).unwrap(),
            ctx.insert_seed(TextArtifact(3), 2).unwrap(),
        ]
    };
    (graph, budget, candidates)
}

fn graph_with_target_eval_seed() -> (
    RunGraph<RunProblem<TextArtifact, PromptInput, AnswerTarget>>,
    BudgetLedger,
    CandidateId,
) {
    let mut graph =
        RunGraph::<RunProblem<TextArtifact, PromptInput, AnswerTarget>>::new(RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let candidate = {
        let mut ctx = RunContext::<RunProblem<TextArtifact, PromptInput, AnswerTarget>>::new(
            &mut graph,
            &mut budget,
        );
        ctx.insert_seed(TextArtifact(1), 0).unwrap()
    };
    (graph, budget, candidate)
}

fn independent_request(candidates: Vec<CandidateId>) -> EvaluationRequest {
    EvaluationRequest::Independent {
        candidates,
        set: EvaluationSet::All,
        granularity: AssessmentGranularity::PerCase,
        purpose: EvaluationPurpose::Validation,
    }
}

fn json_candidate_ref(candidate: CandidateId) -> serde_json::Value {
    serde_json::Value::String(format!("cand_{}", candidate.as_uuid()))
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}

fn scoring_identity(label: &str) -> ScoringEvaluatorIdentity {
    ScoringEvaluatorIdentity {
        label: label.to_owned(),
        runner: RuntimeFingerprint::new(Fingerprint::from_bytes([71; 32])),
        scorer: RuntimeFingerprint::new(Fingerprint::from_bytes([72; 32])),
        dataset: Fingerprint::from_bytes([73; 32]),
        splits: Fingerprint::from_bytes([74; 32]),
        cache_policy: CachePolicy::Never,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextArtifact(i32);

#[derive(Clone, Debug, Eq, PartialEq)]
struct PromptInput {
    addend: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AnswerTarget {
    answer: i32,
}

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

#[derive(Clone, Debug, PartialEq)]
struct TestEvidence;

impl Evidence for TestEvidence {}

struct TestProblem;

impl OptimizationProblem for TestProblem {
    type Artifact = TextArtifact;
    type Case = &'static str;
    type Evidence = TestEvidence;
    type ProposalAnnotations = ();
}

struct OneAssessmentEvaluator;

impl Evaluator<TestProblem> for OneAssessmentEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([31; 32])
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, TestProblem>,
    ) -> Result<Metered<Vec<Assessment<TestProblem>>>, EvaluationError> {
        let leaven_core::ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message("unsupported request".to_owned()));
        };
        Ok(Metered::new(
            candidates
                .into_iter()
                .map(|candidate| Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Unscoped,
                    evidence: TestEvidence,
                    cost: Cost::zero(),
                    metadata: MetadataBag::new(),
                })
                .collect(),
            Cost::zero(),
        ))
    }
}

struct ScoreOutputEvaluator;

impl Evaluator<RunProblem<TextArtifact, &'static str>> for ScoreOutputEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([41; 32])
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, RunProblem<TextArtifact, &'static str>>,
    ) -> Result<Metered<Vec<Assessment<RunProblem<TextArtifact, &'static str>>>>, EvaluationError>
    {
        let leaven_core::ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message("unsupported request".to_owned()));
        };
        let score = ScalarEvidence::new(1.0)
            .map_err(|error| EvaluationError::Message(error.to_string()))?;
        Ok(Metered::new(
            candidates
                .into_iter()
                .map(|candidate| Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Unscoped,
                    evidence: CaseAssessmentEvidence::new(
                        score,
                        OutputRecord::candidate_inline("candidate-output"),
                        "ok",
                    ),
                    cost: Cost::zero(),
                    metadata: MetadataBag::new(),
                })
                .collect(),
            Cost::zero(),
        ))
    }
}

struct ArtifactScoreOutputEvaluator;

impl Evaluator<RunProblem<TextArtifact, &'static str>> for ArtifactScoreOutputEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([45; 32])
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, RunProblem<TextArtifact, &'static str>>,
    ) -> Result<Metered<Vec<Assessment<RunProblem<TextArtifact, &'static str>>>>, EvaluationError>
    {
        let leaven_core::ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message("unsupported request".to_owned()));
        };
        Ok(Metered::new(
            candidates
                .into_iter()
                .map(|candidate| {
                    Ok(Assessment::Independent {
                        candidate,
                        target: AssessmentTarget::Unscoped,
                        evidence: case_assessment_evidence(candidate_artifact_output(
                            "candidate-artifact",
                        ))?,
                        cost: Cost::zero(),
                        metadata: MetadataBag::new(),
                    })
                })
                .collect::<Result<Vec<_>, EvaluationError>>()?,
            Cost::zero(),
        ))
    }
}

struct PairwiseScoreOutputEvaluator;

impl Evaluator<RunProblem<TextArtifact, &'static str>> for PairwiseScoreOutputEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([42; 32])
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, RunProblem<TextArtifact, &'static str>>,
    ) -> Result<Metered<Vec<Assessment<RunProblem<TextArtifact, &'static str>>>>, EvaluationError>
    {
        let leaven_core::ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message("unsupported request".to_owned()));
        };
        let [left, right] = candidates.as_slice() else {
            return Err(EvaluationError::Message("expected pair".to_owned()));
        };
        Ok(Metered::new(
            vec![Assessment::Pairwise {
                left: *left,
                right: *right,
                target: AssessmentTarget::Unscoped,
                evidence: case_assessment_evidence(OutputRecord::candidate_inline(
                    "left-output|right-output",
                ))?
                .with_candidate_outputs(candidate_outputs([
                    (*left, "left-output"),
                    (*right, "right-output"),
                ])?),
                cost: Cost::zero(),
                metadata: MetadataBag::new(),
            }],
            Cost::zero(),
        ))
    }
}

struct ListwiseScoreOutputEvaluator;

impl Evaluator<RunProblem<TextArtifact, &'static str>> for ListwiseScoreOutputEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([46; 32])
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, RunProblem<TextArtifact, &'static str>>,
    ) -> Result<Metered<Vec<Assessment<RunProblem<TextArtifact, &'static str>>>>, EvaluationError>
    {
        let leaven_core::ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message("unsupported request".to_owned()));
        };
        let [left, right, third] = candidates.as_slice() else {
            return Err(EvaluationError::Message(
                "expected three candidates".to_owned(),
            ));
        };
        Ok(Metered::new(
            vec![Assessment::Listwise {
                candidates: candidates.clone(),
                target: AssessmentTarget::Unscoped,
                evidence: case_assessment_evidence(OutputRecord::candidate_inline(
                    "left-output|right-output|third-output",
                ))?
                .with_candidate_outputs(candidate_outputs([
                    (*left, "left-output"),
                    (*right, "right-output"),
                    (*third, "third-output"),
                ])?),
                cost: Cost::zero(),
                metadata: MetadataBag::new(),
            }],
            Cost::zero(),
        ))
    }
}

struct PairwiseMissingCandidateOutputEvaluator;

impl Evaluator<RunProblem<TextArtifact, &'static str>> for PairwiseMissingCandidateOutputEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([47; 32])
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, RunProblem<TextArtifact, &'static str>>,
    ) -> Result<Metered<Vec<Assessment<RunProblem<TextArtifact, &'static str>>>>, EvaluationError>
    {
        let leaven_core::ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message("unsupported request".to_owned()));
        };
        let [left, right] = candidates.as_slice() else {
            return Err(EvaluationError::Message("expected pair".to_owned()));
        };
        Ok(Metered::new(
            vec![Assessment::Pairwise {
                left: *left,
                right: *right,
                target: AssessmentTarget::Unscoped,
                evidence: case_assessment_evidence(OutputRecord::candidate_inline(
                    "left-output|right-output",
                ))?,
                cost: Cost::zero(),
                metadata: MetadataBag::new(),
            }],
            Cost::zero(),
        ))
    }
}

struct BlobScoreOutputEvaluator;

impl Evaluator<RunProblem<TextArtifact, &'static str>> for BlobScoreOutputEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([43; 32])
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, RunProblem<TextArtifact, &'static str>>,
    ) -> Result<Metered<Vec<Assessment<RunProblem<TextArtifact, &'static str>>>>, EvaluationError>
    {
        let leaven_core::ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message("unsupported request".to_owned()));
        };
        Ok(Metered::new(
            candidates
                .into_iter()
                .map(|candidate| {
                    Ok(Assessment::Independent {
                        candidate,
                        target: AssessmentTarget::Unscoped,
                        evidence: case_assessment_evidence(OutputRecord::blob(
                            leaven_kernel::BlobRef {
                                store: "inline".to_owned(),
                                key: "output".to_owned(),
                            },
                        ))?,
                        cost: Cost::zero(),
                        metadata: MetadataBag::new(),
                    })
                })
                .collect::<Result<Vec<_>, EvaluationError>>()?,
            Cost::zero(),
        ))
    }
}

struct AuditedBlobScoreOutputEvaluator;

impl Evaluator<RunProblem<TextArtifact, &'static str>> for AuditedBlobScoreOutputEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([45; 32])
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, RunProblem<TextArtifact, &'static str>>,
    ) -> Result<Metered<Vec<Assessment<RunProblem<TextArtifact, &'static str>>>>, EvaluationError>
    {
        let leaven_core::ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message("unsupported request".to_owned()));
        };
        Ok(Metered::new(
            candidates
                .into_iter()
                .map(|candidate| {
                    Ok(Assessment::Independent {
                        candidate,
                        target: AssessmentTarget::Unscoped,
                        evidence: case_assessment_evidence(audited_candidate_blob_output())?,
                        cost: Cost::zero(),
                        metadata: MetadataBag::new(),
                    })
                })
                .collect::<Result<Vec<_>, EvaluationError>>()?,
            Cost::zero(),
        ))
    }
}

struct PublicOnlyScoreOutputEvaluator;

impl Evaluator<RunProblem<TextArtifact, &'static str>> for PublicOnlyScoreOutputEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([44; 32])
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, RunProblem<TextArtifact, &'static str>>,
    ) -> Result<Metered<Vec<Assessment<RunProblem<TextArtifact, &'static str>>>>, EvaluationError>
    {
        let leaven_core::ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message("unsupported request".to_owned()));
        };
        Ok(Metered::new(
            candidates
                .into_iter()
                .map(|candidate| {
                    Ok(Assessment::Independent {
                        candidate,
                        target: AssessmentTarget::Unscoped,
                        evidence: case_assessment_evidence(OutputRecord::inline("public-only"))?,
                        cost: Cost::zero(),
                        metadata: MetadataBag::new(),
                    })
                })
                .collect::<Result<Vec<_>, EvaluationError>>()?,
            Cost::zero(),
        ))
    }
}

fn case_assessment_evidence(
    output: OutputRecord,
) -> Result<CaseAssessmentEvidence, EvaluationError> {
    let score =
        ScalarEvidence::new(1.0).map_err(|error| EvaluationError::Message(error.to_string()))?;
    Ok(CaseAssessmentEvidence::new(score, output, "ok"))
}

fn candidate_artifact_output(text: impl Into<String>) -> OutputRecord {
    OutputRecord::inline(text).with_metadata(OutputMetadata::new(
        OutputVisibility::Public,
        DataClassSet::new([DataClass::candidate_artifact(), DataClass::public()]),
    ))
}

fn audited_candidate_blob_output() -> OutputRecord {
    OutputRecord::audited_blob(
        leaven_kernel::BlobRef {
            store: "inline".to_owned(),
            key: "score-output".to_owned(),
        },
        OutputBlobAudit::new(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            24,
        )
        .expect("test blob audit is valid"),
    )
    .with_metadata(OutputMetadata::new(
        OutputVisibility::Public,
        DataClassSet::public_candidate_output(),
    ))
}

fn candidate_outputs<const N: usize>(
    outputs: [(CandidateId, &'static str); N],
) -> Result<Vec<CandidateAssessmentOutput>, EvaluationError> {
    outputs
        .into_iter()
        .map(|(candidate, output)| {
            CandidateAssessmentOutput::new(candidate, OutputRecord::candidate_inline(output))
                .map_err(|error| EvaluationError::Message(error.to_string()))
        })
        .collect()
}
