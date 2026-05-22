use std::fmt::Write as _;
use std::fs::{self, File};
use std::process::Command;
use std::sync::Arc;

use arrow_array::builder::{ListBuilder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use p5_skill_paper_reproductions::evoskill::{
    DatasetMaterializationReport, EvoSkillFinalReport, ExactnessClass, ExactnessGapStatus,
    FinalScoreSlot, FinalScoreStatus, LiveRunGateStatus, ManifestBuildInput, ManifestError,
    MaterializationExactness, PaperCloseGateStatus, PaperResultTargetStatus, ProxyRejectionStatus,
    ScoreEvidenceKind, SourceBlockerStatus, SplitAcceptanceStatus, SplitMaterializationReport,
    build_evoskill_final_report, write_evoskill_local_source_pin_manifest,
    write_evoskill_paper_close_split_policy_manifest,
};
use parquet::arrow::ArrowWriter;
use sha2::{Digest, Sha256};

#[test]
fn final_report_exposes_score_slots_costs_errors_and_gaps_without_fake_metrics() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    write_sealqa_parquet(root.path(), 111);
    write_sealqa_judge_source(root.path());

    let report = build_evoskill_final_report(&ManifestBuildInput::new(root.path())).unwrap();

    assert_report_header(&report);
    assert_live_run_gate_blocks_unapproved_spend(&report);
    assert_paper_close_gates_separate_proof_from_blockers(&report);
    assert_source_blockers_are_report_visible(&report);
    assert_officeqa_paper_targets_report_ambiguity_without_blocking(&report);
    assert_proxy_rejection_gates(&report);
    let officeqa = officeqa_materialization(&report);
    let sealqa = sealqa_materialization(&report);
    assert_officeqa_train_12_score_slots(&report, officeqa);
    assert_sealqa_score_slots(&report, sealqa);
    assert_browsecomp_transfer_score_slots(&report);
    assert!(
        report
            .ablations
            .iter()
            .any(|ablation| ablation.id == "skill_merge" && ablation.status == "blocked")
    );
}

#[test]
fn final_report_uses_accepted_substitute_splits_as_unscored_denominator() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    write_sealqa_parquet(root.path(), 111);
    write_sealqa_judge_source(root.path());
    write_evoskill_paper_close_split_policy_manifest(&ManifestBuildInput::new(root.path()))
        .unwrap();

    let report = build_evoskill_final_report(&ManifestBuildInput::new(root.path())).unwrap();

    let source_split_gate = report
        .paper_close_gates
        .iter()
        .find(|gate| gate.id == "source_and_split_materialization")
        .expect("source/split paper-close gate exists");
    assert_eq!(
        source_split_gate.status,
        PaperCloseGateStatus::SourceBlocked
    );
    assert!(
        source_split_gate
            .blocker_ids
            .contains(&"browsecomp_transfer_sample".to_owned())
    );
    assert!(source_split_gate.blocker_ids.iter().all(|blocker| {
        blocker != "officeqa_exact_split_membership"
            && blocker != "officeqa_category_split_manifest"
            && blocker != "sealqa_split_manifest"
    }));
    assert!(
        report
            .manifest
            .source_blockers
            .iter()
            .all(|blocker| blocker.dataset_id != "officeqa" && blocker.dataset_id != "sealqa")
    );
    let missing_browsecomp = report
        .manifest
        .source_materializations
        .iter()
        .find(|materialization| materialization.dataset_id == "browsecomp_transfer")
        .expect("BrowseComp transfer materialization status is embedded");
    assert_eq!(
        missing_browsecomp.blocker_ids,
        ["browsecomp_transfer_sample".to_owned()]
    );

    let officeqa = officeqa_materialization(&report);
    let officeqa_split = officeqa
        .split_materializations
        .iter()
        .find(|split| split.id == "officeqa_difficulty_train_12_val_17")
        .expect("OfficeQA accepted substitute split exists");
    assert_eq!(
        officeqa_split.acceptance_status,
        SplitAcceptanceStatus::AcceptedPaperClosePolicy
    );
    assert!(officeqa_split.blocker_ids.is_empty());
    let officeqa_slots = report
        .score_slots
        .iter()
        .filter(|slot| slot.dataset_id == "officeqa")
        .collect::<Vec<_>>();
    assert!(officeqa_slots.iter().all(|slot| {
        slot.status == FinalScoreStatus::NotRun
            && slot.score.is_none()
            && slot.blocker_ids.is_empty()
            && slot.split_exactness == MaterializationExactness::PaperCloseSubstitute
    }));

    let sealqa = sealqa_materialization(&report);
    let sealqa_split = sealqa
        .split_materializations
        .iter()
        .find(|split| split.id == "sealqa_row_order_train_11_heldout_100")
        .expect("SealQA accepted substitute split exists");
    assert_eq!(
        sealqa_split.acceptance_status,
        SplitAcceptanceStatus::AcceptedPaperClosePolicy
    );
    assert!(sealqa_split.blocker_ids.is_empty());
    let sealqa_slots = report
        .score_slots
        .iter()
        .filter(|slot| slot.dataset_id == "sealqa")
        .collect::<Vec<_>>();
    assert!(sealqa_slots.iter().all(|slot| {
        slot.status == FinalScoreStatus::Blocked
            && slot.score.is_none()
            && slot.blocker_ids == ["sealqa_judge_scored_run".to_owned()]
    }));
}

#[test]
fn final_report_uses_browsecomp_sample_as_unscored_transfer_denominator() {
    let root = tempfile::tempdir().unwrap();
    write_browsecomp_transfer_sample(root.path(), 128);

    let report = build_evoskill_final_report(&ManifestBuildInput::new(root.path())).unwrap();

    assert!(
        report
            .manifest
            .source_blockers
            .iter()
            .all(|blocker| blocker.blocker_id != "browsecomp_transfer_sample")
    );
    let browsecomp = report
        .manifest
        .source_materializations
        .iter()
        .find(|materialization| materialization.dataset_id == "browsecomp_transfer")
        .expect("BrowseComp transfer materialization is embedded");
    assert_eq!(browsecomp.source_rows, Some(128));
    let split = browsecomp
        .split_materializations
        .iter()
        .find(|split| split.id == "browsecomp_transfer_sample_128_heldout")
        .expect("BrowseComp held-out transfer split exists");
    assert_eq!(
        split.exactness,
        MaterializationExactness::PaperCloseSubstitute
    );
    assert_eq!(split.test_rows, Some(128));
    assert!(split.blocker_ids.is_empty());

    let browsecomp_slots = report
        .score_slots
        .iter()
        .filter(|slot| slot.dataset_id == "browsecomp_transfer")
        .collect::<Vec<_>>();
    assert_eq!(browsecomp_slots.len(), 2);
    for slot in browsecomp_slots {
        assert_eq!(slot.split_id, "browsecomp_transfer_sample_128_heldout");
        assert_eq!(slot.split_role, "held_out_test");
        assert_eq!(
            slot.split_exactness,
            MaterializationExactness::PaperCloseSubstitute
        );
        assert_eq!(slot.split_fingerprint, split.split_fingerprint);
        assert_eq!(
            slot.role_source_id_fingerprint.as_deref().unwrap().len(),
            64
        );
        assert_eq!(slot.expected_rows, Some(128));
        assert_eq!(slot.status, FinalScoreStatus::NotRun);
        assert!(slot.score.is_none());
        assert!(slot.blocker_ids.is_empty());
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn final_report_relabels_browsecomp_ablation_after_denominator_materializes() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    write_sealqa_parquet(root.path(), 111);
    write_sealqa_judge_source(root.path());
    write_browsecomp_transfer_sample(root.path(), 128);
    init_git_source(
        &root.path().join("tmp/repros/evoskill"),
        "https://github.com/sentient-agi/EvoSkill.git",
    );
    init_git_source(
        &root.path().join("tmp/repros/officeqa"),
        "https://github.com/databricks/officeqa.git",
    );
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();

    let report = build_evoskill_final_report(&input).unwrap();

    assert!(report.manifest.source_blockers.is_empty());
    assert_eq!(
        report.manifest.exactness,
        ExactnessClass::PaperCloseCandidate
    );
    assert_eq!(report.exactness, ExactnessClass::PaperCloseCandidate);
    let browsecomp = report
        .ablations
        .iter()
        .find(|ablation| ablation.id == "browsecomp_transfer")
        .expect("BrowseComp transfer ablation exists");
    assert_eq!(browsecomp.status, "approval_blocked");
    assert_eq!(
        browsecomp.blocker_ids,
        [
            "sealqa_judge_scored_run".to_owned(),
            "live_run_spend_approval".to_owned()
        ]
    );
    assert!(browsecomp.note.contains("denominator is materialized"));
    assert!(!browsecomp.note.contains("absent"));
    let skill_merge = report
        .ablations
        .iter()
        .find(|ablation| ablation.id == "skill_merge")
        .expect("skill-merge ablation exists");
    assert_eq!(skill_merge.status, "approval_blocked");
    assert_eq!(
        skill_merge.blocker_ids,
        ["live_run_spend_approval".to_owned()]
    );
    assert!(skill_merge.note.contains("declared denominator is ready"));
    let gap = |id: &str| {
        report
            .exactness_gaps
            .iter()
            .find(|gap| gap.id == id)
            .unwrap_or_else(|| panic!("missing exactness gap {id}"))
    };

    assert_eq!(
        gap("source_revision_evoskill_repo_local_checkout").status,
        ExactnessGapStatus::PaperReleaseUnverified
    );
    assert!(
        gap("source_revision_evoskill_repo_local_checkout")
            .required_for_paper_exact
            .contains("paper-release")
    );
    assert_eq!(
        gap("source_revision_officeqa_repo_local_checkout").status,
        ExactnessGapStatus::PaperReleaseUnverified
    );
    assert_eq!(
        gap("split_officeqa_officeqa_difficulty_train_12_val_17").status,
        ExactnessGapStatus::AcceptedPaperCloseSubstitute
    );
    assert!(
        gap("split_officeqa_officeqa_difficulty_train_12_val_17")
            .required_for_paper_exact
            .contains("LLM-clustered")
    );
    assert_eq!(
        gap("split_sealqa_sealqa_row_order_train_11_heldout_100").status,
        ExactnessGapStatus::AcceptedPaperCloseSubstitute
    );
    assert!(
        gap("split_sealqa_sealqa_row_order_train_11_heldout_100")
            .required_for_paper_exact
            .contains("exact train/held-out")
    );
    assert_eq!(
        gap("split_browsecomp_transfer_browsecomp_transfer_sample_128_heldout").status,
        ExactnessGapStatus::AcceptedPaperCloseSubstitute
    );
    assert!(
        gap("split_browsecomp_transfer_browsecomp_transfer_sample_128_heldout")
            .required_for_paper_exact
            .contains("paper author's exact 128-example")
    );
    assert!(
        report
            .exactness_gaps
            .iter()
            .all(|gap| !gap.evidence.is_empty())
    );
    assert!(report.exactness_gaps.iter().all(|gap| {
        !gap.observed.contains("Some(")
            && !gap.observed.contains("None")
            && !gap.evidence.iter().any(|evidence| {
                evidence.contains("PaperCloseSubstitute")
                    || evidence.contains("AcceptedPaperClosePolicy")
            })
    }));
}

#[test]
fn final_report_imports_matching_score_result_sidecar_without_filling_other_slots() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let scored_slot = score_slot(
        &initial_report,
        "officeqa",
        "officeqa_difficulty_train_12_val_17",
        "train",
        "baseline",
    )
    .clone();
    write_score_result_manifest(root.path(), &initial_report, &scored_slot, 1.0);

    let report = build_evoskill_final_report(&input).unwrap();

    assert_eq!(report.schema_version, 19);
    let result_manifest = report
        .score_result_manifest
        .as_ref()
        .expect("score result sidecar is reported");
    assert_eq!(
        result_manifest.relative_path,
        "tmp/replication/evoskill/score_result_manifest.json"
    );
    assert_eq!(result_manifest.schema_version, 5);
    assert_eq!(result_manifest.entries, 1);
    assert_eq!(
        result_manifest.cost.metric_calls,
        scored_slot.expected_rows.unwrap()
    );
    assert_eq!(
        result_manifest.manifest_fingerprint,
        initial_report.manifest_fingerprint.fingerprint
    );
    let reported = score_slot(
        &report,
        "officeqa",
        "officeqa_difficulty_train_12_val_17",
        "train",
        "baseline",
    );
    assert_eq!(reported.status, FinalScoreStatus::Reported);
    assert_eq!(reported.score, Some(1.0));
    assert_reported_score_evidence(reported, ScoreEvidenceKind::RustScorerReplay, None);
    let evidence_artifact = reported
        .score_evidence_artifact
        .as_ref()
        .expect("reported score preserves checked evidence artifact");
    assert_eq!(
        evidence_artifact.relative_path,
        "tmp/replication/evoskill/score-evidence/unit-test-scored-output-import.jsonl"
    );
    assert!(evidence_artifact.bytes > 0);
    assert_eq!(evidence_artifact.sha256.len(), 64);
    assert!(reported.blocker_ids.is_empty());
    let untouched = score_slot(
        &report,
        "officeqa",
        "officeqa_difficulty_train_12_val_17",
        "train",
        "optimized",
    );
    assert_eq!(untouched.status, FinalScoreStatus::NotRun);
    assert!(untouched.score.is_none());
    assert_eq!(untouched.score_evidence_id, None);
    assert_eq!(untouched.score_evidence_artifact, None);
    assert_eq!(untouched.score_evidence_kind, None);
    assert_eq!(untouched.score_evidence_approval_id, None);
    let sealqa = score_slot(
        &report,
        "sealqa",
        "sealqa_row_order_train_11_heldout_100",
        "train",
        "baseline",
    );
    assert_eq!(sealqa.status, FinalScoreStatus::Blocked);
    assert!(
        sealqa
            .blocker_ids
            .contains(&"sealqa_judge_scored_run".to_owned())
    );
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.blocker_id == "sealqa_judge_scored_run")
    );
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.blocker_id == "live_run_spend_approval")
    );
}

#[test]
fn final_report_refuses_score_result_sidecar_that_claims_source_or_split_blockers() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let scored_slot = score_slot(
        &initial_report,
        "officeqa",
        "officeqa_difficulty_train_12_val_17",
        "train",
        "baseline",
    )
    .clone();
    assert_eq!(scored_slot.status, FinalScoreStatus::Blocked);
    assert!(
        scored_slot
            .blocker_ids
            .contains(&"officeqa_exact_split_membership".to_owned())
    );
    write_score_result_manifest(root.path(), &initial_report, &scored_slot, 1.0);

    let error = build_evoskill_final_report(&input).unwrap_err();

    match error {
        ManifestError::ScoreResultManifest { message, .. } => {
            assert!(message.contains("cannot resolve non-score blocker"));
            assert!(message.contains("officeqa_category_split_manifest"));
            assert!(
                message.contains("officeqa|officeqa_difficulty_train_12_val_17|train|baseline")
            );
        }
        other => panic!("expected score result manifest error, got {other:?}"),
    }
}

#[test]
fn final_report_refuses_score_result_sidecar_with_stale_slot_fingerprint() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let scored_slot = score_slot(
        &initial_report,
        "officeqa",
        "officeqa_difficulty_train_12_val_17",
        "train",
        "baseline",
    )
    .clone();
    write_score_result_manifest_with_split_fingerprint(
        root.path(),
        &initial_report,
        &scored_slot,
        "stale-split-fingerprint",
        1.0,
    );

    let error = build_evoskill_final_report(&input).unwrap_err();

    match error {
        ManifestError::ScoreResultManifest { message, .. } => {
            assert!(message.contains("split fingerprint"));
            assert!(
                message.contains("officeqa|officeqa_difficulty_train_12_val_17|train|baseline")
            );
        }
        other => panic!("expected score result manifest error, got {other:?}"),
    }
}

#[test]
fn final_report_refuses_score_result_sidecar_with_tampered_evidence_artifact() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let scored_slot = score_slot(
        &initial_report,
        "officeqa",
        "officeqa_difficulty_train_12_val_17",
        "train",
        "baseline",
    )
    .clone();
    write_score_result_manifest(root.path(), &initial_report, &scored_slot, 1.0);
    fs::write(
        root.path()
            .join("tmp/replication/evoskill/score-evidence/unit-test-scored-output-import.jsonl"),
        b"{\"tampered\":true}\n",
    )
    .unwrap();

    let error = build_evoskill_final_report(&input).unwrap_err();

    match error {
        ManifestError::ScoreResultManifest { message, .. } => {
            assert!(message.contains("evidence artifact"));
            assert!(message.contains("sha256 mismatch") || message.contains("bytes"));
        }
        other => panic!("expected score result manifest error, got {other:?}"),
    }
}

#[test]
fn final_report_refuses_score_result_sidecar_when_officeqa_artifact_predictions_do_not_score() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let scored_slot = score_slot(
        &initial_report,
        "officeqa",
        "officeqa_difficulty_train_12_val_17",
        "train",
        "baseline",
    )
    .clone();
    write_score_result_manifest_with_prediction_override(
        root.path(),
        &initial_report,
        &scored_slot,
        "definitely not the gold answer",
        1.0,
    );

    let error = build_evoskill_final_report(&input).unwrap_err();

    match error {
        ManifestError::ScoreResultManifest { message, .. } => {
            assert!(message.contains("OfficeQA scorer"));
            assert!(
                message.contains("officeqa|officeqa_difficulty_train_12_val_17|train|baseline")
            );
        }
        other => panic!("expected score result manifest error, got {other:?}"),
    }
}

#[test]
fn final_report_imports_browsecomp_score_sidecar_after_recomputing_accuracy() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let scored_slot = score_slot(
        &initial_report,
        "browsecomp_transfer",
        "browsecomp_transfer_sample_128_heldout",
        "held_out_test",
        "baseline",
    )
    .clone();
    write_score_result_manifest(root.path(), &initial_report, &scored_slot, 1.0);

    let report = build_evoskill_final_report(&input).unwrap();

    let reported = score_slot(
        &report,
        "browsecomp_transfer",
        "browsecomp_transfer_sample_128_heldout",
        "held_out_test",
        "baseline",
    );
    assert_eq!(reported.status, FinalScoreStatus::Reported);
    assert_eq!(reported.score, Some(1.0));
    assert_reported_score_evidence(reported, ScoreEvidenceKind::ExactAnswerReplay, None);
    let transfer = score_slot(
        &report,
        "browsecomp_transfer",
        "browsecomp_transfer_sample_128_heldout",
        "held_out_test",
        "sealqa_skill_transfer",
    );
    assert_eq!(transfer.status, FinalScoreStatus::NotRun);
    assert!(transfer.score.is_none());
}

#[test]
fn final_report_imports_sealqa_judge_score_sidecar_with_approval_evidence() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let scored_slot = score_slot(
        &initial_report,
        "sealqa",
        "sealqa_row_order_train_11_heldout_100",
        "train",
        "baseline",
    )
    .clone();
    assert_eq!(scored_slot.status, FinalScoreStatus::Blocked);
    assert_eq!(
        scored_slot.blocker_ids,
        ["sealqa_judge_scored_run".to_owned()]
    );
    write_score_result_manifest_with_judge_template_fingerprint(
        root.path(),
        &initial_report,
        &scored_slot,
        1.0,
        ScoreEvidenceKind::ExternalJudgeRun,
        Some("unit-test-approved-sealqa-judge-run"),
        Some(judge_template_fingerprint_for_dataset(
            &initial_report,
            "sealqa",
        )),
    );

    let report = build_evoskill_final_report(&input).unwrap();

    let reported = score_slot(
        &report,
        "sealqa",
        "sealqa_row_order_train_11_heldout_100",
        "train",
        "baseline",
    );
    assert_eq!(reported.status, FinalScoreStatus::Reported);
    assert_eq!(reported.score, Some(1.0));
    assert_reported_score_evidence(
        reported,
        ScoreEvidenceKind::ExternalJudgeRun,
        Some("unit-test-approved-sealqa-judge-run"),
    );
    assert!(reported.blocker_ids.is_empty());
    assert_eq!(report.cost.llm_calls, scored_slot.expected_rows.unwrap());
    let scorer_gate = report
        .paper_close_gates
        .iter()
        .find(|gate| gate.id == "paper_scorer")
        .expect("paper scorer gate exists");
    assert_eq!(scorer_gate.status, PaperCloseGateStatus::ApprovalBlocked);
    assert!(
        scorer_gate
            .blocker_ids
            .contains(&"sealqa_judge_scored_run".to_owned())
    );
}

#[test]
fn final_report_promotes_scorer_gate_only_after_all_sealqa_judge_slots_reported() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let sealqa_slots = initial_report
        .score_slots
        .iter()
        .filter(|slot| slot.dataset_id == "sealqa")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(sealqa_slots.len(), 4);
    assert!(sealqa_slots.iter().all(|slot| {
        slot.status == FinalScoreStatus::Blocked
            && slot.blocker_ids == ["sealqa_judge_scored_run".to_owned()]
    }));
    write_score_result_manifest_for_slots_with_judge_template_fingerprint(
        root.path(),
        &initial_report,
        &sealqa_slots,
        1.0,
        ScoreEvidenceKind::ExternalJudgeRun,
        Some("unit-test-approved-sealqa-judge-run"),
        Some(judge_template_fingerprint_for_dataset(
            &initial_report,
            "sealqa",
        )),
    );

    let report = build_evoskill_final_report(&input).unwrap();

    let reported_sealqa_slots = report
        .score_slots
        .iter()
        .filter(|slot| slot.dataset_id == "sealqa")
        .collect::<Vec<_>>();
    assert_eq!(reported_sealqa_slots.len(), sealqa_slots.len());
    assert!(reported_sealqa_slots.iter().all(|slot| {
        slot.status == FinalScoreStatus::Reported
            && slot.score == Some(1.0)
            && slot.score_evidence_kind == Some(ScoreEvidenceKind::ExternalJudgeRun)
            && slot.score_evidence_approval_id.as_deref()
                == Some("unit-test-approved-sealqa-judge-run")
            && slot.blocker_ids.is_empty()
    }));
    assert_eq!(
        report.cost.llm_calls,
        sealqa_slots
            .iter()
            .map(|slot| slot.expected_rows.unwrap())
            .sum::<u64>()
    );

    let paper_scorer = report
        .paper_close_gates
        .iter()
        .find(|gate| gate.id == "paper_scorer")
        .expect("paper scorer gate exists");
    assert_eq!(paper_scorer.status, PaperCloseGateStatus::Proven);
    assert!(paper_scorer.blocker_ids.is_empty());
    let live_small_run = report
        .paper_close_gates
        .iter()
        .find(|gate| gate.id == "live_small_run")
        .expect("live small run gate exists");
    assert_eq!(live_small_run.status, PaperCloseGateStatus::ApprovalBlocked);
    assert_eq!(
        live_small_run.blocker_ids,
        ["live_run_spend_approval".to_owned()]
    );
    assert!(
        report
            .errors
            .iter()
            .all(|error| error.blocker_id != "sealqa_judge_scored_run")
    );
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.blocker_id == "live_run_spend_approval")
    );
    let browsecomp = report
        .ablations
        .iter()
        .find(|ablation| ablation.id == "browsecomp_transfer")
        .expect("BrowseComp transfer ablation exists");
    assert_eq!(
        browsecomp.blocker_ids,
        ["live_run_spend_approval".to_owned()]
    );
}

#[test]
fn final_report_refuses_sealqa_judge_sidecar_without_approval_evidence() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let scored_slot = score_slot(
        &initial_report,
        "sealqa",
        "sealqa_row_order_train_11_heldout_100",
        "train",
        "baseline",
    )
    .clone();
    write_score_result_manifest_with_evidence_kind(
        root.path(),
        &initial_report,
        &scored_slot,
        1.0,
        ScoreEvidenceKind::ExternalJudgeRun,
        None,
    );

    let error = build_evoskill_final_report(&input).unwrap_err();

    match error {
        ManifestError::ScoreResultManifest { message, .. } => {
            assert!(message.contains("external judge"));
            assert!(message.contains("approval"));
            assert!(
                message.contains("sealqa|sealqa_row_order_train_11_heldout_100|train|baseline")
            );
        }
        other => panic!("expected score result manifest error, got {other:?}"),
    }
}

#[test]
fn final_report_refuses_sealqa_judge_sidecar_without_template_fingerprint() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let scored_slot = score_slot(
        &initial_report,
        "sealqa",
        "sealqa_row_order_train_11_heldout_100",
        "train",
        "baseline",
    )
    .clone();
    write_score_result_manifest_with_evidence_kind(
        root.path(),
        &initial_report,
        &scored_slot,
        1.0,
        ScoreEvidenceKind::ExternalJudgeRun,
        Some("unit-test-approved-sealqa-judge-run"),
    );

    let error = build_evoskill_final_report(&input).unwrap_err();

    match error {
        ManifestError::ScoreResultManifest { message, .. } => {
            assert!(message.contains("judge template fingerprint"));
            assert!(
                message.contains("sealqa|sealqa_row_order_train_11_heldout_100|train|baseline")
            );
        }
        other => panic!("expected score result manifest error, got {other:?}"),
    }
}

#[test]
fn final_report_refuses_sealqa_judge_sidecar_with_stale_template_fingerprint() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let scored_slot = score_slot(
        &initial_report,
        "sealqa",
        "sealqa_row_order_train_11_heldout_100",
        "train",
        "baseline",
    )
    .clone();
    write_score_result_manifest_with_judge_template_fingerprint(
        root.path(),
        &initial_report,
        &scored_slot,
        1.0,
        ScoreEvidenceKind::ExternalJudgeRun,
        Some("unit-test-approved-sealqa-judge-run"),
        Some("stale-template-fingerprint"),
    );

    let error = build_evoskill_final_report(&input).unwrap_err();

    match error {
        ManifestError::ScoreResultManifest { message, .. } => {
            assert!(message.contains("judge template fingerprint"));
            assert!(message.contains("stale-template-fingerprint"));
            assert!(
                message.contains("sealqa|sealqa_row_order_train_11_heldout_100|train|baseline")
            );
        }
        other => panic!("expected score result manifest error, got {other:?}"),
    }
}

#[test]
fn final_report_refuses_browsecomp_sidecar_when_predictions_do_not_match_answers() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let scored_slot = score_slot(
        &initial_report,
        "browsecomp_transfer",
        "browsecomp_transfer_sample_128_heldout",
        "held_out_test",
        "baseline",
    )
    .clone();
    write_score_result_manifest_with_prediction_override(
        root.path(),
        &initial_report,
        &scored_slot,
        "definitely not the browsecomp answer",
        1.0,
    );

    let error = build_evoskill_final_report(&input).unwrap_err();

    match error {
        ManifestError::ScoreResultManifest { message, .. } => {
            assert!(message.contains("BrowseComp exact-answer scorer"));
            assert!(message.contains(
                "browsecomp_transfer|browsecomp_transfer_sample_128_heldout|held_out_test|baseline"
            ));
        }
        other => panic!("expected score result manifest error, got {other:?}"),
    }
}

fn assert_paper_close_gates_separate_proof_from_blockers(report: &EvoSkillFinalReport) {
    assert_eq!(report.paper_close_gates.len(), 7);
    let gate = |id: &str| {
        report
            .paper_close_gates
            .iter()
            .find(|gate| gate.id == id)
            .unwrap_or_else(|| panic!("missing paper-close gate {id}"))
    };

    assert_eq!(
        gate("replica_manifest").status,
        PaperCloseGateStatus::Proven
    );
    assert_eq!(
        gate("source_and_split_materialization").status,
        PaperCloseGateStatus::SourceBlocked
    );
    assert!(
        gate("source_and_split_materialization")
            .blocker_ids
            .contains(&"officeqa_exact_split_membership".to_owned())
    );
    assert_eq!(
        gate("paper_scorer").status,
        PaperCloseGateStatus::ApprovalBlocked
    );
    assert!(
        gate("paper_scorer")
            .blocker_ids
            .contains(&"sealqa_judge_scored_run".to_owned())
    );
    assert_eq!(
        gate("full_loop_mechanics").status,
        PaperCloseGateStatus::Proven
    );
    assert!(
        gate("full_loop_mechanics")
            .note
            .contains("mechanics evidence only")
    );
    assert_eq!(
        gate("live_small_run").status,
        PaperCloseGateStatus::ApprovalBlocked
    );
    assert_eq!(
        gate("final_report_truth").status,
        PaperCloseGateStatus::Proven
    );
    assert_eq!(gate("proxy_closeout").status, PaperCloseGateStatus::Proven);
}

fn assert_source_blockers_are_report_visible(report: &EvoSkillFinalReport) {
    assert_eq!(report.manifest.source_blockers.len(), 5);
    assert!(report.manifest.source_blockers.iter().any(|blocker| {
        blocker.blocker_id == "officeqa_category_split_manifest"
            && blocker.status == SourceBlockerStatus::MissingLocalArtifact
            && blocker.local_path_candidates.iter().any(|candidate| {
                candidate.relative_path.ends_with("solved_dataset.csv") && !candidate.exists
            })
    }));
    assert!(report.manifest.source_blockers.iter().any(|blocker| {
        blocker.blocker_id == "sealqa_split_manifest"
            && blocker.status == SourceBlockerStatus::MissingExactSplitManifest
    }));
}

fn assert_officeqa_paper_targets_report_ambiguity_without_blocking(report: &EvoSkillFinalReport) {
    assert_eq!(report.manifest.paper_result_targets.len(), 7);
    assert!(
        report
            .errors
            .iter()
            .all(|error| error.blocker_id != "officeqa_reported_result_target")
    );
    let ambiguous_targets = report
        .manifest
        .paper_result_targets
        .iter()
        .filter(|target| target.status == PaperResultTargetStatus::AmbiguousCandidate)
        .collect::<Vec<_>>();
    assert_eq!(ambiguous_targets.len(), 2);
    assert!(ambiguous_targets.iter().all(|target| {
        target.candidate_role == "skill_merge"
            && target.ambiguity_group.as_deref() == Some("officeqa_skill_merge_exact_match")
    }));

    assert_reported_target(
        report,
        "sealqa_baseline_accuracy",
        "sealqa",
        "baseline",
        "llm_judge_accuracy",
        26.6,
    );
    assert_reported_target(
        report,
        "sealqa_optimized_accuracy",
        "sealqa",
        "optimized",
        "llm_judge_accuracy",
        38.7,
    );
    assert_reported_target(
        report,
        "browsecomp_baseline_accuracy",
        "browsecomp_transfer",
        "baseline",
        "accuracy",
        43.5,
    );
    assert_reported_target(
        report,
        "browsecomp_sealqa_skill_transfer_accuracy",
        "browsecomp_transfer",
        "sealqa_skill_transfer",
        "accuracy",
        48.8,
    );
}

fn assert_reported_target(
    report: &EvoSkillFinalReport,
    id: &str,
    dataset_id: &str,
    candidate_role: &str,
    metric: &str,
    value_percent: f64,
) {
    let target = report
        .manifest
        .paper_result_targets
        .iter()
        .find(|target| target.id == id)
        .unwrap_or_else(|| panic!("missing paper result target {id}"));
    assert_eq!(target.status, PaperResultTargetStatus::Reported);
    assert_eq!(target.dataset_id, dataset_id);
    assert_eq!(target.candidate_role, candidate_role);
    assert_eq!(target.metric, metric);
    assert!((target.value_percent - value_percent).abs() < f64::EPSILON);
    assert_eq!(target.ambiguity_group, None);
}

fn assert_report_header(report: &EvoSkillFinalReport) {
    assert_eq!(report.exactness, ExactnessClass::BlockedBeforePaperClose);
    assert_eq!(report.schema_version, 19);
    assert_eq!(report.score_result_manifest, None);
    assert!(report.exactness_gaps.iter().any(|gap| gap.status
        == ExactnessGapStatus::BlockedBeforePaperClose
        && gap.id == "source_blocker_source_pin"));
    assert_eq!(report.cost.llm_calls, 0);
    assert_eq!(report.cost.metric_calls, 0);
    assert_eq!(report.cost.prompt_tokens, 0);
    assert_eq!(report.cost.completion_tokens, 0);
    let loop_report = report
        .loop_report
        .as_ref()
        .expect("OfficeQA mechanics loop should run when the CSV is present");
    assert!(loop_report.proxy_rejection.contains("mechanics only"));
    assert_eq!(
        loop_report.run_manifest.manifest_fingerprint,
        report.manifest_fingerprint.fingerprint
    );
    assert_eq!(
        loop_report.run_manifest.scorer_fingerprint,
        report.scorer_fingerprint.fingerprint
    );
    assert_loop_manifest_matches_embedded_officeqa_materialization(report);
    assert!(
        loop_report
            .run_manifest
            .validation_score_source
            .contains("not paper scorer output")
    );
    assert!(
        loop_report
            .run_manifest
            .proof_limit
            .contains("not live provider")
    );

    assert_eq!(
        report.manifest.scorer.id,
        report.scorer_fingerprint.scorer_id
    );
    let sealqa_judge = report
        .manifest
        .scorer
        .judge_templates
        .iter()
        .find(|template| template.id == "sealqa-auto-grader-placeholder-v1")
        .expect("SealQA judge template is fingerprinted in the scorer manifest");
    assert_eq!(sealqa_judge.runtime_status, "template_pinned_no_spend");
    assert!(sealqa_judge.source_artifact_exists);
    assert!(sealqa_judge.source_artifact_bytes.unwrap() > 20);
    assert_eq!(
        sealqa_judge
            .source_artifact_sha256
            .as_deref()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(sealqa_judge.fingerprint.len(), 64);
    assert_eq!(
        report.manifest_fingerprint.schema_version,
        report.manifest.schema_version
    );
    assert_eq!(report.manifest_fingerprint.fingerprint.len(), 64);
}

fn assert_loop_manifest_matches_embedded_officeqa_materialization(report: &EvoSkillFinalReport) {
    let loop_manifest = &report
        .loop_report
        .as_ref()
        .expect("OfficeQA mechanics loop should run")
        .run_manifest;
    let officeqa = officeqa_materialization(report);
    assert_eq!(
        loop_manifest.source_row_fingerprint,
        officeqa.source_row_fingerprint.as_deref().unwrap()
    );
    let split = officeqa
        .split_materializations
        .iter()
        .find(|split| split.id == loop_manifest.train_split_id)
        .expect("loop train split should be embedded in the manifest");
    assert_eq!(
        loop_manifest.train_split_exactness,
        MaterializationExactness::PaperCloseSubstitute
    );
    assert_eq!(
        loop_manifest.train_split_fingerprint,
        split.split_fingerprint.as_deref().unwrap()
    );
    let train = split
        .role_manifests
        .iter()
        .find(|manifest| manifest.role == "train")
        .expect("loop train role should be embedded in the manifest");
    assert_eq!(
        loop_manifest.train_role_source_id_fingerprint,
        train.source_id_fingerprint
    );
    assert_eq!(loop_manifest.train_rows, train.rows);
    assert_eq!(loop_manifest.validation_split_id, split.id);
    assert_eq!(
        loop_manifest.validation_split_fingerprint,
        split.split_fingerprint.as_deref().unwrap()
    );
    let validation = split
        .role_manifests
        .iter()
        .find(|manifest| manifest.role == "validation")
        .expect("loop validation role should be embedded in the manifest");
    assert_eq!(
        loop_manifest.validation_role_source_id_fingerprint,
        validation.source_id_fingerprint
    );
    assert_eq!(loop_manifest.validation_rows, validation.rows);
    assert!(
        loop_manifest
            .validation_policy
            .contains("full OfficeQA validation role")
    );
}

fn assert_proxy_rejection_gates(report: &EvoSkillFinalReport) {
    assert_eq!(report.proxy_rejection_gates.len(), 5);
    assert!(
        report
            .proxy_rejection_gates
            .iter()
            .all(|gate| { gate.status == ProxyRejectionStatus::RejectedAsCompletionEvidence })
    );

    let gate = |id: &str| {
        report
            .proxy_rejection_gates
            .iter()
            .find(|gate| gate.id == id)
            .unwrap_or_else(|| panic!("missing proxy rejection gate {id}"))
    };

    assert!(
        gate("p5_one_iteration_fixture")
            .why_not
            .contains("OfficeQA/SealQA paper-close")
    );
    assert!(
        gate("git_trust_benchmark")
            .why_not
            .contains("not EvoSkill loop semantics")
    );
    assert!(
        gate("fake_runtime_loop")
            .why_not
            .contains("not live agent behavior")
    );
    assert!(
        gate("single_sample_inspection")
            .why_not
            .contains("train/validation/test")
    );
    assert!(
        gate("just_check_repo_health")
            .why_not
            .contains("repo health only")
    );
}

fn assert_live_run_gate_blocks_unapproved_spend(report: &EvoSkillFinalReport) {
    assert_eq!(
        report.live_run_gate.status,
        LiveRunGateStatus::BlockedNoSpendApproval
    );
    assert_eq!(report.live_run_gate.runtime_role, "paper_agent_runtime");
    assert_eq!(
        report.live_run_gate.candidate_model.as_deref(),
        Some("Codex gpt-5.4-mini low for approved small live runs")
    );
    assert_eq!(
        report.live_run_gate.credential_probe_status,
        "not_probed_no_spend_default"
    );
    assert_eq!(report.live_run_gate.spend_approval_status, "not_approved");
    assert_eq!(
        report.live_run_gate.blocker_ids,
        ["live_run_spend_approval".to_owned()]
    );
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.blocker_id == "live_run_spend_approval")
    );
}

fn officeqa_materialization(report: &EvoSkillFinalReport) -> &DatasetMaterializationReport {
    let officeqa = report
        .manifest
        .source_materializations
        .iter()
        .find(|materialization| materialization.dataset_id == "officeqa")
        .expect("OfficeQA materialization is embedded");
    assert_eq!(officeqa.source_rows, Some(246));
    assert_eq!(
        officeqa.source_row_fingerprint.as_deref().unwrap().len(),
        64
    );
    officeqa
}

fn sealqa_materialization(report: &EvoSkillFinalReport) -> &DatasetMaterializationReport {
    let sealqa = report
        .manifest
        .source_materializations
        .iter()
        .find(|materialization| materialization.dataset_id == "sealqa")
        .expect("SealQA materialization is embedded");
    assert_eq!(sealqa.source_rows, Some(111));
    assert_eq!(sealqa.source_row_fingerprint.as_deref().unwrap().len(), 64);
    sealqa
}

fn assert_officeqa_train_12_score_slots(
    report: &EvoSkillFinalReport,
    officeqa: &DatasetMaterializationReport,
) {
    let train_12_split = officeqa
        .split_materializations
        .iter()
        .find(|split| split.id == "officeqa_difficulty_train_12_val_17")
        .expect("OfficeQA train-12 substitute split is embedded");
    let train_12_slots = report
        .score_slots
        .iter()
        .filter(|slot| {
            slot.dataset_id == "officeqa" && slot.split_id == "officeqa_difficulty_train_12_val_17"
        })
        .collect::<Vec<_>>();
    assert_eq!(train_12_slots.len(), 6);
    for role in ["train", "validation", "held_out_test"] {
        for candidate in ["baseline", "optimized"] {
            let slot = train_12_slots
                .iter()
                .find(|slot| slot.split_role == role && slot.candidate_role == candidate)
                .unwrap_or_else(|| panic!("missing {candidate} {role} slot"));
            assert_officeqa_blocked_slot(slot, train_12_split, role);
        }
    }
}

fn assert_officeqa_blocked_slot(
    slot: &FinalScoreSlot,
    split: &SplitMaterializationReport,
    role: &str,
) {
    assert_materialized_blocked_slot(slot, split, role);
    match slot.candidate_role.as_str() {
        "baseline" => assert_slot_target_ids(slot, &["officeqa_baseline_exact_match_table"]),
        "optimized" => assert_slot_target_ids(
            slot,
            &[
                "officeqa_skill_merge_exact_match_prose",
                "officeqa_skill_merge_exact_match_table",
            ],
        ),
        other => panic!("unexpected OfficeQA score candidate role {other}"),
    }
    assert!(
        slot.blocker_ids
            .contains(&"officeqa_exact_split_membership".to_owned())
    );
}

fn assert_sealqa_score_slots(report: &EvoSkillFinalReport, sealqa: &DatasetMaterializationReport) {
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.blocker_id == "sealqa_split_manifest")
    );
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.blocker_id == "sealqa_judge_scored_run")
    );
    let split = sealqa
        .split_materializations
        .iter()
        .find(|split| split.id == "sealqa_row_order_train_11_heldout_100")
        .expect("SealQA substitute split is embedded");
    let sealqa_slots = report
        .score_slots
        .iter()
        .filter(|slot| slot.dataset_id == "sealqa")
        .collect::<Vec<_>>();
    assert_eq!(sealqa_slots.len(), 4);
    assert!(sealqa_slots.iter().all(|slot| {
        slot.blocker_ids
            .contains(&"sealqa_judge_scored_run".to_owned())
    }));
    for role in ["train", "held_out_test"] {
        for candidate in ["baseline", "optimized"] {
            let slot = sealqa_slots
                .iter()
                .find(|slot| slot.split_role == role && slot.candidate_role == candidate)
                .unwrap_or_else(|| panic!("missing SealQA {candidate} {role} slot"));
            assert_materialized_blocked_slot(slot, split, role);
            match candidate {
                "baseline" => assert_slot_target_ids(slot, &["sealqa_baseline_accuracy"]),
                "optimized" => assert_slot_target_ids(slot, &["sealqa_optimized_accuracy"]),
                _ => unreachable!("test only iterates known SealQA candidate roles"),
            }
        }
    }
}

fn assert_browsecomp_transfer_score_slots(report: &EvoSkillFinalReport) {
    let browsecomp_slots = report
        .score_slots
        .iter()
        .filter(|slot| slot.dataset_id == "browsecomp_transfer")
        .collect::<Vec<_>>();
    assert_eq!(browsecomp_slots.len(), 2);
    assert!(
        browsecomp_slots
            .iter()
            .all(|slot| slot.split_role == "held_out_test")
    );
    assert!(
        browsecomp_slots
            .iter()
            .all(|slot| slot.split_id == "browsecomp_transfer_paper_split_unmaterialized")
    );
    for candidate in ["baseline", "sealqa_skill_transfer"] {
        let slot = browsecomp_slots
            .iter()
            .find(|slot| slot.candidate_role == candidate)
            .unwrap_or_else(|| panic!("missing BrowseComp {candidate} held-out slot"));
        assert_eq!(slot.split_exactness, MaterializationExactness::Blocked);
        assert_eq!(slot.split_fingerprint, None);
        assert_eq!(slot.role_source_id_fingerprint, None);
        assert_eq!(slot.expected_rows, Some(128));
        assert_eq!(slot.status, FinalScoreStatus::Blocked);
        assert!(slot.score.is_none());
        assert!(
            slot.blocker_ids
                .contains(&"browsecomp_transfer_sample".to_owned())
        );
        match candidate {
            "baseline" => assert_slot_target_ids(slot, &["browsecomp_baseline_accuracy"]),
            "sealqa_skill_transfer" => {
                assert_slot_target_ids(slot, &["browsecomp_sealqa_skill_transfer_accuracy"]);
            }
            _ => unreachable!("test only iterates known BrowseComp candidate roles"),
        }
    }
}

fn assert_materialized_blocked_slot(
    slot: &FinalScoreSlot,
    split: &SplitMaterializationReport,
    role: &str,
) {
    let role_manifest = split
        .role_manifests
        .iter()
        .find(|manifest| manifest.role == role)
        .unwrap_or_else(|| panic!("missing source-id manifest for {role}"));
    assert_eq!(
        slot.split_exactness,
        MaterializationExactness::PaperCloseSubstitute
    );
    assert_eq!(slot.split_fingerprint, split.split_fingerprint);
    assert_eq!(
        slot.role_source_id_fingerprint.as_deref(),
        Some(role_manifest.source_id_fingerprint.as_str())
    );
    assert_eq!(slot.status, FinalScoreStatus::Blocked);
    assert!(
        slot.score.is_none(),
        "blocked slots must not fake zero scores"
    );
}

fn assert_slot_target_ids(slot: &FinalScoreSlot, expected: &[&str]) {
    let actual = slot
        .paper_result_target_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn cli_writes_manifest_and_final_report_artifacts() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    let manifest_path = root.path().join("out/replica-manifest.json");
    let report_path = root.path().join("out/final-report.json");

    let output = Command::new(env!("CARGO_BIN_EXE_p5_skill_paper_reproductions"))
        .arg("--root")
        .arg(root.path())
        .arg("--out")
        .arg(&manifest_path)
        .arg("--final-report-out")
        .arg(&report_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(manifest_path.exists());
    let report = fs::read_to_string(report_path).unwrap();
    assert!(report.contains("\"score_slots\""));
    assert!(report.contains("\"exactness_gaps\""));
    assert!(report.contains("\"live_run_gate\""));
    assert!(report.contains("\"paper_close_gates\""));
    assert!(report.contains("\"source_blockers\""));
    assert!(report.contains("\"proxy_rejection_gates\""));
    assert!(report.contains("\"paper_result_targets\""));
    assert!(report.contains("\"paper_result_target_ids\""));
    assert!(report.contains("\"role_source_id_fingerprint\""));
    assert!(report.contains("\"score_result_manifest\""));
    assert!(report.contains("\"score_evidence_id\""));
    assert!(report.contains("\"score_evidence_kind\""));
    assert!(report.contains("\"score_evidence_approval_id\""));
    assert!(report.contains("\"score_evidence_artifact\""));
    assert!(report.contains("\"blocked_before_paper_close\""));
}

#[test]
fn cli_can_persist_local_source_pin_sidecar_before_writing_manifest() {
    let root = tempfile::tempdir().unwrap();
    init_git_source(
        &root.path().join("tmp/repros/evoskill"),
        "https://github.com/sentient-agi/EvoSkill.git",
    );
    init_git_source(
        &root.path().join("tmp/repros/officeqa"),
        "https://github.com/databricks/officeqa.git",
    );
    let manifest_path = root.path().join("out/replica-manifest.json");

    let output = Command::new(env!("CARGO_BIN_EXE_p5_skill_paper_reproductions"))
        .arg("--root")
        .arg(root.path())
        .arg("--out")
        .arg(&manifest_path)
        .arg("--write-local-source-pin-manifest")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let source_pin_path = root
        .path()
        .join("tmp/replication/evoskill/source_pin_manifest.json");
    assert!(source_pin_path.exists());
    let source_pin = fs::read_to_string(source_pin_path).unwrap();
    assert!(source_pin.contains("\"local_checkout_pinned\""));

    let manifest = fs::read_to_string(manifest_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    assert!(
        manifest["source_blockers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|blocker| blocker["blocker_id"] != "source_pin")
    );
    assert!(
        manifest["source_revisions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|revision| revision["paper_release_status"] == "pinned_local_checkout")
    );
}

#[test]
fn cli_can_persist_paper_close_split_policy_sidecar_before_writing_manifest() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    write_sealqa_parquet(root.path(), 111);
    let manifest_path = root.path().join("out/replica-manifest.json");

    let output = Command::new(env!("CARGO_BIN_EXE_p5_skill_paper_reproductions"))
        .arg("--root")
        .arg(root.path())
        .arg("--out")
        .arg(&manifest_path)
        .arg("--write-paper-close-split-policy-manifest")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let split_policy_path = root
        .path()
        .join("tmp/replication/evoskill/split_policy_manifest.json");
    assert!(split_policy_path.exists());
    let split_policy = fs::read_to_string(split_policy_path).unwrap();
    assert!(split_policy.contains("\"accept_documented_paper_close_substitutes\""));

    let manifest = fs::read_to_string(manifest_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    assert!(
        manifest["source_blockers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|blocker| blocker["dataset_id"] != "officeqa"
                && blocker["dataset_id"] != "sealqa")
    );
    assert!(
        manifest["source_materializations"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|materialization| {
                materialization["split_materializations"]
                    .as_array()
                    .unwrap()
                    .iter()
            })
            .filter(|split| split["exactness"] == "paper_close_substitute")
            .all(|split| split["acceptance_status"] == "accepted_paper_close_policy")
    );
}

#[test]
fn cli_writes_browsecomp_public_sample_before_split_policy() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    write_sealqa_parquet(root.path(), 111);
    write_browsecomp_public_csv(root.path(), 256);
    let manifest_path = root.path().join("out/replica-manifest.json");

    let output = Command::new(env!("CARGO_BIN_EXE_p5_skill_paper_reproductions"))
        .arg("--root")
        .arg(root.path())
        .arg("--out")
        .arg(&manifest_path)
        .arg("--write-browsecomp-public-transfer-sample")
        .arg("tmp/replication/evoskill/browsecomp/public_browsecomp_test_set.csv")
        .arg("--write-paper-close-split-policy-manifest")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest = fs::read_to_string(manifest_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    assert!(
        manifest["source_blockers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|blocker| blocker["blocker_id"] != "browsecomp_transfer_sample")
    );
    let browsecomp = manifest["source_materializations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|materialization| materialization["dataset_id"] == "browsecomp_transfer")
        .expect("BrowseComp transfer materialization exists");
    assert_eq!(browsecomp["source_rows"], 128);
    assert_eq!(
        browsecomp["split_materializations"][0]["acceptance_status"],
        "accepted_paper_close_policy"
    );
}

#[test]
fn cli_writes_officeqa_score_result_sidecar_from_prediction_rows() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let slot = score_slot(
        &initial_report,
        "officeqa",
        "officeqa_difficulty_train_12_val_17",
        "train",
        "baseline",
    );
    assert_eq!(slot.status, FinalScoreStatus::NotRun);
    let predictions_path = root
        .path()
        .join("tmp/replication/evoskill/predictions/officeqa-train-baseline.jsonl");
    write_officeqa_prediction_rows(root.path(), &initial_report, slot, &predictions_path);

    let manifest_path = root.path().join("out/replica-manifest.json");
    let report_path = root.path().join("out/final-report.json");
    let output = Command::new(env!("CARGO_BIN_EXE_p5_skill_paper_reproductions"))
        .arg("--root")
        .arg(root.path())
        .arg("--out")
        .arg(&manifest_path)
        .arg("--final-report-out")
        .arg(&report_path)
        .arg("--write-officeqa-score-result")
        .arg("tmp/replication/evoskill/predictions/officeqa-train-baseline.jsonl")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let score_result_path = root
        .path()
        .join("tmp/replication/evoskill/score_result_manifest.json");
    assert!(score_result_path.exists());
    let report = fs::read_to_string(report_path).unwrap();
    let report: EvoSkillFinalReport = serde_json::from_str(&report).unwrap();
    let reported = score_slot(
        &report,
        "officeqa",
        "officeqa_difficulty_train_12_val_17",
        "train",
        "baseline",
    );
    assert_eq!(reported.status, FinalScoreStatus::Reported);
    assert_eq!(reported.score, Some(1.0));
    assert_eq!(
        reported.score_evidence_kind,
        Some(ScoreEvidenceKind::RustScorerReplay)
    );
    assert_eq!(reported.score_evidence_approval_id, None);
    let evidence_artifact = reported
        .score_evidence_artifact
        .as_ref()
        .expect("CLI writer preserves checked score evidence artifact");
    let evidence_body = fs::read_to_string(root.path().join(&evidence_artifact.relative_path))
        .expect("score evidence artifact is readable");
    assert!(evidence_body.contains("\"prediction\""));
    assert!(!evidence_body.contains("ground_truth"));
    assert!(!evidence_body.contains("reference"));
}

#[test]
fn cli_refuses_officeqa_score_result_sidecar_for_blocked_split_slot() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    let input = ManifestBuildInput::new(root.path());
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let slot = score_slot(
        &initial_report,
        "officeqa",
        "officeqa_difficulty_train_12_val_17",
        "train",
        "baseline",
    );
    assert_eq!(slot.status, FinalScoreStatus::Blocked);
    assert!(
        slot.blocker_ids
            .contains(&"officeqa_exact_split_membership".to_owned())
    );
    let predictions_path = root
        .path()
        .join("tmp/replication/evoskill/predictions/officeqa-train-baseline.jsonl");
    write_officeqa_prediction_rows(root.path(), &initial_report, slot, &predictions_path);

    let output = Command::new(env!("CARGO_BIN_EXE_p5_skill_paper_reproductions"))
        .arg("--root")
        .arg(root.path())
        .arg("--out")
        .arg(root.path().join("out/replica-manifest.json"))
        .arg("--write-officeqa-score-result")
        .arg("tmp/replication/evoskill/predictions/officeqa-train-baseline.jsonl")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "blocked score writer unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("refuses blocked slot"),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !root
            .path()
            .join("tmp/replication/evoskill/score_result_manifest.json")
            .exists()
    );
}

#[test]
fn cli_writes_sealqa_judge_score_result_sidecar_from_approved_rows() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let slot = score_slot(
        &initial_report,
        "sealqa",
        "sealqa_row_order_train_11_heldout_100",
        "train",
        "baseline",
    );
    assert_eq!(slot.status, FinalScoreStatus::Blocked);
    assert_eq!(slot.blocker_ids, ["sealqa_judge_scored_run".to_owned()]);
    let rows_path = root
        .path()
        .join("tmp/replication/evoskill/judge/sealqa-train-baseline.jsonl");
    write_sealqa_judge_rows(root.path(), &initial_report, slot, &rows_path, 1.0);

    let manifest_path = root.path().join("out/replica-manifest.json");
    let report_path = root.path().join("out/final-report.json");
    let output = Command::new(env!("CARGO_BIN_EXE_p5_skill_paper_reproductions"))
        .arg("--root")
        .arg(root.path())
        .arg("--out")
        .arg(&manifest_path)
        .arg("--final-report-out")
        .arg(&report_path)
        .arg("--write-sealqa-judge-score-result")
        .arg("tmp/replication/evoskill/judge/sealqa-train-baseline.jsonl")
        .arg("--sealqa-judge-approval-id")
        .arg("unit-test-approved-sealqa-judge-run")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let score_result_path = root
        .path()
        .join("tmp/replication/evoskill/score_result_manifest.json");
    assert!(score_result_path.exists());
    let report = fs::read_to_string(report_path).unwrap();
    let report: EvoSkillFinalReport = serde_json::from_str(&report).unwrap();
    let reported = score_slot(
        &report,
        "sealqa",
        "sealqa_row_order_train_11_heldout_100",
        "train",
        "baseline",
    );
    assert_eq!(reported.status, FinalScoreStatus::Reported);
    assert_eq!(reported.score, Some(1.0));
    assert_eq!(
        reported.score_evidence_kind,
        Some(ScoreEvidenceKind::ExternalJudgeRun)
    );
    assert_eq!(
        reported.score_evidence_approval_id.as_deref(),
        Some("unit-test-approved-sealqa-judge-run")
    );
    assert!(reported.blocker_ids.is_empty());
    let evidence_id = reported
        .score_evidence_id
        .as_deref()
        .expect("SealQA writer records evidence id");
    assert!(
        evidence_id.starts_with("sealqa-external-judge-run-unit-test-approved-sealqa-judge-run")
    );
    let evidence_artifact = reported
        .score_evidence_artifact
        .as_ref()
        .expect("CLI writer preserves checked score evidence artifact");
    let evidence_body = fs::read_to_string(root.path().join(&evidence_artifact.relative_path))
        .expect("score evidence artifact is readable");
    assert!(evidence_body.contains("\"judge_template_fingerprint\""));
    assert!(!evidence_body.contains("ground_truth"));
    assert!(!evidence_body.contains("reference"));
}

#[test]
fn cli_refuses_sealqa_judge_score_result_sidecar_without_approval_id() {
    let root = tempfile::tempdir().unwrap();
    write_denominator_ready_sources(root.path());
    let input = ManifestBuildInput::new(root.path());
    write_evoskill_local_source_pin_manifest(&input).unwrap();
    write_evoskill_paper_close_split_policy_manifest(&input).unwrap();
    let initial_report = build_evoskill_final_report(&input).unwrap();
    let slot = score_slot(
        &initial_report,
        "sealqa",
        "sealqa_row_order_train_11_heldout_100",
        "train",
        "baseline",
    );
    let rows_path = root
        .path()
        .join("tmp/replication/evoskill/judge/sealqa-train-baseline.jsonl");
    write_sealqa_judge_rows(root.path(), &initial_report, slot, &rows_path, 1.0);

    let output = Command::new(env!("CARGO_BIN_EXE_p5_skill_paper_reproductions"))
        .arg("--root")
        .arg(root.path())
        .arg("--out")
        .arg(root.path().join("out/replica-manifest.json"))
        .arg("--write-sealqa-judge-score-result")
        .arg("tmp/replication/evoskill/judge/sealqa-train-baseline.jsonl")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "SealQA judge writer unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("approval id"),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !root
            .path()
            .join("tmp/replication/evoskill/score_result_manifest.json")
            .exists()
    );
}

fn write_denominator_ready_sources(root: &std::path::Path) {
    write_officeqa_full_csv(root, 246);
    write_sealqa_parquet(root, 111);
    write_sealqa_judge_source(root);
    write_browsecomp_transfer_sample(root, 128);
    init_git_source(
        &root.join("tmp/repros/evoskill"),
        "https://github.com/sentient-agi/EvoSkill.git",
    );
    init_git_source(
        &root.join("tmp/repros/officeqa"),
        "https://github.com/databricks/officeqa.git",
    );
}

fn write_officeqa_prediction_rows(
    root: &std::path::Path,
    report: &EvoSkillFinalReport,
    slot: &FinalScoreSlot,
    path: &std::path::Path,
) {
    let mut jsonl = String::new();
    for source_id in score_slot_source_ids(report, slot) {
        let row_number = source_id
            .strip_prefix("UID")
            .expect("OfficeQA fixture source ids are UID-prefixed")
            .parse::<usize>()
            .expect("OfficeQA fixture source ids end in row numbers");
        writeln!(
            jsonl,
            "{}",
            serde_json::json!({
                "dataset_id": &slot.dataset_id,
                "split_id": &slot.split_id,
                "split_role": &slot.split_role,
                "candidate_role": &slot.candidate_role,
                "source_id": source_id,
                "prediction": format!("Answer {row_number}")
            })
        )
        .unwrap();
    }
    let relative_path = path
        .strip_prefix(root)
        .expect("test prediction path lives below root");
    let path = root.join(relative_path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, jsonl).unwrap();
}

fn write_sealqa_judge_rows(
    root: &std::path::Path,
    report: &EvoSkillFinalReport,
    slot: &FinalScoreSlot,
    path: &std::path::Path,
    score: f64,
) {
    let judge_template_fingerprint = judge_template_fingerprint_for_dataset(report, "sealqa");
    let mut jsonl = String::new();
    for source_id in score_slot_source_ids(report, slot) {
        writeln!(
            jsonl,
            "{}",
            serde_json::json!({
                "dataset_id": &slot.dataset_id,
                "split_id": &slot.split_id,
                "split_role": &slot.split_role,
                "candidate_role": &slot.candidate_role,
                "source_id": source_id,
                "prediction": score_artifact_prediction_for_source_id(slot, &source_id),
                "score": score,
                "judge_template_fingerprint": judge_template_fingerprint
            })
        )
        .unwrap();
    }
    let relative_path = path
        .strip_prefix(root)
        .expect("test judged-row path lives below root");
    let path = root.join(relative_path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, jsonl).unwrap();
}

fn score_slot<'a>(
    report: &'a EvoSkillFinalReport,
    dataset_id: &str,
    split_id: &str,
    split_role: &str,
    candidate_role: &str,
) -> &'a FinalScoreSlot {
    report
        .score_slots
        .iter()
        .find(|slot| {
            slot.dataset_id == dataset_id
                && slot.split_id == split_id
                && slot.split_role == split_role
                && slot.candidate_role == candidate_role
        })
        .unwrap_or_else(|| {
            panic!("missing score slot {dataset_id}|{split_id}|{split_role}|{candidate_role}")
        })
}

fn assert_reported_score_evidence(
    slot: &FinalScoreSlot,
    expected_kind: ScoreEvidenceKind,
    expected_approval_id: Option<&str>,
) {
    assert_eq!(
        slot.score_evidence_id.as_deref(),
        Some("unit-test-scored-output-import")
    );
    assert_eq!(slot.score_evidence_kind, Some(expected_kind));
    assert_eq!(
        slot.score_evidence_approval_id.as_deref(),
        expected_approval_id
    );
}

fn write_score_result_manifest(
    root: &std::path::Path,
    report: &EvoSkillFinalReport,
    slot: &FinalScoreSlot,
    score: f64,
) {
    write_score_result_manifest_with_split_fingerprint(
        root,
        report,
        slot,
        slot.split_fingerprint
            .as_deref()
            .expect("materialized score slot carries split fingerprint"),
        score,
    );
}

fn write_score_result_manifest_with_split_fingerprint(
    root: &std::path::Path,
    report: &EvoSkillFinalReport,
    slot: &FinalScoreSlot,
    split_fingerprint: &str,
    score: f64,
) {
    write_score_result_manifest_with_options(
        root,
        report,
        slot,
        split_fingerprint,
        score,
        None,
        ScoreResultEvidenceOptions::new(
            default_score_evidence_kind(slot),
            default_score_evidence_approval_id(slot),
        ),
    );
}

fn write_score_result_manifest_with_evidence_kind(
    root: &std::path::Path,
    report: &EvoSkillFinalReport,
    slot: &FinalScoreSlot,
    score: f64,
    evidence_kind: ScoreEvidenceKind,
    approval_id: Option<&str>,
) {
    write_score_result_manifest_with_options(
        root,
        report,
        slot,
        slot.split_fingerprint
            .as_deref()
            .expect("materialized score slot carries split fingerprint"),
        score,
        None,
        ScoreResultEvidenceOptions::new(evidence_kind, approval_id),
    );
}

fn write_score_result_manifest_with_judge_template_fingerprint(
    root: &std::path::Path,
    report: &EvoSkillFinalReport,
    slot: &FinalScoreSlot,
    score: f64,
    evidence_kind: ScoreEvidenceKind,
    approval_id: Option<&str>,
    judge_template_fingerprint: Option<&str>,
) {
    write_score_result_manifest_with_options(
        root,
        report,
        slot,
        slot.split_fingerprint
            .as_deref()
            .expect("materialized score slot carries split fingerprint"),
        score,
        None,
        ScoreResultEvidenceOptions::new(evidence_kind, approval_id)
            .with_judge_template_fingerprint(judge_template_fingerprint),
    );
}

fn write_score_result_manifest_for_slots_with_judge_template_fingerprint(
    root: &std::path::Path,
    report: &EvoSkillFinalReport,
    slots: &[FinalScoreSlot],
    score: f64,
    evidence_kind: ScoreEvidenceKind,
    approval_id: Option<&str>,
    judge_template_fingerprint: Option<&str>,
) {
    let path = root.join("tmp/replication/evoskill/score_result_manifest.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut entries = Vec::new();
    let mut llm_calls = 0_u64;
    let mut metric_calls = 0_u64;
    for (index, slot) in slots.iter().enumerate() {
        let expected_rows = slot.expected_rows.unwrap();
        let evidence_relative_path = format!(
            "tmp/replication/evoskill/score-evidence/unit-test-scored-output-import-{index}.jsonl"
        );
        let evidence_path = root.join(&evidence_relative_path);
        fs::create_dir_all(evidence_path.parent().unwrap()).unwrap();
        let mut evidence_body = String::new();
        for source_id in score_slot_source_ids(report, slot) {
            let prediction = score_artifact_prediction_for_source_id(slot, &source_id);
            let mut row = serde_json::json!({
                "source_id": source_id,
                "prediction": prediction,
                "score": score
            });
            if let Some(fingerprint) = judge_template_fingerprint {
                row.as_object_mut()
                    .expect("score evidence row is a JSON object")
                    .insert(
                        "judge_template_fingerprint".to_owned(),
                        serde_json::json!(fingerprint),
                    );
            }
            writeln!(evidence_body, "{row}").unwrap();
        }
        fs::write(&evidence_path, evidence_body.as_bytes()).unwrap();
        let evidence_sha256 = sha256_bytes(evidence_body.as_bytes());
        if evidence_kind == ScoreEvidenceKind::ExternalJudgeRun {
            llm_calls += expected_rows;
        }
        metric_calls += expected_rows;
        entries.push(serde_json::json!({
            "dataset_id": &slot.dataset_id,
            "split_id": &slot.split_id,
            "split_role": &slot.split_role,
            "candidate_role": &slot.candidate_role,
            "split_fingerprint": slot
                .split_fingerprint
                .as_deref()
                .expect("materialized score slot carries split fingerprint"),
            "role_source_id_fingerprint": &slot.role_source_id_fingerprint,
            "expected_rows": expected_rows,
            "scored_rows": expected_rows,
            "score": score,
            "resolved_blocker_ids": &slot.blocker_ids,
            "score_evidence_kind": evidence_kind,
            "score_evidence_approval_id": approval_id,
            "evidence_id": format!("unit-test-scored-output-import-{index}"),
            "evidence_artifact": {
                "relative_path": evidence_relative_path,
                "sha256": evidence_sha256,
                "bytes": evidence_body.len()
            }
        }));
    }
    let manifest = serde_json::json!({
        "schema_version": 5,
        "manifest_fingerprint": &report.manifest_fingerprint.fingerprint,
        "scorer_fingerprint": &report.scorer_fingerprint.fingerprint,
        "cost": {
            "llm_calls": llm_calls,
            "metric_calls": metric_calls,
            "prompt_tokens": 0,
            "completion_tokens": 0
        },
        "entries": entries
    });
    fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

fn write_score_result_manifest_with_prediction_override(
    root: &std::path::Path,
    report: &EvoSkillFinalReport,
    slot: &FinalScoreSlot,
    prediction: &str,
    score: f64,
) {
    write_score_result_manifest_with_options(
        root,
        report,
        slot,
        slot.split_fingerprint
            .as_deref()
            .expect("materialized score slot carries split fingerprint"),
        score,
        Some(prediction),
        ScoreResultEvidenceOptions::new(
            default_score_evidence_kind(slot),
            default_score_evidence_approval_id(slot),
        ),
    );
}

#[derive(Clone, Copy)]
struct ScoreResultEvidenceOptions<'a> {
    kind: ScoreEvidenceKind,
    approval_id: Option<&'a str>,
    judge_template_fingerprint: Option<&'a str>,
}

impl<'a> ScoreResultEvidenceOptions<'a> {
    fn new(kind: ScoreEvidenceKind, approval_id: Option<&'a str>) -> Self {
        Self {
            kind,
            approval_id,
            judge_template_fingerprint: None,
        }
    }

    fn with_judge_template_fingerprint(
        mut self,
        judge_template_fingerprint: Option<&'a str>,
    ) -> Self {
        self.judge_template_fingerprint = judge_template_fingerprint;
        self
    }
}

fn write_score_result_manifest_with_options(
    root: &std::path::Path,
    report: &EvoSkillFinalReport,
    slot: &FinalScoreSlot,
    split_fingerprint: &str,
    score: f64,
    prediction_override: Option<&str>,
    evidence: ScoreResultEvidenceOptions<'_>,
) {
    let path = root.join("tmp/replication/evoskill/score_result_manifest.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let expected_rows = slot.expected_rows.unwrap();
    let evidence_relative_path =
        "tmp/replication/evoskill/score-evidence/unit-test-scored-output-import.jsonl";
    let evidence_path = root.join(evidence_relative_path);
    fs::create_dir_all(evidence_path.parent().unwrap()).unwrap();
    let mut evidence_body = String::new();
    for source_id in score_slot_source_ids(report, slot) {
        let prediction = prediction_override
            .map(str::to_owned)
            .unwrap_or_else(|| score_artifact_prediction_for_source_id(slot, &source_id));
        let mut row = serde_json::json!({
            "source_id": source_id,
            "prediction": prediction,
            "score": score
        });
        if let Some(fingerprint) = evidence.judge_template_fingerprint {
            row.as_object_mut()
                .expect("score evidence row is a JSON object")
                .insert(
                    "judge_template_fingerprint".to_owned(),
                    serde_json::json!(fingerprint),
                );
        }
        writeln!(evidence_body, "{row}").unwrap();
    }
    fs::write(&evidence_path, evidence_body.as_bytes()).unwrap();
    let evidence_sha256 = sha256_bytes(evidence_body.as_bytes());
    let llm_calls = if evidence.kind == ScoreEvidenceKind::ExternalJudgeRun {
        expected_rows
    } else {
        0
    };
    let manifest = serde_json::json!({
        "schema_version": 5,
        "manifest_fingerprint": &report.manifest_fingerprint.fingerprint,
        "scorer_fingerprint": &report.scorer_fingerprint.fingerprint,
        "cost": {
            "llm_calls": llm_calls,
            "metric_calls": expected_rows,
            "prompt_tokens": 0,
            "completion_tokens": 0
        },
        "entries": [{
            "dataset_id": &slot.dataset_id,
            "split_id": &slot.split_id,
            "split_role": &slot.split_role,
            "candidate_role": &slot.candidate_role,
            "split_fingerprint": split_fingerprint,
            "role_source_id_fingerprint": &slot.role_source_id_fingerprint,
            "expected_rows": expected_rows,
            "scored_rows": expected_rows,
            "score": score,
            "resolved_blocker_ids": &slot.blocker_ids,
            "score_evidence_kind": evidence.kind,
            "score_evidence_approval_id": evidence.approval_id,
            "evidence_id": "unit-test-scored-output-import",
            "evidence_artifact": {
                "relative_path": evidence_relative_path,
                "sha256": evidence_sha256,
                "bytes": evidence_body.len()
            }
        }]
    });
    fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

fn default_score_evidence_kind(slot: &FinalScoreSlot) -> ScoreEvidenceKind {
    match slot.dataset_id.as_str() {
        "officeqa" => ScoreEvidenceKind::RustScorerReplay,
        "browsecomp_transfer" => ScoreEvidenceKind::ExactAnswerReplay,
        "sealqa" => ScoreEvidenceKind::ExternalJudgeRun,
        other => panic!("test helper has no score evidence kind for dataset {other}"),
    }
}

fn default_score_evidence_approval_id(slot: &FinalScoreSlot) -> Option<&'static str> {
    match slot.dataset_id.as_str() {
        "sealqa" => Some("unit-test-approved-sealqa-judge-run"),
        _ => None,
    }
}

fn judge_template_fingerprint_for_dataset<'a>(
    report: &'a EvoSkillFinalReport,
    dataset_id: &str,
) -> &'a str {
    report
        .manifest
        .scorer
        .judge_templates
        .iter()
        .find(|template| template.dataset_id == dataset_id)
        .expect("test report carries judge template for dataset")
        .fingerprint
        .as_str()
}

fn score_slot_source_ids(report: &EvoSkillFinalReport, slot: &FinalScoreSlot) -> Vec<String> {
    let materialization = report
        .manifest
        .source_materializations
        .iter()
        .find(|materialization| materialization.dataset_id == slot.dataset_id)
        .expect("score slot dataset materialization exists");
    let split = materialization
        .split_materializations
        .iter()
        .find(|split| split.id == slot.split_id)
        .expect("score slot split materialization exists");
    split
        .role_manifests
        .iter()
        .find(|role| role.role == slot.split_role)
        .expect("score slot role materialization exists")
        .source_ids
        .clone()
}

fn score_artifact_prediction_for_source_id(slot: &FinalScoreSlot, source_id: &str) -> String {
    match slot.dataset_id.as_str() {
        "officeqa" => {
            let row_number = source_id
                .strip_prefix("UID")
                .expect("OfficeQA fixture source ids are UID-prefixed")
                .parse::<usize>()
                .expect("OfficeQA fixture source ids end in row numbers");
            format!("Answer {row_number}")
        }
        "browsecomp_transfer" => {
            let row_number = source_id
                .strip_prefix("browsecomp:")
                .expect("BrowseComp fixture source ids are browsecomp-prefixed")
                .parse::<usize>()
                .expect("BrowseComp fixture source ids end in row numbers");
            format!("Browse answer {row_number}")
        }
        "sealqa" => format!("SealQA judged prediction for {source_id}"),
        other => panic!("test helper has no score prediction for dataset {other}"),
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn write_officeqa_full_csv(root: &std::path::Path, rows: usize) {
    let path = root.join("tmp/repros/officeqa/officeqa_full.csv");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut csv = String::from("uid,question,answer,source_docs,source_files,difficulty\n");
    for index in 0..rows {
        let uid = format!("UID{:04}", index + 1);
        let difficulty = if index < 113 { "easy" } else { "hard" };
        writeln!(
            csv,
            "{uid},Question {}?,Answer {},https://example.test/doc{},treasury_{:04}.txt,{difficulty}",
            index + 1,
            index + 1,
            index + 1,
            index + 1
        )
        .unwrap();
    }
    fs::write(path, csv).unwrap();
}

fn write_browsecomp_transfer_sample(root: &std::path::Path, rows: usize) {
    let path = root.join("tmp/replication/evoskill/browsecomp/transfer_sample.jsonl");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut jsonl = String::new();
    for index in 0..rows {
        let stratum = if index % 2 == 0 { "simple" } else { "hard" };
        writeln!(
            jsonl,
            "{}",
            serde_json::json!({
                "source_id": format!("browsecomp:{index:03}"),
                "question": format!("Browse question {index}?"),
                "answer": format!("Browse answer {index}"),
                "stratum": stratum
            })
        )
        .unwrap();
    }
    fs::write(path, jsonl).unwrap();
}

fn write_browsecomp_public_csv(root: &std::path::Path, rows: usize) {
    let path = root.join("tmp/replication/evoskill/browsecomp/public_browsecomp_test_set.csv");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let canary =
        "BENCHMARK DATA SHOULD NEVER APPEAR AS PLAIN TEXT ONLINE. canary GUID browsecomp:test";
    let mut csv = String::from("problem,answer,problem_topic,canary\n");
    for index in 0..rows {
        let topic = if index % 2 == 0 { "alpha" } else { "beta" };
        let problem = encrypt_browsecomp_public_field(
            &format!("BrowseComp public question {index}?"),
            canary,
        );
        let answer = encrypt_browsecomp_public_field(&format!("Public answer {index}"), canary);
        writeln!(csv, "{problem},{answer},{topic},{canary}").unwrap();
    }
    fs::write(path, csv).unwrap();
}

fn encrypt_browsecomp_public_field(value: &str, canary: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canary.as_bytes());
    let digest = hasher.finalize();
    let encrypted = value
        .as_bytes()
        .iter()
        .zip(digest.iter().copied().cycle())
        .map(|(plain, key)| *plain ^ key)
        .collect::<Vec<_>>();
    BASE64_STANDARD.encode(encrypted)
}

fn write_sealqa_judge_source(root: &std::path::Path) {
    let path = root.join(
        "tmp/skill_opt_sources/arx_2603.02766/src/appendix/agent-prompts/auto_grader_placeholder.md",
    );
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        "# Auto-Grader Prompt (Placeholder)\n\nPinned test source.\n",
    )
    .unwrap();
}

fn init_git_source(path: &std::path::Path, remote_url: &str) {
    fs::create_dir_all(path).unwrap();
    run_git(path, &["init", "-b", "main"]);
    run_git(path, &["config", "user.email", "paper-close@example.test"]);
    run_git(path, &["config", "user.name", "Paper Close"]);
    fs::write(path.join("README.md"), "paper source fixture").unwrap();
    run_git(path, &["add", "README.md"]);
    run_git(path, &["commit", "-m", "fixture"]);
    run_git(path, &["remote", "add", "origin", remote_url]);
}

fn run_git(path: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_sealqa_parquet(root: &std::path::Path, rows: usize) {
    let path = root.join("tmp/replication/evoskill/sealqa/seal-0.parquet");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let schema = Arc::new(Schema::new(vec![
        Field::new("canary", DataType::Utf8, false),
        Field::new("question", DataType::Utf8, false),
        Field::new("answer", DataType::Utf8, false),
        Field::new("topic", DataType::Utf8, true),
        Field::new(
            "urls",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
    ]));
    let canaries = (0..rows)
        .map(|index| format!("duplicate-canary-{}", index % 4))
        .collect::<Vec<_>>();
    let questions = (0..rows)
        .map(|index| format!("Seal question {index}?"))
        .collect::<Vec<_>>();
    let answers = (0..rows)
        .map(|index| format!("Seal answer {index}"))
        .collect::<Vec<_>>();
    let topics = (0..rows)
        .map(|index| {
            if index % 2 == 0 {
                Some("topic-a".to_owned())
            } else {
                Some("topic-b".to_owned())
            }
        })
        .collect::<Vec<_>>();
    let urls = {
        let values = StringBuilder::new();
        let mut builder = ListBuilder::new(values);
        for index in 0..rows {
            builder
                .values()
                .append_value(format!("https://example.test/seal/{index}"));
            builder.append(true);
        }
        Arc::new(builder.finish()) as ArrayRef
    };
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(canaries)) as ArrayRef,
            Arc::new(StringArray::from(questions)) as ArrayRef,
            Arc::new(StringArray::from(answers)) as ArrayRef,
            Arc::new(StringArray::from(topics)) as ArrayRef,
            urls,
        ],
    )
    .unwrap();

    let file = File::create(path).unwrap();
    let mut writer = ArrowWriter::try_new(file, schema, None).unwrap();
    writer.write(&batch).unwrap();
    writer.close().unwrap();
}
