use std::fmt::Write as _;
use std::fs;
use std::fs::File;
use std::sync::Arc;

use arrow_array::builder::{ListBuilder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use leaven_kernel::CaseId;
use p5_skill_paper_reproductions::evoskill::{
    ManifestBuildInput, MaterializationExactness, SourceMaterializationStatus,
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
