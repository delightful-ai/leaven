use std::collections::{BTreeMap, BTreeSet};

use leaven_core::PartitionId;
use leaven_eval::{Case, Dataset, DatasetSplits, NoTarget, SplitPolicy, SplitRole};
use leaven_kernel::{CaseId, Fingerprint};

use crate::{OptimizeError, compatibility::case_set_version};

pub(super) struct CasePlan<I, T> {
    pub(super) dataset: Dataset<Case<I, T>>,
    pub(super) splits: DatasetSplits,
    pub(super) case_set: leaven_engine::CaseSet<Case<I, T>>,
}

pub(super) fn cases_from_inputs<I>(start: usize, inputs: Vec<I>) -> Vec<Case<I, NoTarget>> {
    inputs
        .into_iter()
        .enumerate()
        .map(|(offset, input)| Case::input(CaseId::from_index(start + offset), input))
        .collect()
}

pub(super) fn build_case_plan<I, T>(
    train: &[Case<I, T>],
    validation: &[Case<I, T>],
    test: &[Case<I, T>],
    case_content: Fingerprint,
) -> Result<CasePlan<I, T>, OptimizeError>
where
    I: Clone,
    T: Clone,
{
    let all_cases = all_cases(train, validation, test);
    let dataset = Dataset::from_cases(all_cases.clone())?;
    let splits = dataset_splits(train, validation, test, case_content);
    let case_set = case_set(all_cases, train.len(), validation.len(), test.len());
    Ok(CasePlan {
        dataset,
        splits,
        case_set,
    })
}

fn all_cases<I: Clone, T: Clone>(
    train: &[Case<I, T>],
    validation: &[Case<I, T>],
    test: &[Case<I, T>],
) -> Vec<Case<I, T>> {
    train
        .iter()
        .chain(validation)
        .chain(test)
        .cloned()
        .collect()
}

pub(super) fn case_set_cases<I: Clone, T: Clone>(
    train: &[Case<I, T>],
    validation: &[Case<I, T>],
    test: &[Case<I, T>],
) -> Vec<Case<I, T>> {
    all_cases(train, validation, test)
}

fn case_set<I: Clone, T: Clone>(
    cases: Vec<Case<I, T>>,
    train: usize,
    validation: usize,
    test: usize,
) -> leaven_engine::CaseSet<Case<I, T>> {
    let train_ids = cases
        .iter()
        .take(train)
        .map(|case| case.id)
        .collect::<Vec<_>>();
    let validation_ids = cases
        .iter()
        .skip(train)
        .take(validation)
        .map(|case| case.id)
        .collect::<Vec<_>>();
    let test_ids = cases
        .iter()
        .skip(train + validation)
        .take(test)
        .map(|case| case.id)
        .collect::<Vec<_>>();
    let entries = cases.into_iter().map(|case| (case.id, case));
    leaven_engine::CaseSet::from_entries(entries)
        .with_partition(PartitionId::from("TRAIN"), train_ids)
        .with_partition(PartitionId::from("VALIDATION"), validation_ids)
        .with_partition(PartitionId::from("TEST"), test_ids)
}

fn dataset_splits<I, T>(
    train: &[Case<I, T>],
    validation: &[Case<I, T>],
    test: &[Case<I, T>],
    case_content: Fingerprint,
) -> DatasetSplits {
    let known = train
        .iter()
        .chain(validation)
        .chain(test)
        .map(|case| case.id)
        .collect::<BTreeSet<_>>();
    let roles = BTreeMap::from([
        (PartitionId::from("TRAIN"), SplitRole::Train),
        (PartitionId::from("VALIDATION"), SplitRole::Validation),
        (PartitionId::from("TEST"), SplitRole::Test),
    ]);
    let cases = BTreeMap::from([
        (
            PartitionId::from("TRAIN"),
            train.iter().map(|case| case.id).collect(),
        ),
        (
            PartitionId::from("VALIDATION"),
            validation.iter().map(|case| case.id).collect(),
        ),
        (
            PartitionId::from("TEST"),
            test.iter().map(|case| case.id).collect(),
        ),
    ]);
    DatasetSplits::new(
        case_set_version(case_content),
        roles,
        cases,
        &known,
        SplitPolicy::DisjointRequired,
    )
    .expect("builder constructs disjoint split ids")
}
