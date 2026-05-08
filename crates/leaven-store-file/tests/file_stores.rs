use bytes::Bytes;
use leaven_kernel::EvidenceRef;
use leaven_store::{CheckpointBytes, CheckpointStore, Evidence, EvidenceStore, StoreError};
use leaven_store_file::{FileCheckpointStore, FileEvidenceStore};

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct TestEvidence {
    message: String,
}

impl Evidence for TestEvidence {}

#[test]
fn file_evidence_round_trips_and_reopens_without_overwrite() {
    let root = temp_root("evidence");
    let store = FileEvidenceStore::<TestEvidence>::open("evidence", &root).unwrap();

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
fn file_checkpoint_round_trips_latest_pointer() {
    let root = temp_root("checkpoint");
    let store = FileCheckpointStore::open(&root).unwrap();

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

fn temp_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "leaven-store-file-test-{name}-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}
