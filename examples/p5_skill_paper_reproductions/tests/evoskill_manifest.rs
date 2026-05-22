use std::fs;
use std::fs::File;
use std::sync::Arc;

use arrow_array::builder::{ListBuilder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use p5_skill_paper_reproductions::evoskill::{
    ExactnessClass, ManifestBuildInput, SourceMaterializationStatus, SplitManifestStatus,
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

    let manifest = build_evoskill_replica_manifest(&ManifestBuildInput::new(root.path())).unwrap();

    assert_eq!(manifest.paper.arxiv_id, "2603.02766");
    assert_eq!(manifest.exactness, ExactnessClass::BlockedBeforePaperClose);
    assert_eq!(manifest.scorer.tolerances, [0.0, 0.01, 0.025, 0.05, 0.10]);
    assert!((manifest.scorer.failure_threshold - 0.8).abs() < f64::EPSILON);
    assert_eq!(manifest.frontier.capacity, 3);
    assert_eq!(manifest.frontier.parent_selection, "round-robin");
    assert!(
        manifest
            .proxy_rejections
            .iter()
            .any(|proxy| proxy.contains("P5"))
    );
    assert!(
        manifest
            .blockers
            .iter()
            .any(|blocker| blocker.id == "officeqa_category_split_manifest")
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
