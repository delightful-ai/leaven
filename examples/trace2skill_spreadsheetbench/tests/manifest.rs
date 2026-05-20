use std::path::Path;

use leaven_eval::SplitRole;
use leaven_kernel::{CaseId, MetadataKey, MetadataValue};
use trace2skill_spreadsheetbench::load_verified_400_manifest;

fn assert_string_metadata(value: Option<&MetadataValue>, expected: &str) {
    let Some(MetadataValue::String(actual)) = value else {
        panic!("expected string metadata");
    };
    assert_eq!(actual, expected);
}

fn assert_u64_metadata(value: Option<&MetadataValue>, expected: u64) {
    let Some(MetadataValue::U64(actual)) = value else {
        panic!("expected u64 metadata");
    };
    assert_eq!(*actual, expected);
}

#[test]
fn lowers_upstream_verified_400_manifest_to_cases_and_paper_splits() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../tmp/repros/trace2skill-upstream/data/spreadsheetbench_verified/spreadsheetbench_verified_400/dataset.json",
    );
    let manifest = load_verified_400_manifest(&manifest_path).unwrap();

    assert_eq!(manifest.dataset.cases().len(), 400);
    assert_eq!(
        manifest.dataset.cases().keys().copied().collect::<Vec<_>>(),
        (0..400).map(CaseId::from_index).collect::<Vec<_>>()
    );

    let first = manifest
        .dataset
        .cases()
        .get(&CaseId::from_index(0))
        .expect("row 0 case exists");
    assert_eq!(first.input.spreadsheet_path, "spreadsheet/13-1");
    assert_eq!(first.target.as_ref().unwrap().answer_position, "A3:D32");
    assert_string_metadata(first.metadata.get(&MetadataKey::from("source_id")), "13-1");
    assert_u64_metadata(
        first.metadata.get(&MetadataKey::from("source_row_index")),
        0,
    );

    let last = manifest
        .dataset
        .cases()
        .get(&CaseId::from_index(399))
        .expect("row 399 case exists");
    assert_eq!(last.input.spreadsheet_path, "spreadsheet/59902");
    assert_eq!(last.target.as_ref().unwrap().answer_position, "C5:C28");
    assert_string_metadata(last.metadata.get(&MetadataKey::from("source_id")), "59902");
    assert_u64_metadata(
        last.metadata.get(&MetadataKey::from("source_row_index")),
        399,
    );

    let train = manifest
        .splits
        .cases(&SplitRole::Train.partition_id())
        .expect("train split exists");
    let test = manifest
        .splits
        .cases(&SplitRole::Test.partition_id())
        .expect("test split exists");

    assert_eq!(train.len(), 200);
    assert_eq!(test.len(), 200);
    assert_eq!(train.first(), Some(&CaseId::from_index(0)));
    assert_eq!(train.last(), Some(&CaseId::from_index(199)));
    assert_eq!(test.first(), Some(&CaseId::from_index(200)));
    assert_eq!(test.last(), Some(&CaseId::from_index(399)));
}
