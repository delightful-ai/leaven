use std::fmt::Write as _;
use std::fs;
use std::fs::File;
use std::process::Command;
use std::sync::Arc;

use arrow_array::builder::{ListBuilder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use p5_skill_paper_reproductions::evoskill::{
    ExactnessClass, ManifestBuildInput, ManifestError, MaterializationExactness,
    PaperResultTargetStatus, SourceBlockerStatus, SourceMaterializationStatus,
    SourcePaperReleaseStatus, SourceRemoteProbeStatus, SourceRevisionStatus, SplitManifestStatus,
    build_evoskill_replica_manifest,
};
use parquet::arrow::ArrowWriter;

#[test]
fn evoskill_manifest_records_paper_close_denominator_without_claiming_proof() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("tmp/skill_opt_sources/arx_2603.02766")).unwrap();
    fs::write(
        root.path()
            .join("tmp/skill_opt_sources/arx_2603.02766/full_source.md"),
        "EvoSkill paper fixture",
    )
    .unwrap();
    write_sealqa_judge_source(root.path());

    let manifest = build_evoskill_replica_manifest(&ManifestBuildInput::new(root.path())).unwrap();

    assert_eq!(manifest.schema_version, 13);
    assert_eq!(manifest.paper.arxiv_id, "2603.02766");
    assert_eq!(manifest.exactness, ExactnessClass::BlockedBeforePaperClose);
    assert_eq!(manifest.scorer.tolerances, [0.0, 0.01, 0.025, 0.05, 0.10]);
    assert!((manifest.scorer.failure_threshold - 0.8).abs() < f64::EPSILON);
    let sealqa_judge = manifest
        .scorer
        .judge_templates
        .iter()
        .find(|template| template.id == "sealqa-auto-grader-placeholder-v1")
        .expect("SealQA judge template is recorded");
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
    assert_eq!(manifest.frontier.capacity, 3);
    assert_eq!(manifest.frontier.parent_selection, "round-robin");
    assert!(
        manifest
            .proxy_rejections
            .iter()
            .any(|proxy| proxy.contains("P5") && proxy.contains("paper-close"))
    );
    assert!(
        manifest
            .blockers
            .iter()
            .any(|blocker| blocker.id == "officeqa_category_split_manifest")
    );
    assert!(
        manifest
            .blockers
            .iter()
            .any(|blocker| blocker.id == "live_run_spend_approval")
    );
    assert!(
        manifest
            .blockers
            .iter()
            .all(|blocker| blocker.id != "officeqa_reported_result_target")
    );
    assert_paper_result_targets_cover_all_paper_headlines(&manifest);
    assert_source_blockers_report_missing_paper_artifacts(&manifest);
}

fn assert_paper_result_targets_cover_all_paper_headlines(
    manifest: &p5_skill_paper_reproductions::evoskill::EvoSkillReplicaManifest,
) {
    assert_eq!(manifest.paper_result_targets.len(), 7);
    let baseline = manifest
        .paper_result_targets
        .iter()
        .find(|target| target.id == "officeqa_baseline_exact_match_table")
        .expect("baseline OfficeQA exact-match target is recorded");
    assert_eq!(baseline.status, PaperResultTargetStatus::Reported);
    assert_eq!(baseline.dataset_id, "officeqa");
    assert_eq!(baseline.candidate_role, "baseline");
    assert_eq!(baseline.metric, "exact_match_0_percent_tolerance");
    assert!((baseline.value_percent - 60.6).abs() < f64::EPSILON);

    let skill_merge_targets = manifest
        .paper_result_targets
        .iter()
        .filter(|target| target.candidate_role == "skill_merge")
        .collect::<Vec<_>>();
    assert_eq!(skill_merge_targets.len(), 2);
    assert!(skill_merge_targets.iter().all(|target| {
        target.status == PaperResultTargetStatus::AmbiguousCandidate
            && target.ambiguity_group.as_deref() == Some("officeqa_skill_merge_exact_match")
    }));
    let mut values = skill_merge_targets
        .iter()
        .map(|target| target.value_percent)
        .collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    assert_eq!(values, [67.9, 68.1]);

    assert_reported_target(
        manifest,
        "sealqa_baseline_accuracy",
        "sealqa",
        "baseline",
        "llm_judge_accuracy",
        26.6,
    );
    assert_reported_target(
        manifest,
        "sealqa_optimized_accuracy",
        "sealqa",
        "optimized",
        "llm_judge_accuracy",
        38.7,
    );
    assert_reported_target(
        manifest,
        "browsecomp_baseline_accuracy",
        "browsecomp_transfer",
        "baseline",
        "accuracy",
        43.5,
    );
    assert_reported_target(
        manifest,
        "browsecomp_sealqa_skill_transfer_accuracy",
        "browsecomp_transfer",
        "sealqa_skill_transfer",
        "accuracy",
        48.8,
    );
}

fn assert_reported_target(
    manifest: &p5_skill_paper_reproductions::evoskill::EvoSkillReplicaManifest,
    id: &str,
    dataset_id: &str,
    candidate_role: &str,
    metric: &str,
    value_percent: f64,
) {
    let target = manifest
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

fn assert_source_blockers_report_missing_paper_artifacts(
    manifest: &p5_skill_paper_reproductions::evoskill::EvoSkillReplicaManifest,
) {
    assert_eq!(manifest.source_blockers.len(), 5);
    let blocker = |id: &str| {
        manifest
            .source_blockers
            .iter()
            .find(|blocker| blocker.blocker_id == id)
            .unwrap_or_else(|| panic!("missing source blocker {id}"))
    };

    assert_eq!(
        blocker("source_pin").status,
        SourceBlockerStatus::UnresolvedSourcePolicy
    );
    assert!(
        blocker("source_pin")
            .required_for
            .contains(&"paper_close_comparison_denominator".to_owned())
    );

    let officeqa_category = blocker("officeqa_category_split_manifest");
    assert_eq!(
        officeqa_category.status,
        SourceBlockerStatus::MissingLocalArtifact
    );
    assert_eq!(officeqa_category.dataset_id, "officeqa");
    assert!(
        officeqa_category
            .local_path_candidates
            .iter()
            .any(
                |candidate| candidate.relative_path.ends_with("solved_dataset.csv")
                    && !candidate.exists
            )
    );

    let officeqa_split = blocker("officeqa_exact_split_membership");
    assert_eq!(
        officeqa_split.status,
        SourceBlockerStatus::MissingExactSplitManifest
    );
    assert!(
        officeqa_split
            .note
            .contains("paper exact membership is absent")
    );

    assert_eq!(
        blocker("sealqa_split_manifest").status,
        SourceBlockerStatus::MissingExactSplitManifest
    );
    assert_eq!(
        blocker("browsecomp_transfer_sample").status,
        SourceBlockerStatus::MissingLocalArtifact
    );
}

#[test]
fn manifest_declares_source_universe_without_confusing_substitutes_for_exact_splits() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    let sealqa = root
        .path()
        .join("tmp/replication/evoskill/sealqa/seal-0.parquet");
    write_sealqa_parquet(&sealqa, 111);

    let manifest = build_evoskill_replica_manifest(&ManifestBuildInput::new(root.path())).unwrap();

    let officeqa = manifest
        .source_universe
        .iter()
        .find(|entry| entry.dataset_id == "officeqa")
        .expect("OfficeQA source universe entry is declared");
    assert_eq!(officeqa.source_artifact_ids, ["officeqa_full_csv"]);
    assert_eq!(officeqa.source_revision_ids, ["officeqa_repo"]);
    assert_eq!(officeqa.paper_rows, Some(246));
    assert_eq!(officeqa.materialized_rows, Some(246));
    assert_eq!(
        officeqa.source_row_fingerprint.as_deref().unwrap().len(),
        64
    );
    assert_eq!(officeqa.split_ids.len(), 3);
    assert!(
        officeqa
            .split_exactness
            .iter()
            .all(|exactness| *exactness == MaterializationExactness::PaperCloseSubstitute)
    );
    assert!(
        officeqa
            .blocker_ids
            .contains(&"officeqa_exact_split_membership".to_owned())
    );
    assert!(
        !officeqa
            .blocker_ids
            .contains(&"officeqa_reported_result_target".to_owned())
    );

    let sealqa = manifest
        .source_universe
        .iter()
        .find(|entry| entry.dataset_id == "sealqa")
        .expect("SealQA source universe entry is declared");
    assert_eq!(sealqa.source_artifact_ids, ["sealqa_parquet"]);
    assert!(sealqa.source_revision_ids.is_empty());
    assert_eq!(sealqa.paper_rows, Some(111));
    assert_eq!(sealqa.materialized_rows, Some(111));
    assert_eq!(
        sealqa.split_ids,
        ["sealqa_row_order_train_11_heldout_100".to_owned()]
    );
    assert_eq!(
        sealqa.split_exactness,
        [MaterializationExactness::PaperCloseSubstitute]
    );
    assert_eq!(sealqa.blocker_ids, ["sealqa_split_manifest".to_owned()]);

    let browsecomp = manifest
        .source_universe
        .iter()
        .find(|entry| entry.dataset_id == "browsecomp_transfer")
        .expect("BrowseComp transfer blocker is declared in the source universe");
    assert_eq!(
        browsecomp.source_artifact_ids,
        ["browsecomp_transfer_sample"]
    );
    assert!(browsecomp.source_revision_ids.is_empty());
    assert_eq!(browsecomp.paper_rows, Some(128));
    assert_eq!(browsecomp.materialized_rows, None);
    assert_eq!(
        browsecomp.blocker_ids,
        ["browsecomp_transfer_sample".to_owned()]
    );
}

#[test]
fn source_artifacts_are_fingerprinted_without_certifying_missing_splits() {
    let root = tempfile::tempdir().unwrap();
    let sealqa = root
        .path()
        .join("tmp/replication/evoskill/sealqa/seal-0.parquet");
    write_sealqa_parquet(&sealqa, 111);

    let manifest = build_evoskill_replica_manifest(&ManifestBuildInput::new(root.path())).unwrap();
    let sealqa_artifact = manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.id == "sealqa_parquet")
        .expect("sealqa parquet artifact is recorded");

    assert!(sealqa_artifact.exists);
    assert!(sealqa_artifact.bytes.unwrap() > 20);
    assert_eq!(sealqa_artifact.sha256.as_deref().unwrap().len(), 64);

    let sealqa_dataset = manifest
        .datasets
        .iter()
        .find(|dataset| dataset.id == "sealqa")
        .expect("SealQA dataset is recorded");
    assert_eq!(
        sealqa_dataset.split_status,
        SplitManifestStatus::BlockedMissingSplitManifest
    );
    assert_eq!(
        sealqa_dataset.blocker_ids,
        vec!["sealqa_split_manifest".to_owned()]
    );
    let sealqa_materialization = manifest
        .source_materializations
        .iter()
        .find(|materialization| materialization.dataset_id == "sealqa")
        .expect("SealQA materialization is recorded");
    assert_eq!(
        sealqa_materialization.source_status,
        SourceMaterializationStatus::Materialized
    );

    let sealqa_blocker = manifest
        .source_blockers
        .iter()
        .find(|blocker| blocker.blocker_id == "sealqa_split_manifest")
        .expect("SealQA split blocker is recorded");
    let sealqa_candidate = sealqa_blocker
        .local_path_candidates
        .iter()
        .find(|candidate| candidate.relative_path.ends_with("seal-0.parquet"))
        .expect("SealQA parquet candidate is recorded");
    assert!(sealqa_candidate.exists);
    assert!(sealqa_candidate.is_file);
    assert!(!sealqa_candidate.is_dir);
    assert!(sealqa_candidate.bytes.unwrap() > 20);
    assert_eq!(sealqa_candidate.sha256.as_deref().unwrap().len(), 64);
}

#[test]
fn browsecomp_transfer_sample_materializes_denominator_without_score_proof() {
    let root = tempfile::tempdir().unwrap();
    write_browsecomp_transfer_sample(root.path(), 128);

    let manifest = build_evoskill_replica_manifest(&ManifestBuildInput::new(root.path())).unwrap();

    let browsecomp_dataset = manifest
        .datasets
        .iter()
        .find(|dataset| dataset.id == "browsecomp_transfer")
        .expect("BrowseComp transfer dataset is declared");
    assert_eq!(
        browsecomp_dataset.split_status,
        SplitManifestStatus::PaperCloseSubstituteAccepted
    );
    assert!(browsecomp_dataset.blocker_ids.is_empty());

    let browsecomp = manifest
        .source_universe
        .iter()
        .find(|entry| entry.dataset_id == "browsecomp_transfer")
        .expect("BrowseComp transfer source universe entry exists");
    assert_eq!(
        browsecomp.source_artifact_ids,
        ["browsecomp_transfer_sample"]
    );
    assert_eq!(browsecomp.materialized_rows, Some(128));
    assert_eq!(
        browsecomp.source_row_fingerprint.as_deref().unwrap().len(),
        64
    );
    assert_eq!(
        browsecomp.split_ids,
        ["browsecomp_transfer_sample_128_heldout".to_owned()]
    );
    assert_eq!(
        browsecomp.split_exactness,
        [MaterializationExactness::PaperCloseSubstitute]
    );
    assert!(browsecomp.blocker_ids.is_empty());
    assert!(
        manifest
            .source_blockers
            .iter()
            .all(|blocker| blocker.blocker_id != "browsecomp_transfer_sample")
    );
    assert!(
        manifest
            .blockers
            .iter()
            .all(|blocker| blocker.id != "browsecomp_transfer_sample")
    );
}

#[test]
fn source_revisions_record_local_git_identity_without_claiming_paper_pin() {
    let root = tempfile::tempdir().unwrap();
    init_git_source(
        &root.path().join("tmp/repros/evoskill"),
        "https://github.com/sentient-agi/EvoSkill.git",
    );

    let manifest = build_evoskill_replica_manifest(&ManifestBuildInput::new(root.path())).unwrap();
    let evoskill = manifest
        .source_revisions
        .iter()
        .find(|revision| revision.id == "evoskill_repo")
        .expect("EvoSkill source revision is recorded");

    assert_eq!(evoskill.status, SourceRevisionStatus::Present);
    assert_eq!(evoskill.branch.as_deref(), Some("main"));
    assert_eq!(
        evoskill.remote_url.as_deref(),
        Some("https://github.com/sentient-agi/EvoSkill.git")
    );
    assert_eq!(evoskill.head.as_deref().unwrap().len(), 40);
    assert_eq!(
        evoskill.paper_release_status,
        SourcePaperReleaseStatus::Unresolved
    );
    assert_eq!(
        evoskill.remote_probe_status,
        SourceRemoteProbeStatus::NotProbedNoNetworkDefault
    );
    assert_eq!(evoskill.paper_release_ref, None);
    assert_eq!(evoskill.paper_release_head, None);
    assert_eq!(evoskill.remote_head, None);
    assert_eq!(evoskill.blocker_ids, ["source_pin".to_owned()]);
}

#[test]
fn source_pin_manifest_resolves_local_checkout_policy_without_network_probe() {
    let root = tempfile::tempdir().unwrap();
    let evoskill_path = root.path().join("tmp/repros/evoskill");
    let officeqa_path = root.path().join("tmp/repros/officeqa");
    init_git_source(
        &evoskill_path,
        "https://github.com/sentient-agi/EvoSkill.git",
    );
    init_git_source(&officeqa_path, "https://github.com/databricks/officeqa.git");
    write_source_pin_manifest(
        root.path(),
        &git_stdout(&evoskill_path, &["rev-parse", "HEAD"]),
        &git_stdout(&officeqa_path, &["rev-parse", "HEAD"]),
    );

    let manifest = build_evoskill_replica_manifest(&ManifestBuildInput::new(root.path())).unwrap();

    assert!(
        manifest
            .source_blockers
            .iter()
            .all(|blocker| blocker.blocker_id != "source_pin")
    );
    assert!(
        manifest
            .blockers
            .iter()
            .all(|blocker| blocker.id != "source_pin")
    );
    let source_pin_artifact = manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.id == "source_pin_manifest")
        .expect("source pin manifest artifact is recorded");
    assert!(source_pin_artifact.exists);
    assert_eq!(source_pin_artifact.sha256.as_deref().unwrap().len(), 64);
    let evoskill = manifest
        .source_revisions
        .iter()
        .find(|revision| revision.id == "evoskill_repo")
        .expect("EvoSkill source revision is recorded");
    assert_eq!(
        evoskill.paper_release_status,
        SourcePaperReleaseStatus::PinnedLocalCheckout
    );
    assert_eq!(
        evoskill.paper_release_ref.as_deref(),
        Some("local_checkout_pinned")
    );
    assert_eq!(evoskill.paper_release_head, evoskill.head);
    assert!(evoskill.blocker_ids.is_empty());
    assert_eq!(
        evoskill.remote_probe_status,
        SourceRemoteProbeStatus::NotProbedNoNetworkDefault
    );
}

#[test]
fn source_pin_manifest_refuses_head_mismatch() {
    let root = tempfile::tempdir().unwrap();
    let evoskill_path = root.path().join("tmp/repros/evoskill");
    let officeqa_path = root.path().join("tmp/repros/officeqa");
    init_git_source(
        &evoskill_path,
        "https://github.com/sentient-agi/EvoSkill.git",
    );
    init_git_source(&officeqa_path, "https://github.com/databricks/officeqa.git");
    write_source_pin_manifest(
        root.path(),
        "0000000000000000000000000000000000000000",
        &git_stdout(&officeqa_path, &["rev-parse", "HEAD"]),
    );

    let error = build_evoskill_replica_manifest(&ManifestBuildInput::new(root.path())).unwrap_err();

    match error {
        ManifestError::SourcePinManifest { message, .. } => {
            assert!(message.contains("evoskill_repo"));
            assert!(message.contains("head mismatch"));
        }
        other => panic!("expected source pin manifest error, got {other:?}"),
    }
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

fn write_source_pin_manifest(root: &std::path::Path, evoskill_head: &str, officeqa_head: &str) {
    let path = root.join("tmp/replication/evoskill/source_pin_manifest.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let manifest = serde_json::json!({
        "schema_version": 1,
        "policy": "local_checkout_pinned",
        "sources": [
            {
                "id": "evoskill_repo",
                "relative_path": "tmp/repros/evoskill",
                "head": evoskill_head,
                "branch": "main",
                "remote_url": "https://github.com/sentient-agi/EvoSkill.git"
            },
            {
                "id": "officeqa_repo",
                "relative_path": "tmp/repros/officeqa",
                "head": officeqa_head,
                "branch": "main",
                "remote_url": "https://github.com/databricks/officeqa.git"
            }
        ]
    });
    fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
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

fn git_stdout(path: &std::path::Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
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

fn write_sealqa_parquet(path: &std::path::Path, rows: usize) {
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
        .map(|index| Some(format!("topic-{}", index % 2)))
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
