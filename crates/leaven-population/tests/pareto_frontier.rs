use leaven_core::PartitionId;
use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence};
use leaven_kernel::{AssessmentId, CandidateId, CaseId};
use leaven_population::{ParetoFrontier, PartitionFilter};
use proptest::prelude::*;
use std::collections::BTreeSet;

#[test]
fn dominated_candidate_is_not_admitted() {
    let mut frontier = ParetoFrontier::by_case().build();
    let strong = CandidateId::new();
    let weak = CandidateId::new();

    frontier.observe_casewise_scalar(
        strong,
        AssessmentId::new(),
        &casewise(&[(0, 1.0), (1, 1.0)]),
    );
    frontier.observe_casewise_scalar(weak, AssessmentId::new(), &casewise(&[(0, 0.5), (1, 0.5)]));

    assert!(frontier.contains(strong));
    assert!(!frontier.contains(weak));
}

#[test]
fn non_regressing_improvement_replaces_weaker_candidate() {
    let mut frontier = ParetoFrontier::by_case().build();
    let old = CandidateId::new();
    let new = CandidateId::new();

    frontier.observe_casewise_scalar(old, AssessmentId::new(), &casewise(&[(0, 0.5), (1, 0.5)]));
    frontier.observe_casewise_scalar(new, AssessmentId::new(), &casewise(&[(0, 0.5), (1, 0.9)]));

    assert!(!frontier.contains(old));
    assert!(frontier.contains(new));
    assert_eq!(frontier.best(), Some(new));
}

#[test]
fn incomparable_candidates_remain_together() {
    let mut frontier = ParetoFrontier::by_case().build();
    let left = CandidateId::new();
    let right = CandidateId::new();

    frontier.observe_casewise_scalar(left, AssessmentId::new(), &casewise(&[(0, 1.0), (1, 0.1)]));
    frontier.observe_casewise_scalar(right, AssessmentId::new(), &casewise(&[(0, 0.1), (1, 1.0)]));

    assert!(frontier.contains(left));
    assert!(frontier.contains(right));
}

#[test]
fn admission_is_order_independent() {
    let first = CandidateId::new();
    let second = CandidateId::new();
    let first_evidence = casewise(&[(0, 0.5), (1, 0.5)]);
    let second_evidence = casewise(&[(0, 0.5), (1, 0.9)]);

    let mut left_order = ParetoFrontier::by_case().build();
    left_order.observe_casewise_scalar(first, AssessmentId::new(), &first_evidence);
    left_order.observe_casewise_scalar(second, AssessmentId::new(), &second_evidence);

    let mut right_order = ParetoFrontier::by_case().build();
    right_order.observe_casewise_scalar(second, AssessmentId::new(), &second_evidence);
    right_order.observe_casewise_scalar(first, AssessmentId::new(), &first_evidence);

    assert_eq!(left_order.contains(first), right_order.contains(first));
    assert_eq!(left_order.contains(second), right_order.contains(second));
    assert_eq!(left_order.best(), right_order.best());
}

proptest! {
    #[test]
    fn generated_frontier_admission_is_order_independent(
        first_case_zero in 0.0_f64..1.0,
        first_case_one in 0.0_f64..1.0,
        second_case_zero in 0.0_f64..1.0,
        second_case_one in 0.0_f64..1.0,
    ) {
        let first = CandidateId::new();
        let second = CandidateId::new();
        let first_evidence = casewise(&[(0, first_case_zero), (1, first_case_one)]);
        let second_evidence = casewise(&[(0, second_case_zero), (1, second_case_one)]);

        let mut left_order = ParetoFrontier::by_case().build();
        left_order.observe_casewise_scalar(first, AssessmentId::new(), &first_evidence);
        left_order.observe_casewise_scalar(second, AssessmentId::new(), &second_evidence);

        let mut right_order = ParetoFrontier::by_case().build();
        right_order.observe_casewise_scalar(second, AssessmentId::new(), &second_evidence);
        right_order.observe_casewise_scalar(first, AssessmentId::new(), &first_evidence);

        prop_assert_eq!(left_order.contains(first), right_order.contains(first));
        prop_assert_eq!(left_order.contains(second), right_order.contains(second));
        prop_assert_eq!(left_order.best(), right_order.best());
    }
}

#[test]
fn observing_frontier_candidate_again_reweights_it() {
    let mut frontier = ParetoFrontier::by_case().build();
    let candidate = CandidateId::new();

    frontier.observe_casewise_scalar(candidate, AssessmentId::new(), &casewise(&[(0, 0.5)]));
    let events =
        frontier.observe_casewise_scalar(candidate, AssessmentId::new(), &casewise(&[(0, 0.9)]));

    assert!(frontier.contains(candidate));
    assert!(matches!(
        events.as_slice(),
        [leaven_engine::PopulationEvent::Reweighted { .. }]
    ));
}

#[test]
fn default_frontier_has_identity_and_no_best_candidate() {
    let frontier = ParetoFrontier::default();
    let id = frontier.id();

    assert_eq!(frontier.partition_filter(), &PartitionFilter::All);
    assert_eq!(frontier.id(), id);
    assert_eq!(frontier.best(), None);
}

#[test]
fn best_uses_average_score_and_stable_candidate_tiebreak() {
    let mut frontier = ParetoFrontier::by_case().build();
    let earlier = CandidateId::new();
    let later = CandidateId::new();

    frontier.observe_casewise_scalar(earlier, AssessmentId::new(), &casewise(&[(0, 0.9)]));
    frontier.observe_casewise_scalar(later, AssessmentId::new(), &casewise(&[(1, 0.9)]));

    assert!(frontier.contains(earlier));
    assert!(frontier.contains(later));
    assert_eq!(frontier.best(), Some(std::cmp::min(earlier, later)));
}

#[test]
fn sparse_candidates_do_not_dominate_missing_cases() {
    let mut frontier = ParetoFrontier::by_case().build();
    let case_zero = CandidateId::new();
    let case_one = CandidateId::new();

    frontier.observe_casewise_scalar(case_zero, AssessmentId::new(), &casewise(&[(0, 1.0)]));
    frontier.observe_casewise_scalar(case_one, AssessmentId::new(), &casewise(&[(1, 0.1)]));

    assert!(frontier.contains(case_zero));
    assert!(frontier.contains(case_one));
}

#[test]
fn empty_sparse_candidate_is_not_dominated_by_scored_candidate() {
    let mut frontier = ParetoFrontier::by_case().build();
    let scored = CandidateId::new();
    let empty = CandidateId::new();

    frontier.observe_casewise_scalar(scored, AssessmentId::new(), &casewise(&[(0, 1.0)]));
    frontier.observe_casewise_scalar(empty, AssessmentId::new(), &casewise(&[]));

    assert!(frontier.contains(scored));
    assert!(frontier.contains(empty));
}

#[test]
fn empty_casewise_observation_has_no_best_score() {
    let mut frontier = ParetoFrontier::by_case().build();
    let candidate = CandidateId::new();

    frontier.observe_casewise_scalar(candidate, AssessmentId::new(), &casewise(&[]));

    assert!(frontier.contains(candidate));
    assert_eq!(frontier.best(), None);
}

#[test]
fn builder_preserves_partition_filter() {
    let filter = BTreeSet::from([PartitionId::from("TRAIN")]);
    let frontier = ParetoFrontier::by_case()
        .partition_filter(filter.clone())
        .build();

    assert_eq!(frontier.partition_filter(), &PartitionFilter::Only(filter));
}

#[test]
fn partition_filter_excludes_non_matching_observation_before_update() {
    let train = PartitionId::from("TRAIN");
    let validation = PartitionId::from("VALIDATION");
    let mut frontier = ParetoFrontier::by_case()
        .partition_filter(BTreeSet::from([train.clone()]))
        .build();
    let candidate = CandidateId::new();

    let events = frontier.observe_partitioned_casewise_scalar(
        &validation,
        candidate,
        AssessmentId::new(),
        &casewise(&[(0, 1.0)]),
    );

    assert!(!frontier.contains(candidate));
    assert!(matches!(
        events.as_slice(),
        [leaven_engine::PopulationEvent::Ignored { .. }]
    ));

    frontier.observe_partitioned_casewise_scalar(
        &train,
        candidate,
        AssessmentId::new(),
        &casewise(&[(0, 1.0)]),
    );

    assert!(frontier.contains(candidate));
}

#[test]
fn partition_filter_excludes_unpartitioned_observation_before_update() {
    let mut frontier = ParetoFrontier::by_case()
        .partition_filter(BTreeSet::from([PartitionId::from("TRAIN")]))
        .build();
    let candidate = CandidateId::new();

    frontier.observe_casewise_scalar(candidate, AssessmentId::new(), &casewise(&[(0, 1.0)]));

    assert!(!frontier.contains(candidate));
    assert_eq!(frontier.best(), None);
}

fn casewise(scores: &[(u64, f64)]) -> CasewiseEvidence<ScalarEvidence> {
    CasewiseEvidence::new(
        scores
            .iter()
            .map(|(case, score)| {
                CaseOutcome::new(CaseId::new(*case), ScalarEvidence::new(*score).unwrap())
            })
            .collect(),
    )
}
