use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use leaven_eval::{CategoryRoundRobinSampler, SamplerError};
use leaven_kernel::CaseId;

#[test]
fn category_round_robin_sampler_cycles_categories_and_cases_without_replacement() {
    let mut sampler = CategoryRoundRobinSampler::new(
        BTreeMap::from([
            (
                "alpha".into(),
                vec![
                    CaseId::from_index(0),
                    CaseId::from_index(1),
                    CaseId::from_index(2),
                ],
            ),
            (
                "beta".into(),
                vec![CaseId::from_index(10), CaseId::from_index(11)],
            ),
        ]),
        NonZeroUsize::new(2).unwrap(),
        NonZeroUsize::new(2).unwrap(),
    )
    .unwrap();
    assert!(sampler.sampled_cases().is_empty());

    let first = sampler.next_batch();
    assert_eq!(
        first.iter().map(|sample| sample.case).collect::<Vec<_>>(),
        vec![
            CaseId::from_index(0),
            CaseId::from_index(1),
            CaseId::from_index(10),
            CaseId::from_index(11),
        ]
    );
    assert_eq!(
        sampler.sampled_cases(),
        [
            CaseId::from_index(0),
            CaseId::from_index(1),
            CaseId::from_index(10),
            CaseId::from_index(11)
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(sampler.category_cursor(), 0);

    let second = sampler.next_batch();
    assert_eq!(
        second.iter().map(|sample| sample.case).collect::<Vec<_>>(),
        vec![
            CaseId::from_index(2),
            CaseId::from_index(0),
            CaseId::from_index(10),
            CaseId::from_index(11),
        ]
    );
    assert_eq!(sampler.category_cursor(), 0);
}

#[test]
fn category_round_robin_sampler_state_is_checkpointable() {
    let mut sampler = CategoryRoundRobinSampler::new(
        BTreeMap::from([
            (
                "alpha".into(),
                vec![CaseId::from_index(0), CaseId::from_index(1)],
            ),
            (
                "beta".into(),
                vec![CaseId::from_index(10), CaseId::from_index(11)],
            ),
            ("gamma".into(), vec![CaseId::from_index(20)]),
        ]),
        NonZeroUsize::new(2).unwrap(),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    assert_eq!(
        sampler
            .next_batch()
            .into_iter()
            .map(|sample| sample.category.to_string())
            .collect::<Vec<_>>(),
        vec!["alpha", "beta"]
    );

    let restored = serde_json::from_str::<CategoryRoundRobinSampler>(
        &serde_json::to_string(&sampler).unwrap(),
    )
    .unwrap();
    sampler = restored;

    let resumed = sampler.next_batch();
    assert_eq!(
        resumed
            .iter()
            .map(|sample| (sample.category.as_str(), sample.case))
            .collect::<Vec<_>>(),
        vec![
            ("gamma", CaseId::from_index(20)),
            ("alpha", CaseId::from_index(1)),
        ]
    );
}

#[test]
fn category_round_robin_sampler_refuses_empty_or_duplicated_pools() {
    let empty = CategoryRoundRobinSampler::new(
        BTreeMap::new(),
        NonZeroUsize::new(1).unwrap(),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap_err();
    assert_eq!(empty, SamplerError::NoCategories);

    let empty_category = CategoryRoundRobinSampler::new(
        BTreeMap::from([("alpha".into(), Vec::new())]),
        NonZeroUsize::new(1).unwrap(),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap_err();
    assert_eq!(empty_category, SamplerError::EmptyCategory("alpha".into()));

    let duplicate_case = CategoryRoundRobinSampler::new(
        BTreeMap::from([
            ("alpha".into(), vec![CaseId::from_index(0)]),
            ("beta".into(), vec![CaseId::from_index(0)]),
        ]),
        NonZeroUsize::new(1).unwrap(),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap_err();
    assert_eq!(
        duplicate_case,
        SamplerError::DuplicateCaseCategory {
            case: CaseId::from_index(0),
            left: "alpha".into(),
            right: "beta".into(),
        }
    );
}
