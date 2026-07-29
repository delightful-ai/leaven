use leaven_core::{EvaluationSet, PartitionId, Tag, Window};
use leaven_engine::{CaseSet, EvaluationResolveError, UnsupportedEvaluationSet};
use leaven_kernel::CaseId;

#[test]
fn all_and_unscoped_resolve_to_every_case() {
    let cases = CaseSet::builder().cases(vec!["a", "b", "c"]).build();

    assert_eq!(ids(&cases, &EvaluationSet::All), vec![0, 1, 2]);
    assert_eq!(ids(&cases, &EvaluationSet::Unscoped), vec![0, 1, 2]);
}

#[test]
fn partitions_and_explicit_cases_resolve_or_refuse_unknown_ids() {
    let train = PartitionId::from("train");
    let cases = CaseSet::new(vec!["a", "b", "c"])
        .with_partition(train.clone(), vec![CaseId::new(0), CaseId::new(2)]);

    assert_eq!(ids(&cases, &EvaluationSet::Partition(train)), vec![0, 2]);
    assert_eq!(
        ids(&cases, &EvaluationSet::Cases(vec![CaseId::new(1)])),
        vec![1]
    );
    assert!(matches!(
        cases.resolve(&EvaluationSet::Cases(vec![CaseId::new(99)])),
        Err(EvaluationResolveError::UnknownCase(_))
    ));
    assert!(matches!(
        cases.resolve(&EvaluationSet::Partition(PartitionId::from("missing"))),
        Err(EvaluationResolveError::UnknownPartition(_))
    ));
}

#[test]
fn partitions_containing_maps_cases_back_to_named_membership() {
    let train = PartitionId::from("TRAIN");
    let test = PartitionId::from("TEST");
    let cases = CaseSet::new(vec!["a", "b", "c"])
        .with_partition(train.clone(), vec![CaseId::new(0), CaseId::new(1)])
        .with_partition(test.clone(), vec![CaseId::new(2)]);

    assert_eq!(cases.partitions_containing(CaseId::new(0)), vec![train]);
    assert_eq!(cases.partitions_containing(CaseId::new(2)), vec![test.clone()]);
    assert_eq!(
        cases.hidden_partitions_for_cases(&[CaseId::new(0), CaseId::new(2)], &[test.clone()]),
        vec![test]
    );
    assert!(
        cases
            .hidden_partitions_for_cases(&[CaseId::new(0)], &[PartitionId::from("TEST")])
            .is_empty()
    );
}

#[test]
fn get_returns_present_cases_and_none_for_unknown_ids() {
    let cases = CaseSet::new(vec!["a", "b", "c"]);

    assert_eq!(cases.get(CaseId::from_index(0)), Some(&"a"));
    assert_eq!(cases.get(CaseId::from_index(2)), Some(&"c"));
    assert_eq!(cases.get(CaseId::new(99)), None);
}

#[test]
fn explicit_case_ids_resolve_and_lookup_without_positional_rewrite() {
    let train = PartitionId::from("train");
    let alpha = CaseId::new(700);
    let beta = CaseId::new(900);
    let cases = CaseSet::from_entries([(alpha, "a"), (beta, "b")])
        .with_partition(train.clone(), vec![beta, alpha]);

    assert_eq!(ids(&cases, &EvaluationSet::All), vec![700, 900]);
    assert_eq!(
        ids(&cases, &EvaluationSet::Partition(train)),
        vec![900, 700]
    );
    assert_eq!(cases.get(alpha), Some(&"a"));
    assert_eq!(cases.get(CaseId::from_index(0)), None);
}

#[test]
fn set_combinators_resolve_stably() {
    let left = EvaluationSet::Cases(vec![CaseId::new(0), CaseId::new(1)]);
    let right = EvaluationSet::Cases(vec![CaseId::new(1), CaseId::new(2)]);
    let cases = CaseSet::new(vec!["a", "b", "c"]);

    assert_eq!(
        ids(
            &cases,
            &EvaluationSet::Union(vec![right.clone(), left.clone()])
        ),
        vec![0, 1, 2]
    );
    assert_eq!(
        ids(
            &cases,
            &EvaluationSet::Intersect(vec![left.clone(), right.clone()])
        ),
        vec![1]
    );
    assert_eq!(
        ids(
            &cases,
            &EvaluationSet::Difference(Box::new(left), Box::new(right))
        ),
        vec![0]
    );
}

#[test]
fn sample_and_recent_are_deterministic_subsets() {
    let cases = CaseSet::new(vec!["a", "b", "c", "d"]);

    assert_eq!(
        ids(
            &cases,
            &EvaluationSet::Recent {
                window: Window { limit: 2 }
            }
        ),
        vec![3, 2]
    );
    assert_eq!(
        ids(
            &cases,
            &EvaluationSet::Sample {
                of: Box::new(EvaluationSet::All),
                n: 2,
                seed: 1,
            }
        ),
        vec![1, 2]
    );
    assert_eq!(
        ids(
            &cases,
            &EvaluationSet::Sample {
                of: Box::new(EvaluationSet::Intersect(Vec::new())),
                n: 2,
                seed: 1,
            }
        ),
        Vec::<u64>::new()
    );
}

#[test]
fn tag_index_dependent_sets_are_typed_unsupported_errors() {
    let cases = CaseSet::new(vec!["a"]);

    let err = cases
        .resolve(&EvaluationSet::Tagged(Tag("gold".into())))
        .unwrap_err();

    assert!(matches!(
        err,
        EvaluationResolveError::UnsupportedSet(UnsupportedEvaluationSet::Tagged(Tag(tag)))
            if tag.as_str() == "gold"
    ));
    assert_eq!(
        UnsupportedEvaluationSet::Tagged(Tag("gold".into())).to_string(),
        "tagged:gold"
    );

    let err = cases
        .resolve(&EvaluationSet::Stratified {
            of: Box::new(EvaluationSet::All),
            k: 1,
            by: Tag("difficulty".into()),
            seed: 0,
        })
        .unwrap_err();

    assert!(matches!(
        err,
        EvaluationResolveError::UnsupportedSet(UnsupportedEvaluationSet::Stratified { by: Tag(tag) })
            if tag.as_str() == "difficulty"
    ));
    assert_eq!(
        (UnsupportedEvaluationSet::Stratified {
            by: Tag("difficulty".into())
        })
        .to_string(),
        "stratified-by:difficulty"
    );
}

#[test]
fn empty_intersection_resolves_to_empty_set() {
    let cases = CaseSet::new(vec!["a", "b"]);

    assert_eq!(
        ids(&cases, &EvaluationSet::Intersect(Vec::new())),
        Vec::<u64>::new()
    );
}

fn ids(cases: &CaseSet<&'static str>, set: &EvaluationSet) -> Vec<u64> {
    cases
        .resolve(set)
        .unwrap()
        .case_ids
        .into_iter()
        .map(|id| id.0)
        .collect()
}
