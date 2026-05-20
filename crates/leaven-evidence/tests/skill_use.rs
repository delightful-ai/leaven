use leaven_artifact_skill::SkillName;
use leaven_evidence::{
    OutputRecord, SkillTrajectoryUseEvidence, SkillTrajectoryUseEvidenceError, SkillUseConfidence,
    SkillUseEvent, SkillUseEvidence, SkillUseKind, SkillUseSource,
};
use leaven_kernel::FiniteF64;

fn skill(name: &str) -> SkillName {
    SkillName::new(name).unwrap()
}

fn finite(value: f64) -> FiniteF64 {
    FiniteF64::new(value).unwrap()
}

#[test]
fn skill_trajectory_use_evidence_records_retrieved_skills_in_step_order() {
    let task_skill = skill("task-returns");
    let step_skill = skill("step-stripes");
    let retrieved_task = SkillUseEvent::new(
        task_skill,
        SkillUseKind::Retrieved,
        SkillUseSource::Router,
        SkillUseConfidence::Observed,
    )
    .with_step_index(0)
    .with_evidence(OutputRecord::inline("retrieved from task pool"));
    let retrieved_step = SkillUseEvent::new(
        step_skill.clone(),
        SkillUseKind::Retrieved,
        SkillUseSource::Router,
        SkillUseConfidence::Observed,
    )
    .with_step_index(3)
    .with_evidence(OutputRecord::inline("retrieved from step pool"));
    let triggered_step = SkillUseEvent::new(
        step_skill.clone(),
        SkillUseKind::Triggered,
        SkillUseSource::RuntimeTelemetry,
        SkillUseConfidence::Inferred,
    )
    .with_step_index(3);

    let evidence = SkillTrajectoryUseEvidence::new(
        "minishop-returns",
        "skill-rollout-0",
        finite(1.0),
        vec![
            retrieved_task.clone(),
            retrieved_step.clone(),
            triggered_step.clone(),
        ],
    )
    .unwrap();

    assert_eq!(evidence.task_id(), "minishop-returns");
    assert_eq!(evidence.trajectory_id(), "skill-rollout-0");
    assert_eq!(evidence.reward(), finite(1.0));
    assert_eq!(
        evidence
            .retrieved_skills()
            .iter()
            .map(|skill| skill.as_str())
            .collect::<Vec<_>>(),
        ["task-returns", "step-stripes"]
    );
    assert_eq!(
        evidence.skill_events(),
        &[retrieved_task, retrieved_step.clone(), triggered_step]
    );
    assert_eq!(
        evidence.events_for_skill(&step_skill),
        [&retrieved_step, evidence.skill_events().last().unwrap()]
    );
}

#[test]
fn skill_trajectory_use_evidence_refuses_blank_identities() {
    let event = SkillUseEvent::new(
        skill("step-stripes"),
        SkillUseKind::Retrieved,
        SkillUseSource::Router,
        SkillUseConfidence::Observed,
    );

    assert_eq!(
        SkillTrajectoryUseEvidence::new(" ", "trajectory", finite(0.0), vec![event.clone()])
            .unwrap_err(),
        SkillTrajectoryUseEvidenceError::EmptyTaskId,
    );
    assert_eq!(
        SkillTrajectoryUseEvidence::new("task", "\n", finite(0.0), vec![event]).unwrap_err(),
        SkillTrajectoryUseEvidenceError::EmptyTrajectoryId,
    );
}
