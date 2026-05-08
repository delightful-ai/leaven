use bytes::Bytes;
use leaven_kernel::BlobRef;
use leaven_store::{BlobStore, BlobWrite, CheckpointBytes, CheckpointStore, StoreError};
use leaven_store_inline::InlineStore;

#[test]
fn blobs_round_trip_by_reference() {
    let store = InlineStore::new("inline");

    let reference = BlobStore::put(
        &store,
        BlobWrite {
            bytes: Bytes::from_static(b"blob"),
            content_type: Some("text/plain".to_owned()),
        },
    )
    .unwrap();
    let bytes = BlobStore::get(&store, &reference).unwrap();

    assert_eq!(reference.store, "inline");
    assert_eq!(reference.key, "0");
    assert_eq!(bytes, Bytes::from_static(b"blob"));
}

#[test]
fn checkpoints_round_trip_by_id() {
    let store = InlineStore::new("inline");

    let id =
        CheckpointStore::put(&store, CheckpointBytes(Bytes::from_static(b"checkpoint"))).unwrap();
    let bytes = CheckpointStore::get(&store, id).unwrap();

    assert_eq!(bytes, CheckpointBytes(Bytes::from_static(b"checkpoint")));
    assert_eq!(CheckpointStore::latest(&store).unwrap(), Some(id));
}

#[test]
fn missing_blob_and_checkpoint_are_typed_store_errors() {
    let store = InlineStore::new("inline");

    let blob = BlobStore::get(
        &store,
        &BlobRef {
            store: "inline".to_owned(),
            key: "missing".to_owned(),
        },
    )
    .unwrap_err();
    assert!(matches!(blob, StoreError::BlobNotFound(_)));

    let checkpoint = CheckpointStore::get(&store, leaven_kernel::CheckpointId::new()).unwrap_err();
    assert!(matches!(
        checkpoint,
        StoreError::OperationFailed {
            operation: "get_checkpoint",
            retryable: Some(false),
            ..
        }
    ));
}

#[test]
fn default_store_uses_inline_namespace_and_rejects_wrong_blob_namespace() {
    let store = InlineStore::default();

    let reference = BlobStore::put(
        &store,
        BlobWrite {
            bytes: Bytes::from_static(b"blob"),
            content_type: None,
        },
    )
    .unwrap();
    assert_eq!(reference.store, "inline");

    let wrong_namespace = BlobStore::get(
        &store,
        &BlobRef {
            store: "other".to_owned(),
            key: reference.key,
        },
    )
    .unwrap_err();
    assert!(matches!(wrong_namespace, StoreError::BlobNotFound(_)));
}
