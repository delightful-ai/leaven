use std::collections::{BTreeMap, BTreeSet};

use leaven_core::{CaseSetVersion, PartitionId};
use leaven_eval::{
    Case, Dataset, DatasetError, DatasetSplits, DatasetSplitsError, EvaluationUse, FinalTestPolicy,
    RowOrderSplitBuilder, SplitPolicy, SplitRole, SplitUse, SplitUsePolicy, SplitUsePolicyError,
    StratifiedSplitBuilder,
};
use leaven_kernel::{CaseId, MetadataBag, MetadataKey, MetadataValue};

#[test]
fn dataset_fingerprint_tracks_ordered_case_membership() {
    let left = Dataset::from_ordered(vec!["a", "b"]);
    let right = Dataset::from_ordered(vec!["a", "b", "c"]);

    assert_ne!(left.fingerprint(), right.fingerprint());
    assert_eq!(
        left.cases().keys().copied().collect::<Vec<_>>(),
        vec![CaseId::from_index(0), CaseId::from_index(1),]
    );
    assert!(left.metadata().is_empty());
}

#[test]
fn dataset_builder_refuses_duplicate_explicit_case_ids() {
    let duplicate = Dataset::builder()
        .case(CaseId::from_index(7), "first")
        .unwrap()
        .case(CaseId::from_index(7), "second");
    let Err(error) = duplicate else {
        panic!("expected duplicate case id error");
    };

    assert_eq!(error, DatasetError::DuplicateCase(CaseId::from_index(7)));
}

#[test]
fn dataset_from_case_envelopes_preserves_ids_and_metadata() {
    let mut metadata = MetadataBag::new();
    metadata.insert("source_id", MetadataValue::String("aime/2024/1".to_owned()));
    let dataset = Dataset::from_cases(vec![
        Case::targeted(CaseId::new(42), "problem", "answer").with_metadata(metadata),
    ])
    .unwrap();

    let case = dataset
        .cases()
        .get(&CaseId::new(42))
        .expect("case id is preserved");
    assert_eq!(case.input, "problem");
    assert_eq!(case.target, Some("answer"));
    let source_id = case
        .metadata
        .get(&MetadataKey::from("source_id"))
        .expect("metadata is preserved");
    let MetadataValue::String(source_id) = source_id else {
        panic!("source id should be stored as string metadata");
    };
    assert_eq!(source_id, "aime/2024/1");

    let duplicate = Dataset::from_cases(vec![
        Case::input(CaseId::new(42), "first"),
        Case::input(CaseId::new(42), "second"),
    ])
    .unwrap_err();
    assert_eq!(duplicate, DatasetError::DuplicateCase(CaseId::new(42)));
}

#[test]
fn split_roles_map_to_conventional_partition_ids() {
    assert_eq!(SplitRole::Train.partition_id(), PartitionId::from("TRAIN"));
    assert_eq!(
        SplitRole::Validation.partition_id(),
        PartitionId::from("VALIDATION")
    );
    assert_eq!(SplitRole::Test.partition_id(), PartitionId::from("TEST"));
    assert_eq!(
        SplitRole::Search.partition_id(),
        PartitionId::from("SEARCH")
    );
    assert_eq!(SplitRole::Probe.partition_id(), PartitionId::from("PROBE"));
    assert_eq!(
        SplitRole::ReportOnly.partition_id(),
        PartitionId::from("REPORT_ONLY")
    );
    assert_eq!(
        SplitRole::Custom("audit".into()).partition_id(),
        PartitionId::from("audit")
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
fn split_policy_refuses_unknown_cases_but_can_allow_documented_overlap() {
    let known = BTreeSet::from([CaseId::from_index(0), CaseId::from_index(1)]);
    let roles = BTreeMap::from([
        (PartitionId::from("TRAIN"), SplitRole::Train),
        (PartitionId::from("PROBE"), SplitRole::Probe),
    ]);
    let overlapping = BTreeMap::from([
        (PartitionId::from("TRAIN"), vec![CaseId::from_index(0)]),
        (PartitionId::from("PROBE"), vec![CaseId::from_index(0)]),
    ]);

    let allowed = DatasetSplits::new(
        CaseSetVersion("v1".to_owned()),
        roles.clone(),
        overlapping,
        &known,
        SplitPolicy::OverlapAllowed {
            reason: "probe cases mirror train for calibration".to_owned(),
        },
    )
    .unwrap();
    assert!(matches!(
        allowed.policy(),
        SplitPolicy::OverlapAllowed { .. }
    ));
    assert_eq!(
        allowed.cases(&PartitionId::from("PROBE")),
        Some([CaseId::from_index(0)].as_slice())
    );

    let unknown = DatasetSplits::new(
        CaseSetVersion("v1".to_owned()),
        roles,
        BTreeMap::from([(PartitionId::from("TRAIN"), vec![CaseId::from_index(99)])]),
        &known,
        SplitPolicy::DisjointRequired,
    )
    .unwrap_err();
    assert_eq!(
        unknown,
        DatasetSplitsError::UnknownCase(CaseId::from_index(99))
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
    assert_eq!(left.version(), &CaseSetVersion("v1".to_owned()));
    assert_eq!(left.policy(), &SplitPolicy::DisjointRequired);
}

#[test]
fn exact_stratified_split_builder_allocates_requested_counts_by_category() {
    let splits = StratifiedSplitBuilder::new(BTreeMap::from([
        (
            "easy".into(),
            vec![
                CaseId::from_index(0),
                CaseId::from_index(1),
                CaseId::from_index(2),
                CaseId::from_index(3),
                CaseId::from_index(4),
            ],
        ),
        (
            "hard".into(),
            vec![
                CaseId::from_index(10),
                CaseId::from_index(11),
                CaseId::from_index(12),
                CaseId::from_index(13),
                CaseId::from_index(14),
                CaseId::from_index(15),
                CaseId::from_index(16),
            ],
        ),
    ]))
    .unwrap()
    .role_count(SplitRole::Train, 5)
    .role_count(SplitRole::Validation, 3)
    .role_count(SplitRole::Test, 2)
    .build(CaseSetVersion("officeqa-categories-v1".to_owned()))
    .unwrap();

    assert_eq!(
        splits.cases(&SplitRole::Train.partition_id()),
        Some(
            [
                CaseId::from_index(0),
                CaseId::from_index(1),
                CaseId::from_index(10),
                CaseId::from_index(11),
                CaseId::from_index(12),
            ]
            .as_slice()
        )
    );
    assert_eq!(
        splits.cases(&SplitRole::Validation.partition_id()),
        Some(
            [
                CaseId::from_index(2),
                CaseId::from_index(13),
                CaseId::from_index(14),
            ]
            .as_slice()
        )
    );
    assert_eq!(
        splits.cases(&SplitRole::Test.partition_id()),
        Some([CaseId::from_index(3), CaseId::from_index(15)].as_slice())
    );
    assert_eq!(splits.policy(), &SplitPolicy::DisjointRequired);
}

#[test]
fn exact_stratified_split_builder_refuses_ambiguous_or_oversized_manifests() {
    let duplicate = StratifiedSplitBuilder::new(BTreeMap::from([
        ("easy".into(), vec![CaseId::from_index(0)]),
        ("hard".into(), vec![CaseId::from_index(0)]),
    ]))
    .unwrap_err();
    assert_eq!(
        duplicate,
        DatasetSplitsError::DuplicateStratifiedCase {
            case: CaseId::from_index(0),
            left: "easy".into(),
            right: "hard".into(),
        }
    );

    let oversized = StratifiedSplitBuilder::new(BTreeMap::from([(
        "easy".into(),
        vec![CaseId::from_index(0), CaseId::from_index(1)],
    )]))
    .unwrap()
    .role_count(SplitRole::Train, 2)
    .role_count(SplitRole::Validation, 1)
    .build(CaseSetVersion("too-large".to_owned()))
    .unwrap_err();
    assert_eq!(
        oversized,
        DatasetSplitsError::InsufficientStratifiedCases {
            requested: 3,
            available: 2,
        }
    );

    let duplicate_role = StratifiedSplitBuilder::new(BTreeMap::from([(
        "easy".into(),
        vec![CaseId::from_index(0), CaseId::from_index(1)],
    )]))
    .unwrap()
    .role_count(SplitRole::Train, 1)
    .role_count(SplitRole::Train, 1)
    .build(CaseSetVersion("duplicate-role".to_owned()))
    .unwrap_err();
    assert_eq!(
        duplicate_role,
        DatasetSplitsError::DuplicateSplitRole(SplitRole::Train)
    );
}

#[test]
fn row_order_split_builder_preserves_upstream_slice_ranges() {
    let ordered_cases = (0..400).map(CaseId::from_index).collect::<Vec<_>>();
    let splits = RowOrderSplitBuilder::new(ordered_cases)
        .role_range(SplitRole::Train, 0..200)
        .role_range(SplitRole::Test, 200..400)
        .build(CaseSetVersion(
            "trace2skill-spreadsheetbench-verified-400-v1".to_owned(),
        ))
        .unwrap();

    let train = splits
        .cases(&SplitRole::Train.partition_id())
        .expect("train slice exists");
    let test = splits
        .cases(&SplitRole::Test.partition_id())
        .expect("test slice exists");

    assert_eq!(train.len(), 200);
    assert_eq!(test.len(), 200);
    assert_eq!(train.first(), Some(&CaseId::from_index(0)));
    assert_eq!(train.last(), Some(&CaseId::from_index(199)));
    assert_eq!(test.first(), Some(&CaseId::from_index(200)));
    assert_eq!(test.last(), Some(&CaseId::from_index(399)));
    assert_eq!(splits.policy(), &SplitPolicy::DisjointRequired);
}

#[test]
fn row_order_split_builder_refuses_invalid_ranges_and_duplicate_roles() {
    let duplicate_role = RowOrderSplitBuilder::new(vec![
        CaseId::from_index(0),
        CaseId::from_index(1),
        CaseId::from_index(2),
    ])
    .role_range(SplitRole::Train, 0..1)
    .role_range(SplitRole::Train, 1..2)
    .build(CaseSetVersion("duplicate-role".to_owned()))
    .unwrap_err();
    assert_eq!(
        duplicate_role,
        DatasetSplitsError::DuplicateSplitRole(SplitRole::Train)
    );

    let invalid_range = RowOrderSplitBuilder::new(vec![CaseId::from_index(0)])
        .role_range(SplitRole::Test, 0..2)
        .build(CaseSetVersion("invalid-range".to_owned()))
        .unwrap_err();
    assert_eq!(
        invalid_range,
        DatasetSplitsError::InvalidRowRange {
            role: SplitRole::Test,
            start: 0,
            end: 2,
            len: 1,
        }
    );
}

#[test]
fn split_use_policy_preserves_optimizer_and_final_test_boundaries() {
    let policy = SplitUsePolicy::gepa_train_val_test();

    let train = policy.use_for(&PartitionId::from("TRAIN"));
    assert!(train.allows(&EvaluationUse::ProposerFeedback));
    assert!(train.allows(&EvaluationUse::ParentSelection));
    assert!(train.allows(&EvaluationUse::PartSelection));
    assert!(train.allows(&EvaluationUse::CandidateAcceptance));
    assert!(train.allows(&EvaluationUse::PopulationObservation));
    assert!(train.allows(&EvaluationUse::Report));
    assert!(!train.allows(&EvaluationUse::FinalTest));

    let validation = policy.use_for(&PartitionId::from("VALIDATION"));
    assert!(validation.allows(&EvaluationUse::Report));
    assert!(!validation.allows(&EvaluationUse::CandidateAcceptance));

    let test = policy.use_for(&PartitionId::from("TEST"));
    assert!(test.allows(&EvaluationUse::FinalTest));
    assert!(test.allows(&EvaluationUse::Report));
    assert!(!test.allows(&EvaluationUse::ProposerFeedback));

    let unknown = policy.use_for(&PartitionId::from("AD_HOC"));
    assert!(unknown.allows(&EvaluationUse::Report));
    assert!(!unknown.allows(&EvaluationUse::PopulationObservation));
    assert_eq!(policy.final_test(), &FinalTestPolicy::FinalReportOnly);
    assert_eq!(FinalTestPolicy::Disabled, FinalTestPolicy::Disabled);
    assert_eq!(
        FinalTestPolicy::ExplicitlyAllowedInLoop {
            reason: "benchmark exception".to_owned(),
        },
        FinalTestPolicy::ExplicitlyAllowedInLoop {
            reason: "benchmark exception".to_owned(),
        }
    );
}

#[test]
fn evaluator_only_split_use_cannot_mix_with_optimizer_uses() {
    let evaluator_only = SplitUse::new([EvaluationUse::EvaluatorOnly]).unwrap();
    assert!(evaluator_only.allows(&EvaluationUse::EvaluatorOnly));
    assert!(!evaluator_only.allows(&EvaluationUse::Report));

    let error = SplitUse::new([EvaluationUse::EvaluatorOnly, EvaluationUse::Report]).unwrap_err();

    assert_eq!(error, SplitUsePolicyError::ContradictoryEvaluatorOnly);
}
