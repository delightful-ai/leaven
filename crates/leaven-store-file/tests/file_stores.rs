use bytes::Bytes;
use leaven_kernel::EvidenceRef;
use leaven_store::{CheckpointBytes, CheckpointStore, Evidence, EvidenceStore, StoreError};
use leaven_store_file::{FileCheckpointStore, FileEvidenceStore, FileJsonCheckpointStore};

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct TestEvidence {
    message: String,
}

impl Evidence for TestEvidence {}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct TestCheckpoint {
    phase: String,
    count: usize,
}

#[test]
fn file_evidence_round_trips_and_reopens_without_overwrite() {
    let root = temp_root("evidence");
    let store = FileEvidenceStore::<TestEvidence>::open("evidence", &root).unwrap();
    assert_eq!(store.root(), root.as_path());

    let first = store
        .put(TestEvidence {
            message: "first".to_owned(),
        })
        .unwrap();
    assert_eq!(first.store, "evidence");
    assert_eq!(first.key, "0");
    assert_eq!(store.get(&first).unwrap().message, "first");

    let reopened = FileEvidenceStore::<TestEvidence>::open("evidence", &root).unwrap();
    let second = reopened
        .put(TestEvidence {
            message: "second".to_owned(),
        })
        .unwrap();
    assert_eq!(second.key, "1");
    assert_eq!(reopened.get(&first).unwrap().message, "first");
    assert_eq!(reopened.get(&second).unwrap().message, "second");
}

#[test]
fn file_evidence_open_reports_directory_creation_failure() {
    let root = temp_root("evidence-open-file");
    let file_path = root.join("not-a-directory");
    std::fs::write(&file_path, b"file").unwrap();

    let Err(err) = FileEvidenceStore::<TestEvidence>::open("evidence", file_path) else {
        panic!("opening a file path as an evidence store unexpectedly succeeded");
    };

    assert!(matches!(
        err,
        StoreError::OperationFailed {
            operation: "open",
            ..
        }
    ));
}

#[test]
fn file_evidence_rejects_wrong_store_name() {
    let root = temp_root("wrong-store");
    let store = FileEvidenceStore::<TestEvidence>::open("evidence", root).unwrap();
    let err = store
        .get(&EvidenceRef {
            store: "other".to_owned(),
            key: "0".to_owned(),
        })
        .unwrap_err();

    assert!(matches!(err, StoreError::EvidenceNotFound(_)));
}

#[test]
fn file_evidence_rejects_invalid_keys_and_bad_payloads() {
    let root = temp_root("invalid-key");
    let store = FileEvidenceStore::<TestEvidence>::open("evidence", &root).unwrap();

    let invalid_key = store
        .get(&EvidenceRef {
            store: "evidence".to_owned(),
            key: "../escape".to_owned(),
        })
        .unwrap_err();
    assert!(matches!(
        invalid_key,
        StoreError::OperationFailed {
            operation: "evidence_path",
            ..
        }
    ));

    std::fs::write(root.join("0.json"), b"{not json").unwrap();
    let bad_payload = store
        .get(&EvidenceRef {
            store: "evidence".to_owned(),
            key: "0".to_owned(),
        })
        .unwrap_err();
    assert!(matches!(bad_payload, StoreError::Serialization(_)));
}

#[test]
fn file_evidence_reopen_skips_non_numeric_json_files() {
    let root = temp_root("scan");
    std::fs::write(root.join("notes.txt"), b"ignore").unwrap();
    std::fs::write(root.join("abc.json"), b"ignore").unwrap();
    std::fs::write(root.join("7.json"), br#"{"message":"old"}"#).unwrap();

    let store = FileEvidenceStore::<TestEvidence>::open("evidence", &root).unwrap();
    let reference = store
        .put(TestEvidence {
            message: "new".to_owned(),
        })
        .unwrap();

    assert_eq!(reference.key, "8");
}

#[test]
fn file_checkpoint_round_trips_latest_pointer() {
    let root = temp_root("checkpoint");
    let store = FileCheckpointStore::open(&root).unwrap();
    assert_eq!(store.root(), root.as_path());

    let first = store
        .put(CheckpointBytes(Bytes::from_static(b"first")))
        .unwrap();
    let second = store
        .put(CheckpointBytes(Bytes::from_static(b"second")))
        .unwrap();

    assert_eq!(store.get(first).unwrap().0, Bytes::from_static(b"first"));
    assert_eq!(store.get(second).unwrap().0, Bytes::from_static(b"second"));
    assert_eq!(store.latest().unwrap(), Some(second));

    let reopened = FileCheckpointStore::open(root).unwrap();
    assert_eq!(reopened.latest().unwrap(), Some(second));
}

#[test]
fn file_json_checkpoint_round_trips_latest_typed_checkpoint() {
    let root = temp_root("checkpoint-json");
    let store = FileJsonCheckpointStore::<TestCheckpoint>::open(&root).unwrap();
    assert_eq!(store.root(), root.as_path());

    let first = store
        .put(&TestCheckpoint {
            phase: "first".to_owned(),
            count: 1,
        })
        .unwrap();
    let second = store
        .put(&TestCheckpoint {
            phase: "second".to_owned(),
            count: 2,
        })
        .unwrap();

    assert_eq!(
        store.get(first).unwrap(),
        TestCheckpoint {
            phase: "first".to_owned(),
            count: 1,
        }
    );
    assert_eq!(
        store.latest().unwrap(),
        Some((
            second,
            TestCheckpoint {
                phase: "second".to_owned(),
                count: 2,
            },
        ))
    );

    let reopened = FileJsonCheckpointStore::<TestCheckpoint>::open(root).unwrap();
    assert_eq!(
        reopened
            .latest()
            .unwrap()
            .map(|(_id, checkpoint)| checkpoint),
        Some(TestCheckpoint {
            phase: "second".to_owned(),
            count: 2,
        })
    );
}

#[test]
fn file_checkpoint_open_reports_directory_creation_failure() {
    let root = temp_root("checkpoint-open-file");
    let file_path = root.join("not-a-directory");
    std::fs::write(&file_path, b"file").unwrap();

    let err = FileCheckpointStore::open(file_path).unwrap_err();

    assert!(matches!(
        err,
        StoreError::OperationFailed {
            operation: "open",
            ..
        }
    ));
}

#[test]
fn file_checkpoint_latest_reports_absent_and_malformed_pointers() {
    let root = temp_root("checkpoint-latest");
    let store = FileCheckpointStore::open(&root).unwrap();
    assert_eq!(store.latest().unwrap(), None);

    std::fs::write(root.join("LATEST"), "not-a-uuid").unwrap();
    let err = store.latest().unwrap_err();
    assert!(matches!(err, StoreError::Serialization(_)));
}

#[test]
fn file_checkpoint_missing_id_is_operation_failure() {
    let root = temp_root("checkpoint-missing");
    let store = FileCheckpointStore::open(root).unwrap();
    let Err(err) = store.get(leaven_kernel::CheckpointId::new()) else {
        panic!("missing checkpoint unexpectedly loaded");
    };

    assert!(matches!(
        err,
        StoreError::OperationFailed {
            operation: "get",
            ..
        }
    ));
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "leaven-store-file-test-{name}-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}
