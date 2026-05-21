use std::{fs, path::PathBuf};

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
    let root = unique_temp_dir("trace2skill-manifest");
    let manifest_path = fixture_manifest_path(&root);
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
        .split_manifest
        .cases_for_role(&SplitRole::Train)
        .expect("train split exists");
    let test = manifest
        .split_manifest
        .cases_for_role(&SplitRole::Test)
        .expect("test split exists");

    assert_eq!(train.len(), 200);
    assert_eq!(test.len(), 200);
    assert_eq!(train.first(), Some(&CaseId::from_index(0)));
    assert_eq!(train.last(), Some(&CaseId::from_index(199)));
    assert_eq!(test.first(), Some(&CaseId::from_index(200)));
    assert_eq!(test.last(), Some(&CaseId::from_index(399)));
    assert!(
        manifest
            .split_manifest
            .required_roles()
            .contains(&SplitRole::Train)
    );
    assert!(
        manifest
            .split_manifest
            .required_roles()
            .contains(&SplitRole::Test)
    );
}

fn fixture_manifest_path(root: &PathBuf) -> PathBuf {
    let path = root.join("dataset.json");
    let rows = (0..400)
        .map(|index| {
            let (id, spreadsheet_path, answer_position) = match index {
                0 => (
                    "13-1".to_owned(),
                    "spreadsheet/13-1".to_owned(),
                    "A3:D32".to_owned(),
                ),
                199 => (
                    "52575".to_owned(),
                    "spreadsheet/52575".to_owned(),
                    "B2:B8".to_owned(),
                ),
                399 => (
                    "59902".to_owned(),
                    "spreadsheet/59902".to_owned(),
                    "C5:C28".to_owned(),
                ),
                _ => (
                    format!("row-{index}"),
                    format!("spreadsheet/row-{index}"),
                    "A1:A1".to_owned(),
                ),
            };
            serde_json::json!({
                "id": id,
                "instruction": format!("task {index}"),
                "spreadsheet_path": spreadsheet_path,
                "instruction_type": "synthetic",
                "answer_position": answer_position,
                "answer_sheet": null,
                "data_position": null,
            })
        })
        .collect::<Vec<_>>();
    fs::write(&path, serde_json::to_vec(&rows).unwrap()).unwrap();
    path
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}
