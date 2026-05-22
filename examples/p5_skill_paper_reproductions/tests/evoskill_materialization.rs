use std::fmt::Write as _;
use std::fs;

use leaven_kernel::CaseId;
use p5_skill_paper_reproductions::evoskill::{
    ManifestBuildInput, MaterializationExactness, SourceMaterializationStatus,
    build_evoskill_replica_manifest, materialize_officeqa_source,
};

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
