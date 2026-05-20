use leaven_evidence::{PairedRolloutEvidence, PairedRolloutEvidenceError, RolloutGroupOutcome};
use leaven_kernel::FiniteF64;

fn finite(value: f64) -> FiniteF64 {
    FiniteF64::new(value).unwrap()
}

#[test]
fn paired_rollout_evidence_records_group_rewards_and_gap() {
    let baseline = RolloutGroupOutcome::new(2.try_into().unwrap(), finite(0.25));
    let skill_group = RolloutGroupOutcome::new(2.try_into().unwrap(), finite(0.75));

    let evidence = PairedRolloutEvidence::new(
        "alfworld-put-cool-mug",
        baseline.clone(),
        skill_group.clone(),
    )
    .unwrap();

    assert_eq!(evidence.task_id(), "alfworld-put-cool-mug");
    assert_eq!(evidence.baseline(), &baseline);
    assert_eq!(evidence.treatment(), &skill_group);
    assert_eq!(evidence.treatment_minus_baseline(), finite(0.5));
}

#[test]
fn paired_rollout_evidence_refuses_blank_task_identity() {
    let baseline = RolloutGroupOutcome::new(1.try_into().unwrap(), finite(0.0));
    let treatment = RolloutGroupOutcome::new(1.try_into().unwrap(), finite(1.0));

    assert_eq!(
        PairedRolloutEvidence::new("   ", baseline, treatment).unwrap_err(),
        PairedRolloutEvidenceError::EmptyTaskId,
    );
}
