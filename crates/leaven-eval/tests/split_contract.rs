use std::collections::{BTreeMap, BTreeSet};

use leaven_core::{CaseSetVersion, PartitionId};
use leaven_eval::{Dataset, DatasetSplits, DatasetSplitsError, SplitPolicy, SplitRole};
use leaven_kernel::CaseId;

#[test]
fn dataset_fingerprint_tracks_ordered_case_membership() {
    let left = Dataset::from_ordered(vec!["a", "b"]);
    let right = Dataset::from_ordered(vec!["a", "b", "c"]);

    assert_ne!(left.fingerprint(), right.fingerprint());
    assert_eq!(
        left.cases().keys().copied().collect::<Vec<_>>(),
        vec![CaseId::from_index(0), CaseId::from_index(1),]
    );
}

#[test]
fn disjoint_split_policy_refuses_overlap() {
    let known = BTreeSet::from([CaseId::from_index(0), CaseId::from_index(1)]);
    let roles = BTreeMap::from([
        (PartitionId::from("TRAIN"), SplitRole::Train),
        (PartitionId::from("TEST"), SplitRole::Test),
    ]);
    let cases = BTreeMap::from([
        (PartitionId::from("TRAIN"), vec![CaseId::from_index(0)]),
        (PartitionId::from("TEST"), vec![CaseId::from_index(0)]),
    ]);

    let error = DatasetSplits::new(
        CaseSetVersion("v1".to_owned()),
        roles,
        cases,
        &known,
        SplitPolicy::DisjointRequired,
    )
    .unwrap_err();

    let DatasetSplitsError::OverlappingCase { case, left, right } = error else {
        panic!("expected overlap error");
    };
    assert_eq!(case, CaseId::from_index(0));
    assert_eq!(
        BTreeSet::from([left, right]),
        BTreeSet::from([SplitRole::Train, SplitRole::Test])
    );
}

#[test]
fn split_fingerprint_tracks_roles_and_membership() {
    let known = BTreeSet::from([
        CaseId::from_index(0),
        CaseId::from_index(1),
        CaseId::from_index(2),
    ]);
    let roles = BTreeMap::from([
        (PartitionId::from("TRAIN"), SplitRole::Train),
        (PartitionId::from("VALIDATION"), SplitRole::Validation),
    ]);
    let left = DatasetSplits::new(
        CaseSetVersion("v1".to_owned()),
        roles.clone(),
        BTreeMap::from([
            (PartitionId::from("TRAIN"), vec![CaseId::from_index(0)]),
            (PartitionId::from("VALIDATION"), vec![CaseId::from_index(1)]),
        ]),
        &known,
        SplitPolicy::DisjointRequired,
    )
    .unwrap();
    let right = DatasetSplits::new(
        CaseSetVersion("v1".to_owned()),
        roles,
        BTreeMap::from([
            (PartitionId::from("TRAIN"), vec![CaseId::from_index(0)]),
            (PartitionId::from("VALIDATION"), vec![CaseId::from_index(2)]),
        ]),
        &known,
        SplitPolicy::DisjointRequired,
    )
    .unwrap();

    assert_ne!(left.fingerprint(), right.fingerprint());
    assert_eq!(
        left.role(&PartitionId::from("VALIDATION")),
        Some(&SplitRole::Validation)
    );
}
