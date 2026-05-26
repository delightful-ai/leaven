//! D2Skill-style paired-rollout utility credit inputs.

use std::collections::BTreeSet;

use leaven_artifact_skill::SkillName;
use leaven_evidence::{PairedRolloutEvidence, SkillTrajectoryUseEvidence};
use leaven_kernel::FiniteF64;
use serde::{Deserialize, Serialize};

use super::{SkillUtilitySmoothing, SkillUtilityState, SkillUtilityUpdate};

/// One finite utility credit assigned to a validated skill identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityCredit {
    skill: SkillName,
    credit: FiniteF64,
}

impl SkillUtilityCredit {
    /// Build a utility credit for one skill.
    #[must_use]
    pub const fn new(skill: SkillName, credit: FiniteF64) -> Self {
        Self { skill, credit }
    }

    /// Credited skill.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Signed utility credit.
    #[must_use]
    pub const fn credit(&self) -> FiniteF64 {
        self.credit
    }
}

/// One skill-injected trajectory's reward and retrieved step skills.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillStepTrajectoryOutcome {
    trajectory_id: String,
    reward: FiniteF64,
    retrieved_step_skills: Vec<SkillName>,
}

impl SkillStepTrajectoryOutcome {
    /// Build a step-skill trajectory outcome.
    ///
    /// # Errors
    ///
    /// Returns [`SkillStepTrajectoryOutcomeError`] when the trajectory
    /// identity is blank after trimming.
    pub fn new(
        trajectory_id: impl Into<String>,
        reward: FiniteF64,
        retrieved_step_skills: Vec<SkillName>,
    ) -> Result<Self, SkillStepTrajectoryOutcomeError> {
        let trajectory_id = trajectory_id.into();
        if trajectory_id.trim().is_empty() {
            return Err(SkillStepTrajectoryOutcomeError::EmptyTrajectoryId);
        }

        Ok(Self {
            trajectory_id,
            reward,
            retrieved_step_skills,
        })
    }

    /// Runner-provided trajectory identity.
    #[must_use]
    pub fn trajectory_id(&self) -> &str {
        &self.trajectory_id
    }

    /// Terminal reward or success indicator for this skill-injected trajectory.
    #[must_use]
    pub const fn reward(&self) -> FiniteF64 {
        self.reward
    }

    /// Step skills retrieved by this trajectory, in runner observation order.
    #[must_use]
    pub fn retrieved_step_skills(&self) -> &[SkillName] {
        &self.retrieved_step_skills
    }

    /// Build a step-skill trajectory outcome from generic skill-use evidence.
    #[must_use]
    pub fn from_skill_use_evidence(evidence: &SkillTrajectoryUseEvidence) -> Self {
        Self {
            trajectory_id: evidence.trajectory_id().to_owned(),
            reward: evidence.reward(),
            retrieved_step_skills: evidence.retrieved_skills().into_iter().cloned().collect(),
        }
    }
}

/// Refusal reasons for step-trajectory outcome construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillStepTrajectoryOutcomeError {
    /// The trajectory identity was blank.
    EmptyTrajectoryId,
}

impl std::fmt::Display for SkillStepTrajectoryOutcomeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTrajectoryId => {
                f.write_str("skill step trajectory outcome requires a non-empty trajectory id")
            }
        }
    }
}

impl std::error::Error for SkillStepTrajectoryOutcomeError {}

/// D2Skill-style utility input derived from paired rollout evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillPairedRolloutUtilityInput {
    paired_rollout: PairedRolloutEvidence,
    task_skills: Vec<SkillName>,
    step_skill_credits: Vec<SkillUtilityCredit>,
}

impl SkillPairedRolloutUtilityInput {
    /// Build utility-update input from a paired rollout and retrieved skills.
    ///
    /// # Errors
    ///
    /// Returns [`SkillPairedRolloutUtilityInputError`] when a task skill is
    /// repeated, which would make the group-level task delta ambiguous.
    pub fn new(
        paired_rollout: PairedRolloutEvidence,
        task_skills: Vec<SkillName>,
        step_skill_credits: Vec<SkillUtilityCredit>,
    ) -> Result<Self, SkillPairedRolloutUtilityInputError> {
        let mut seen = BTreeSet::new();
        for skill in &task_skills {
            if !seen.insert(skill.clone()) {
                return Err(SkillPairedRolloutUtilityInputError::DuplicateTaskSkill {
                    skill: skill.clone(),
                });
            }
        }

        Ok(Self {
            paired_rollout,
            task_skills,
            step_skill_credits,
        })
    }

    /// Build utility-update input from skill-injected trajectory outcomes.
    ///
    /// Each retrieved step skill receives the `D2Skill` trajectory-level credit
    /// `trajectory_reward - baseline_group_mean`.
    ///
    /// # Errors
    ///
    /// Returns [`SkillPairedRolloutUtilityInputError`] when a task skill is
    /// repeated.
    pub fn from_step_trajectories(
        paired_rollout: PairedRolloutEvidence,
        task_skills: Vec<SkillName>,
        step_trajectories: Vec<SkillStepTrajectoryOutcome>,
    ) -> Result<Self, SkillPairedRolloutUtilityInputError> {
        let baseline_mean = paired_rollout.baseline().mean_reward();
        let step_skill_credits = step_trajectories
            .into_iter()
            .flat_map(|trajectory| {
                let credit = FiniteF64::new(trajectory.reward.as_f64() - baseline_mean.as_f64())
                    .expect("difference between finite rewards remains finite");
                trajectory
                    .retrieved_step_skills
                    .into_iter()
                    .map(move |skill| SkillUtilityCredit::new(skill, credit))
            })
            .collect();

        Self::new(paired_rollout, task_skills, step_skill_credits)
    }

    /// Build utility-update input from generic skill-use trajectory evidence.
    ///
    /// Retrieved skills in each trajectory receive the `D2Skill`
    /// trajectory-level credit `trajectory_reward - baseline_group_mean`.
    ///
    /// # Errors
    ///
    /// Returns [`SkillPairedRolloutUtilityInputError`] when a task skill is
    /// repeated.
    pub fn from_skill_use_trajectories(
        paired_rollout: PairedRolloutEvidence,
        task_skills: Vec<SkillName>,
        step_trajectories: &[SkillTrajectoryUseEvidence],
    ) -> Result<Self, SkillPairedRolloutUtilityInputError> {
        let outcomes = step_trajectories
            .iter()
            .map(SkillStepTrajectoryOutcome::from_skill_use_evidence)
            .collect();
        Self::from_step_trajectories(paired_rollout, task_skills, outcomes)
    }

    /// Paired rollout evidence supplying the task-level reward gap.
    #[must_use]
    pub const fn paired_rollout(&self) -> &PairedRolloutEvidence {
        &self.paired_rollout
    }

    /// Treatment-minus-baseline task-level utility signal.
    #[must_use]
    pub fn task_delta(&self) -> FiniteF64 {
        self.paired_rollout.treatment_minus_baseline()
    }

    /// Retrieved task skills with the shared task-level utility signal.
    #[must_use]
    pub fn task_skill_credits(&self) -> Vec<SkillUtilityCredit> {
        let task_delta = self.task_delta();
        self.task_skills
            .iter()
            .cloned()
            .map(|skill| SkillUtilityCredit::new(skill, task_delta))
            .collect()
    }

    /// Step-skill utility credits supplied by trajectory-level credit assignment.
    #[must_use]
    pub fn step_skill_credits(&self) -> &[SkillUtilityCredit] {
        &self.step_skill_credits
    }

    /// Task and step utility credits in application order.
    #[must_use]
    pub fn all_utility_credits(&self) -> Vec<SkillUtilityCredit> {
        let mut credits = self.task_skill_credits();
        credits.extend(self.step_skill_credits.iter().cloned());
        credits
    }

    /// Apply task and step utility credits to optimizer-owned utility state.
    pub fn apply_to_state(
        &self,
        state: &mut SkillUtilityState,
        task_smoothing: SkillUtilitySmoothing,
        step_smoothing: SkillUtilitySmoothing,
    ) -> SkillPairedRolloutUtilityUpdates {
        let task_updates = self
            .task_skill_credits()
            .into_iter()
            .map(|credit| state.observe_delta(credit.skill, credit.credit, task_smoothing))
            .collect();
        let step_updates = self
            .step_skill_credits
            .iter()
            .cloned()
            .map(|credit| state.observe_delta(credit.skill, credit.credit, step_smoothing))
            .collect();

        SkillPairedRolloutUtilityUpdates {
            task_updates,
            step_updates,
        }
    }
}

/// Utility updates produced by applying paired rollout skill credits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillPairedRolloutUtilityUpdates {
    task_updates: Vec<SkillUtilityUpdate>,
    step_updates: Vec<SkillUtilityUpdate>,
}

impl SkillPairedRolloutUtilityUpdates {
    /// Updates applied to retrieved task skills.
    #[must_use]
    pub fn task_updates(&self) -> &[SkillUtilityUpdate] {
        &self.task_updates
    }

    /// Updates applied to retrieved step skills.
    #[must_use]
    pub fn step_updates(&self) -> &[SkillUtilityUpdate] {
        &self.step_updates
    }
}

/// Refusal reasons for paired rollout utility inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillPairedRolloutUtilityInputError {
    /// A task skill appeared more than once.
    DuplicateTaskSkill {
        /// Repeated skill identity.
        skill: SkillName,
    },
}

impl std::fmt::Display for SkillPairedRolloutUtilityInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateTaskSkill { skill } => {
                write!(
                    f,
                    "paired rollout task skill `{skill}` was credited more than once"
                )
            }
        }
    }
}

impl std::error::Error for SkillPairedRolloutUtilityInputError {}
