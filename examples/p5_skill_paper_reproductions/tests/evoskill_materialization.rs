use std::fmt::Write as _;
use std::fs;
use std::fs::File;
use std::sync::Arc;

use arrow_array::builder::{ListBuilder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use leaven_kernel::CaseId;
use p5_skill_paper_reproductions::evoskill::{
    ManifestBuildInput, ManifestError, MaterializationExactness, SourceMaterializationStatus,
    build_evoskill_replica_manifest, materialize_officeqa_source, materialize_sealqa_source,
};
use parquet::arrow::ArrowWriter;

#[test]
fn officeqa_csv_lowers_to_row_stable_cases_with_targets_hidden_from_inputs() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);

    let materialization = materialize_officeqa_source(&ManifestBuildInput::new(root.path()))
        .unwrap()
        .expect("OfficeQA CSV exists");

    assert_eq!(
        materialization.report.source_status,
        SourceMaterializationStatus::Materialized
    );
    assert_eq!(materialization.report.source_rows, Some(246));
    assert_eq!(materialization.report.case_rows, Some(246));
    assert_eq!(
        materialization.report.target_policy,
        "answers are scorer targets only; runner inputs exclude answers"
    );
    assert_eq!(
        materialization
            .report
            .source_row_fingerprint
            .as_deref()
            .unwrap()
            .len(),
        64
    );

    let first = &materialization.rows.rows()[0];
    assert_eq!(first.source_id(), "UID0001");
    assert_eq!(first.input().question, "Question 1?");
    assert_eq!(first.input().difficulty, "easy");
    assert_eq!(first.target().map(String::as_str), Some("Answer 1"));

    let dataset = materialization.rows.clone().into_dataset().unwrap();
    assert!(dataset.cases().contains_key(&CaseId::from_index(0)));
    assert!(dataset.cases().contains_key(&CaseId::from_index(245)));
}

#[test]
fn officeqa_difficulty_substitute_records_split_fingerprints_without_exact_claim() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);

    let materialization = materialize_officeqa_source(&ManifestBuildInput::new(root.path()))
        .unwrap()
        .expect("OfficeQA CSV exists");

    assert_eq!(materialization.report.strata[0].name, "easy");
    assert_eq!(materialization.report.strata[0].rows, 113);
    assert_eq!(materialization.report.strata[1].name, "hard");
    assert_eq!(materialization.report.strata[1].rows, 133);

    let train_12 = materialization
        .report
        .split_materializations
        .iter()
        .find(|split| split.id == "officeqa_difficulty_train_12_val_17")
        .expect("train-12 substitute split exists");

    assert_eq!(
        train_12.exactness,
        MaterializationExactness::PaperCloseSubstitute
    );
    assert_eq!(train_12.train_rows, 12);
    assert_eq!(train_12.validation_rows, Some(17));
    assert_eq!(train_12.test_rows, Some(217));
    assert_eq!(train_12.split_fingerprint.as_deref().unwrap().len(), 64);
    assert_eq!(train_12.role_manifests.len(), 3);
    let train_manifest = train_12
        .role_manifests
        .iter()
        .find(|role| role.role == "train")
        .expect("train role manifest exists");
    assert_eq!(train_manifest.rows, 12);
    assert_eq!(train_manifest.source_ids.len(), 12);
    assert_eq!(train_manifest.source_id_fingerprint.len(), 64);
    assert!(
        train_manifest
            .source_ids
            .iter()
            .all(|source_id| source_id.starts_with("UID"))
    );
    assert!(
        train_manifest
            .source_ids
            .iter()
            .all(|source_id| !source_id.contains("Answer")),
        "split membership exposes source ids, not hidden targets"
    );
    let validation_manifest = train_12
        .role_manifests
        .iter()
        .find(|role| role.role == "validation")
        .expect("validation role manifest exists");
    assert_eq!(validation_manifest.rows, 17);
    let held_out_manifest = train_12
        .role_manifests
        .iter()
        .find(|role| role.role == "held_out_test")
        .expect("held-out role manifest exists");
    assert_eq!(held_out_manifest.rows, 217);
    assert!(
        train_12
            .blocker_ids
            .contains(&"officeqa_category_split_manifest".to_owned())
    );
    assert!(
        train_12
            .blocker_ids
            .contains(&"officeqa_exact_split_membership".to_owned())
    );
}

#[test]
fn officeqa_exact_split_manifest_replaces_substitute_blockers_when_present() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    write_officeqa_exact_split_manifest(root.path());

    let materialization = materialize_officeqa_source(&ManifestBuildInput::new(root.path()))
        .unwrap()
        .expect("OfficeQA CSV exists");

    assert!(materialization.report.blocker_ids.is_empty());
    assert_eq!(materialization.report.split_materializations.len(), 1);
    let split = &materialization.report.split_materializations[0];
    assert_eq!(split.id, "officeqa_paper_declared_train_12_val_17");
    assert_eq!(split.method, "paper_declared_source_id_manifest");
    assert_eq!(split.exactness, MaterializationExactness::PaperExact);
    assert_eq!(split.train_rows, 12);
    assert_eq!(split.validation_rows, Some(17));
    assert_eq!(split.test_rows, Some(217));
    assert!(split.blocker_ids.is_empty());

    let train_manifest = split
        .role_manifests
        .iter()
        .find(|role| role.role == "train")
        .expect("train role manifest exists");
    assert_eq!(
        train_manifest.source_ids,
        (1..=12)
            .map(|index| format!("UID{index:04}"))
            .collect::<Vec<_>>()
    );
    assert!(
        train_manifest
            .source_ids
            .iter()
            .all(|source_id| !source_id.contains("Answer")),
        "exact split membership exposes source ids, not hidden targets"
    );
}

#[test]
fn officeqa_exact_split_manifest_refuses_unknown_source_ids() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    let train = vec!["UID9999".to_owned()];
    let validation = (13..=29)
        .map(|index| format!("UID{index:04}"))
        .collect::<Vec<_>>();
    let held_out_test = (30..=246)
        .map(|index| format!("UID{index:04}"))
        .collect::<Vec<_>>();
    write_exact_split_manifest(
        &root
            .path()
            .join("tmp/replication/evoskill/officeqa/paper_split_manifest.json"),
        "officeqa",
        "officeqa_bad_split",
        &train,
        &validation,
        &held_out_test,
    );

    let error = materialize_officeqa_source(&ManifestBuildInput::new(root.path())).unwrap_err();
    match error {
        ManifestError::SplitManifest { path, message } => {
            assert!(path.ends_with("paper_split_manifest.json"));
            assert!(message.contains("unknown source id `UID9999`"));
        }
        other => panic!("expected split manifest error, got {other:?}"),
    }
}

#[test]
fn officeqa_exact_split_manifest_refuses_partial_row_coverage() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    let train = (1..=12)
        .map(|index| format!("UID{index:04}"))
        .collect::<Vec<_>>();
    let validation = (13..=29)
        .map(|index| format!("UID{index:04}"))
        .collect::<Vec<_>>();
    let held_out_test = (30..=200)
        .map(|index| format!("UID{index:04}"))
        .collect::<Vec<_>>();
    write_exact_split_manifest(
        &root
            .path()
            .join("tmp/replication/evoskill/officeqa/paper_split_manifest.json"),
        "officeqa",
        "officeqa_partial_split",
        &train,
        &validation,
        &held_out_test,
    );

    let error = materialize_officeqa_source(&ManifestBuildInput::new(root.path())).unwrap_err();
    match error {
        ManifestError::SplitManifest { message, .. } => {
            assert!(message.contains("declared split covers 200 rows"));
            assert!(message.contains("source manifest has 246 rows"));
        }
        other => panic!("expected split manifest error, got {other:?}"),
    }
}

#[test]
fn replica_manifest_embeds_materialization_but_stays_blocked_before_paper_close() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);

    let manifest = build_evoskill_replica_manifest(&ManifestBuildInput::new(root.path())).unwrap();
    let officeqa = manifest
        .source_materializations
        .iter()
        .find(|materialization| materialization.dataset_id == "officeqa")
        .expect("OfficeQA materialization report exists");

    assert_eq!(officeqa.source_rows, Some(246));
    assert_eq!(officeqa.split_materializations.len(), 3);
    assert!(
        officeqa
            .split_materializations
            .iter()
            .all(|split| split.exactness == MaterializationExactness::PaperCloseSubstitute)
    );
    assert!(
        manifest
            .blockers
            .iter()
            .any(|blocker| blocker.id == "officeqa_category_split_manifest")
    );
}

#[test]
fn replica_manifest_marks_exact_source_universe_when_split_manifest_is_present() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    write_officeqa_exact_split_manifest(root.path());

    let manifest = build_evoskill_replica_manifest(&ManifestBuildInput::new(root.path())).unwrap();

    let officeqa = manifest
        .source_universe
        .iter()
        .find(|entry| entry.dataset_id == "officeqa")
        .expect("OfficeQA source universe entry exists");
    assert_eq!(
        officeqa.source_artifact_ids,
        ["officeqa_full_csv", "officeqa_exact_split_manifest"]
    );
    assert_eq!(
        officeqa.split_exactness,
        [MaterializationExactness::PaperExact]
    );
    assert!(
        !officeqa
            .blocker_ids
            .contains(&"officeqa_exact_split_membership".to_owned())
    );
    assert!(
        manifest
            .source_blockers
            .iter()
            .all(|blocker| blocker.blocker_id != "officeqa_exact_split_membership")
    );
    assert!(
        manifest
            .blockers
            .iter()
            .all(|blocker| blocker.id != "officeqa_exact_split_membership")
    );
}

#[test]
fn sealqa_parquet_lowers_to_cases_and_row_order_substitute_split() {
    let root = tempfile::tempdir().unwrap();
    write_sealqa_parquet(root.path(), 111);

    let materialization = materialize_sealqa_source(&ManifestBuildInput::new(root.path()))
        .unwrap()
        .expect("SealQA parquet exists");

    assert_eq!(
        materialization.report.source_status,
        SourceMaterializationStatus::Materialized
    );
    assert_eq!(materialization.report.source_rows, Some(111));
    assert_eq!(materialization.report.case_rows, Some(111));
    assert_eq!(
        materialization.report.target_policy,
        "answers are scorer targets only; runner inputs exclude answers"
    );
    assert_eq!(
        materialization
            .report
            .source_row_fingerprint
            .as_deref()
            .unwrap()
            .len(),
        64
    );

    let first = &materialization.rows.rows()[0];
    assert_eq!(first.source_id(), "sealqa:000");
    assert_eq!(first.input().question, "Seal question 0?");
    assert_eq!(first.input().topic.as_deref(), Some("topic-a"));
    assert_eq!(first.input().urls, vec!["https://example.test/seal/0"]);
    assert_eq!(first.target().map(String::as_str), Some("Seal answer 0"));

    let split = materialization
        .report
        .split_materializations
        .iter()
        .find(|split| split.id == "sealqa_row_order_train_11_heldout_100")
        .expect("SealQA row-order substitute split exists");
    assert_eq!(
        split.exactness,
        MaterializationExactness::PaperCloseSubstitute
    );
    assert_eq!(split.train_rows, 11);
    assert_eq!(split.validation_rows, None);
    assert_eq!(split.test_rows, Some(100));
    assert_eq!(split.split_fingerprint.as_deref().unwrap().len(), 64);
    assert_eq!(split.role_manifests.len(), 2);
    let train_manifest = split
        .role_manifests
        .iter()
        .find(|role| role.role == "train")
        .expect("SealQA train role manifest exists");
    assert_eq!(train_manifest.rows, 11);
    assert_eq!(
        train_manifest.source_ids.first().map(String::as_str),
        Some("sealqa:000")
    );
    assert_eq!(
        train_manifest.source_ids.last().map(String::as_str),
        Some("sealqa:010")
    );
    assert_eq!(train_manifest.source_id_fingerprint.len(), 64);
    let held_out_manifest = split
        .role_manifests
        .iter()
        .find(|role| role.role == "held_out_test")
        .expect("SealQA held-out role manifest exists");
    assert_eq!(held_out_manifest.rows, 100);
    assert_eq!(
        held_out_manifest.source_ids.first().map(String::as_str),
        Some("sealqa:011")
    );
    assert_eq!(
        held_out_manifest.source_ids.last().map(String::as_str),
        Some("sealqa:110")
    );
    assert!(
        split
            .blocker_ids
            .contains(&"sealqa_split_manifest".to_owned())
    );
}

#[test]
fn sealqa_exact_split_manifest_replaces_row_order_blocker_when_present() {
    let root = tempfile::tempdir().unwrap();
    write_sealqa_parquet(root.path(), 111);
    write_sealqa_exact_split_manifest(root.path());

    let materialization = materialize_sealqa_source(&ManifestBuildInput::new(root.path()))
        .unwrap()
        .expect("SealQA parquet exists");

    assert!(materialization.report.blocker_ids.is_empty());
    assert_eq!(materialization.report.split_materializations.len(), 1);
    let split = &materialization.report.split_materializations[0];
    assert_eq!(split.id, "sealqa_paper_declared_train_11_heldout_100");
    assert_eq!(split.method, "paper_declared_source_id_manifest");
    assert_eq!(split.exactness, MaterializationExactness::PaperExact);
    assert_eq!(split.train_rows, 11);
    assert_eq!(split.validation_rows, None);
    assert_eq!(split.test_rows, Some(100));
    assert!(split.blocker_ids.is_empty());

    let held_out_manifest = split
        .role_manifests
        .iter()
        .find(|role| role.role == "held_out_test")
        .expect("held-out role manifest exists");
    assert_eq!(
        held_out_manifest.source_ids.first().map(String::as_str),
        Some("sealqa:011")
    );
    assert_eq!(
        held_out_manifest.source_ids.last().map(String::as_str),
        Some("sealqa:110")
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

fn write_officeqa_exact_split_manifest(root: &std::path::Path) {
    let train = (1..=12)
        .map(|index| format!("UID{index:04}"))
        .collect::<Vec<_>>();
    let validation = (13..=29)
        .map(|index| format!("UID{index:04}"))
        .collect::<Vec<_>>();
    let held_out_test = (30..=246)
        .map(|index| format!("UID{index:04}"))
        .collect::<Vec<_>>();
    write_exact_split_manifest(
        &root.join("tmp/replication/evoskill/officeqa/paper_split_manifest.json"),
        "officeqa",
        "officeqa_paper_declared_train_12_val_17",
        &train,
        &validation,
        &held_out_test,
    );
}

fn write_sealqa_exact_split_manifest(root: &std::path::Path) {
    let train = (0..11)
        .map(|index| format!("sealqa:{index:03}"))
        .collect::<Vec<_>>();
    let validation = Vec::new();
    let held_out_test = (11..111)
        .map(|index| format!("sealqa:{index:03}"))
        .collect::<Vec<_>>();
    write_exact_split_manifest(
        &root.join("tmp/replication/evoskill/sealqa/paper_split_manifest.json"),
        "sealqa",
        "sealqa_paper_declared_train_11_heldout_100",
        &train,
        &validation,
        &held_out_test,
    );
}

fn write_exact_split_manifest(
    path: &std::path::Path,
    dataset_id: &str,
    split_id: &str,
    train: &[String],
    validation: &[String],
    held_out_test: &[String],
) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let manifest = serde_json::json!({
        "schema_version": 1,
        "dataset_id": dataset_id,
        "split_id": split_id,
        "roles": {
            "train": train,
            "validation": validation,
            "held_out_test": held_out_test,
        },
    });
    fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
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
