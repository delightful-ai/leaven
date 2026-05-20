//! Rollout-group evidence for baseline-versus-treatment comparisons.

use std::num::NonZeroUsize;

use leaven_core::Evidence;
use leaven_kernel::FiniteF64;
use serde::{Deserialize, Serialize};

/// Mean reward observed for one rollout group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RolloutGroupOutcome {
    trajectory_count: NonZeroUsize,
    mean_reward: FiniteF64,
}

impl RolloutGroupOutcome {
    /// Build a rollout group outcome from a non-empty trajectory group.
    #[must_use]
    pub const fn new(trajectory_count: NonZeroUsize, mean_reward: FiniteF64) -> Self {
        Self {
            trajectory_count,
            mean_reward,
        }
    }

    /// Number of trajectories represented by this group.
    #[must_use]
    pub const fn trajectory_count(&self) -> NonZeroUsize {
        self.trajectory_count
    }

    /// Mean reward or success rate for the group.
    #[must_use]
    pub const fn mean_reward(&self) -> FiniteF64 {
        self.mean_reward
    }
}

/// Evidence comparing one baseline rollout group against one treatment group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PairedRolloutEvidence {
    task_id: String,
    baseline: RolloutGroupOutcome,
    treatment: RolloutGroupOutcome,
}

impl PairedRolloutEvidence {
    /// Build paired rollout evidence for one benchmark task.
    ///
    /// # Errors
    ///
    /// Returns [`PairedRolloutEvidenceError`] when the upstream task identity
    /// is blank after trimming.
    pub fn new(
        task_id: impl Into<String>,
        baseline: RolloutGroupOutcome,
        treatment: RolloutGroupOutcome,
    ) -> Result<Self, PairedRolloutEvidenceError> {
        let task_id = task_id.into();
        if task_id.trim().is_empty() {
            return Err(PairedRolloutEvidenceError::EmptyTaskId);
        }

        Ok(Self {
            task_id,
            baseline,
            treatment,
        })
    }

    /// Upstream task id for the paired rollout.
    #[must_use]
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Baseline group outcome.
    #[must_use]
    pub const fn baseline(&self) -> &RolloutGroupOutcome {
        &self.baseline
    }

    /// Treatment group outcome.
    #[must_use]
    pub const fn treatment(&self) -> &RolloutGroupOutcome {
        &self.treatment
    }

    /// Treatment mean reward minus baseline mean reward.
    #[must_use]
    pub fn treatment_minus_baseline(&self) -> FiniteF64 {
        FiniteF64::new(self.treatment.mean_reward().as_f64() - self.baseline.mean_reward().as_f64())
            .expect("difference between finite rewards remains finite")
    }
}

impl Evidence for PairedRolloutEvidence {}

/// Refusal reasons for paired rollout evidence construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PairedRolloutEvidenceError {
    /// The upstream task id was blank.
    #[error("paired rollout evidence requires a non-empty task id")]
    EmptyTaskId,
}
