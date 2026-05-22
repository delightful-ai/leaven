use std::fmt::Write as _;
use std::fs::{self, File};
use std::process::Command;
use std::sync::Arc;

use arrow_array::builder::{ListBuilder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use p5_skill_paper_reproductions::evoskill::{
    DatasetMaterializationReport, EvoSkillFinalReport, ExactnessClass, FinalScoreSlot,
    FinalScoreStatus, LiveRunGateStatus, ManifestBuildInput, MaterializationExactness,
    PaperCloseGateStatus, PaperResultTargetStatus, ProxyRejectionStatus, SourceBlockerStatus,
    SplitAcceptanceStatus, SplitMaterializationReport, build_evoskill_final_report,
    write_evoskill_local_source_pin_manifest, write_evoskill_paper_close_split_policy_manifest,
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
    assert_eq!(report.schema_version, 10);
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
            .child_score_source
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
    assert!(report.contains("\"live_run_gate\""));
    assert!(report.contains("\"paper_close_gates\""));
    assert!(report.contains("\"source_blockers\""));
    assert!(report.contains("\"proxy_rejection_gates\""));
    assert!(report.contains("\"paper_result_targets\""));
    assert!(report.contains("\"paper_result_target_ids\""));
    assert!(report.contains("\"role_source_id_fingerprint\""));
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
