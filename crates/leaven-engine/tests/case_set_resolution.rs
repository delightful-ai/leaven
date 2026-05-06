use leaven_core::{EvaluationSet, PartitionId, Tag, Window};
use leaven_engine::{CaseSet, EvaluationResolveError};
use leaven_kernel::CaseId;

#[test]
fn all_and_unscoped_resolve_to_every_case() {
    let cases = CaseSet::new(vec!["a", "b", "c"]);

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
fn sample_recent_and_stratified_are_deterministic_subsets() {
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
            &EvaluationSet::Stratified {
                of: Box::new(EvaluationSet::All),
                k: 2,
                by: Tag("unused".into()),
                seed: 0,
            }
        ),
        vec![0, 1]
    );
}

#[test]
fn unsupported_tagged_set_is_a_typed_error() {
    let cases = CaseSet::new(vec!["a"]);

    assert!(matches!(
        cases.resolve(&EvaluationSet::Tagged(Tag("gold".into()))),
        Err(EvaluationResolveError::UnsupportedSet(_))
    ));
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
