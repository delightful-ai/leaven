use leaven_kernel::EvidenceRef;
use leaven_store::{Evidence, EvidenceStore, StoreError};
use leaven_store_inline::InlineEvidenceStore;

#[derive(Clone, Debug, Eq, PartialEq)]
struct TestEvidence(&'static str);

impl Evidence for TestEvidence {}

#[test]
fn evidence_round_trips_by_reference() {
    let store = InlineEvidenceStore::new("inline");

    let reference = store.put(TestEvidence("score")).unwrap();
    let evidence = store.get(&reference).unwrap();

    assert_eq!(reference.store, "inline");
    assert_eq!(reference.key, "0");
    assert_eq!(evidence, TestEvidence("score"));
}

#[test]
fn wrong_store_name_is_not_found() {
    let store = InlineEvidenceStore::<TestEvidence>::new("inline");

    let err = store
        .get(&EvidenceRef {
            store: "other".to_owned(),
            key: "0".to_owned(),
        })
        .unwrap_err();

    assert!(matches!(err, StoreError::EvidenceNotFound(_)));
}

#[test]
fn missing_evidence_key_is_not_found() {
    let store = InlineEvidenceStore::<TestEvidence>::new("inline");

    let err = store
        .get(&EvidenceRef {
            store: "inline".to_owned(),
            key: "missing".to_owned(),
        })
        .unwrap_err();

    assert!(matches!(err, StoreError::EvidenceNotFound(_)));
}
