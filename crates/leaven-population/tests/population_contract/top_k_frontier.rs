use std::num::NonZeroUsize;

use leaven_evidence::ScalarEvidence;
use leaven_kernel::{AssessmentId, CandidateId};
use leaven_population::{TopKFrontier, TopKParentSelectionPolicy, TopKParentSelector};

#[test]
fn top_k_frontier_fills_to_capacity_then_evicts_weakest_only_on_improvement() {
    let mut frontier = TopKFrontier::new(NonZeroUsize::new(3).unwrap());
    let low = CandidateId::new();
    let mid = CandidateId::new();
    let high = CandidateId::new();
    let better_than_low = CandidateId::new();
    let tied_with_low = CandidateId::new();

    assert!(matches!(
        frontier
            .observe(low, AssessmentId::new(), ScalarEvidence::new(0.1).unwrap())
            .as_slice(),
        [leaven_engine::PopulationEvent::Inserted { .. }]
    ));
    frontier.observe(mid, AssessmentId::new(), ScalarEvidence::new(0.4).unwrap());
    frontier.observe(high, AssessmentId::new(), ScalarEvidence::new(0.9).unwrap());

    let tie_events = frontier.observe(
        tied_with_low,
        AssessmentId::new(),
        ScalarEvidence::new(0.1).unwrap(),
    );
    assert!(matches!(
        tie_events.as_slice(),
        [leaven_engine::PopulationEvent::Ignored { .. }]
    ));
    assert!(!frontier.contains(tied_with_low));
    assert_eq!(frontier.weakest(), Some(low));

    let replace_events = frontier.observe(
        better_than_low,
        AssessmentId::new(),
        ScalarEvidence::new(0.2).unwrap(),
    );
    assert!(matches!(
        replace_events.as_slice(),
        [leaven_engine::PopulationEvent::Replaced { old, new, .. }]
            if *old == low && *new == better_than_low
    ));

    assert_eq!(frontier.best(), Some(high));
    assert_eq!(frontier.weakest(), Some(better_than_low));
    assert_eq!(frontier.members(), &[high, mid, better_than_low]);
}

#[test]
fn top_k_frontier_updates_existing_member_without_duplicate_membership() {
    let mut frontier = TopKFrontier::new(NonZeroUsize::new(2).unwrap());
    let candidate = CandidateId::new();
    let other = CandidateId::new();

    frontier.observe(
        candidate,
        AssessmentId::new(),
        ScalarEvidence::new(0.3).unwrap(),
    );
    frontier.observe(
        other,
        AssessmentId::new(),
        ScalarEvidence::new(0.6).unwrap(),
    );
    let events = frontier.observe(
        candidate,
        AssessmentId::new(),
        ScalarEvidence::new(0.8).unwrap(),
    );

    assert!(matches!(
        events.as_slice(),
        [leaven_engine::PopulationEvent::Reweighted { candidate: changed, .. }]
            if *changed == candidate
    ));
    assert_eq!(frontier.members(), &[candidate, other]);
    assert_eq!(frontier.best_score(), Some(0.8));
    assert_eq!(frontier.member_score(candidate), Some(0.8));
}

#[test]
fn top_k_parent_selector_best_tracks_current_highest_score_without_advancing_cursor() {
    let mut frontier = TopKFrontier::new(NonZeroUsize::new(3).unwrap());
    let low = CandidateId::new();
    let mid = CandidateId::new();
    let high = CandidateId::new();
    let mut selector = TopKParentSelector::with_cursor(TopKParentSelectionPolicy::Best, 2);

    frontier.observe(low, AssessmentId::new(), ScalarEvidence::new(0.1).unwrap());
    frontier.observe(high, AssessmentId::new(), ScalarEvidence::new(0.9).unwrap());
    frontier.observe(mid, AssessmentId::new(), ScalarEvidence::new(0.4).unwrap());

    assert_eq!(selector.select(&frontier), Some(high));
    assert_eq!(selector.cursor(), 2);

    let new_high = CandidateId::new();
    frontier.observe(
        new_high,
        AssessmentId::new(),
        ScalarEvidence::new(1.0).unwrap(),
    );

    assert_eq!(selector.select(&frontier), Some(new_high));
    assert_eq!(selector.cursor(), 2);
}

#[test]
fn top_k_parent_selector_round_robin_cycles_over_current_frontier_order() {
    let mut frontier = TopKFrontier::new(NonZeroUsize::new(3).unwrap());
    let low = CandidateId::new();
    let mid = CandidateId::new();
    let high = CandidateId::new();
    let mut selector = TopKParentSelector::round_robin();

    frontier.observe(low, AssessmentId::new(), ScalarEvidence::new(0.1).unwrap());
    frontier.observe(high, AssessmentId::new(), ScalarEvidence::new(0.9).unwrap());
    frontier.observe(mid, AssessmentId::new(), ScalarEvidence::new(0.4).unwrap());

    assert_eq!(selector.select(&frontier), Some(high));
    assert_eq!(selector.cursor(), 1);
    assert_eq!(selector.select(&frontier), Some(mid));
    assert_eq!(selector.cursor(), 2);
    assert_eq!(selector.select(&frontier), Some(low));
    assert_eq!(selector.cursor(), 0);
    assert_eq!(selector.select(&frontier), Some(high));
}

#[test]
fn top_k_parent_selector_round_robin_empty_frontier_does_not_consume_cursor() {
    let frontier = TopKFrontier::new(NonZeroUsize::new(2).unwrap());
    let mut selector = TopKParentSelector::with_cursor(TopKParentSelectionPolicy::RoundRobin, 3);

    assert_eq!(selector.select(&frontier), None);
    assert_eq!(selector.cursor(), 3);
}
