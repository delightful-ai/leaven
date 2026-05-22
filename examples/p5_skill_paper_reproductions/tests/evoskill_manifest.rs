use std::fs;

use p5_skill_paper_reproductions::evoskill::{
    ExactnessClass, ManifestBuildInput, SplitManifestStatus, build_evoskill_replica_manifest,
};

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
    fs::create_dir_all(sealqa.parent().unwrap()).unwrap();
    fs::write(&sealqa, b"sealqa parquet bytes").unwrap();

    let manifest = build_evoskill_replica_manifest(&ManifestBuildInput::new(root.path())).unwrap();
    let sealqa_artifact = manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.id == "sealqa_parquet")
        .expect("sealqa parquet artifact is recorded");

    assert!(sealqa_artifact.exists);
    assert_eq!(sealqa_artifact.bytes, Some(20));
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
}
