use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence};
use leaven_kernel::{AssessmentId, CandidateId, CaseId};
use leaven_population::ParetoFrontier;

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
