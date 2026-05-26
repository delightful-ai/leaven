//! Skill utility state for optimizer-owned retrieval bookkeeping.

use std::collections::BTreeMap;

mod paired_rollout;
mod pruning;
mod retrieval;

pub use paired_rollout::{
    SkillPairedRolloutUtilityInput, SkillPairedRolloutUtilityInputError,
    SkillPairedRolloutUtilityUpdates, SkillStepTrajectoryOutcome, SkillStepTrajectoryOutcomeError,
    SkillUtilityCredit,
};
pub use pruning::{
    SkillPruningCandidate, SkillUtilityPrunePlan, SkillUtilityPruner, SkillUtilityPruningConfig,
    SkillUtilityPruningError, SkillUtilityPruningRank,
};
pub use retrieval::{
    SkillRetrievalCandidate, SkillSimilarityCandidate, SkillSimilarityCandidateError,
    SkillSimilarityRank, SkillTwoStageRetrievalConfig, SkillTwoStageRetrievalConfigError,
    SkillTwoStageRetrievalError, SkillTwoStageRetrievalPlan, SkillTwoStageRetriever,
    SkillUtilityRank, SkillUtilityRanker, SkillUtilityRankingWeights,
    SkillUtilityRankingWeightsError,
};

use leaven_artifact_skill::SkillName;
use leaven_kernel::FiniteF64;
use serde::{Deserialize, Serialize};

/// Retrieval/use counters associated with one skill's utility state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkillUseStats {
    /// Number of times a skill was retrieved for consideration.
    pub retrievals: u64,
    /// Number of times runtime evidence says the skill was triggered/used.
    pub triggers: u64,
    /// Number of utility observations folded into the EMA.
    pub utility_updates: u64,
}

impl SkillUseStats {
    fn record_retrieval(&mut self) {
        self.retrievals = self.retrievals.saturating_add(1);
    }

    fn record_trigger(&mut self) {
        self.triggers = self.triggers.saturating_add(1);
    }

    fn record_utility_update(&mut self) {
        self.utility_updates = self.utility_updates.saturating_add(1);
    }
}

/// EMA smoothing weight for skill utility updates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct SkillUtilitySmoothing(f64);

impl SkillUtilitySmoothing {
    /// No movement toward the new observation.
    pub const ZERO: Self = Self(0.0);
    /// Replace utility with the new observation.
    pub const ONE: Self = Self(1.0);

    /// Constructs a smoothing weight in the inclusive range `[0.0, 1.0]`.
    ///
    /// # Errors
    ///
    /// Returns [`SkillUtilitySmoothingError`] when the value is not finite or
    /// falls outside the EMA weight range.
    pub fn new(value: f64) -> Result<Self, SkillUtilitySmoothingError> {
        if !value.is_finite() {
            return Err(SkillUtilitySmoothingError::NonFinite { value });
        }
        if !(0.0..=1.0).contains(&value) {
            return Err(SkillUtilitySmoothingError::OutOfRange { value });
        }
        Ok(Self(value))
    }

    /// Returns zero smoothing.
    #[must_use]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    /// Returns full replacement smoothing.
    #[must_use]
    pub const fn one() -> Self {
        Self::ONE
    }

    /// Returns the numeric smoothing weight.
    #[must_use]
    pub const fn as_f64(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for SkillUtilitySmoothing {
    type Error = SkillUtilitySmoothingError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SkillUtilitySmoothing> for f64 {
    fn from(value: SkillUtilitySmoothing) -> Self {
        value.0
    }
}

/// Error returned when constructing [`SkillUtilitySmoothing`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SkillUtilitySmoothingError {
    /// Value was NaN or infinite.
    NonFinite {
        /// Rejected value.
        value: f64,
    },
    /// Value was finite but outside `[0.0, 1.0]`.
    OutOfRange {
        /// Rejected value.
        value: f64,
    },
}

impl std::fmt::Display for SkillUtilitySmoothingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFinite { value } => {
                write!(f, "skill utility smoothing must be finite, got {value}")
            }
            Self::OutOfRange { value } => {
                write!(
                    f,
                    "skill utility smoothing must be between 0.0 and 1.0, got {value}"
                )
            }
        }
    }
}

impl std::error::Error for SkillUtilitySmoothingError {}

/// Result of applying one skill utility observation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityUpdate {
    skill: SkillName,
    utility_before: FiniteF64,
    utility_after: FiniteF64,
    stats_after: SkillUseStats,
}

impl SkillUtilityUpdate {
    /// Skill whose utility changed.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Utility before the observation.
    #[must_use]
    pub const fn utility_before(&self) -> FiniteF64 {
        self.utility_before
    }

    /// Utility after the observation.
    #[must_use]
    pub const fn utility_after(&self) -> FiniteF64 {
        self.utility_after
    }

    /// Counters after the observation.
    #[must_use]
    pub const fn stats_after(&self) -> SkillUseStats {
        self.stats_after
    }
}

/// Outcome of transferring utility state across a skill rename.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SkillUtilityTransfer {
    /// Source state moved to the target skill name.
    Transferred,
    /// No utility or use stats existed for the source skill.
    SourceMissing,
    /// The target already had utility or use stats, so no state moved.
    TargetExists,
}

/// Optimizer-owned skill utility and use counters keyed by validated skill name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityState {
    utilities: BTreeMap<SkillName, FiniteF64>,
    stats: BTreeMap<SkillName, SkillUseStats>,
}

impl SkillUtilityState {
    /// Returns all tracked utilities.
    #[must_use]
    pub const fn utilities(&self) -> &BTreeMap<SkillName, FiniteF64> {
        &self.utilities
    }

    /// Returns all tracked use counters.
    #[must_use]
    pub const fn stats_by_skill(&self) -> &BTreeMap<SkillName, SkillUseStats> {
        &self.stats
    }

    /// Returns a skill's utility. Unseen skills start at zero.
    #[must_use]
    pub fn utility(&self, skill: &SkillName) -> FiniteF64 {
        self.utilities
            .get(skill)
            .copied()
            .unwrap_or(FiniteF64::ZERO)
    }

    /// Returns a skill's counters. Unseen skills have zero counters.
    #[must_use]
    pub fn stats(&self, skill: &SkillName) -> SkillUseStats {
        self.stats.get(skill).copied().unwrap_or_default()
    }

    /// Records that a skill was retrieved for possible use.
    pub fn record_retrieval(&mut self, skill: SkillName) {
        self.stats.entry(skill).or_default().record_retrieval();
    }

    /// Records that runtime evidence says a skill triggered.
    pub fn record_trigger(&mut self, skill: SkillName) {
        self.stats.entry(skill).or_default().record_trigger();
    }

    /// Folds one signed utility delta into the skill's EMA utility.
    pub fn observe_delta(
        &mut self,
        skill: SkillName,
        delta: FiniteF64,
        smoothing: SkillUtilitySmoothing,
    ) -> SkillUtilityUpdate {
        let utility_before = self.utility(&skill);
        let utility_after = ema(utility_before, delta, smoothing);
        let stats_after = {
            let stats = self.stats.entry(skill.clone()).or_default();
            stats.record_utility_update();
            *stats
        };
        self.utilities.insert(skill.clone(), utility_after);
        SkillUtilityUpdate {
            skill,
            utility_before,
            utility_after,
            stats_after,
        }
    }

    /// Transfers utility and use counters across a skill rename.
    pub fn transfer_skill(&mut self, from: &SkillName, to: SkillName) -> SkillUtilityTransfer {
        if from == &to {
            return if self.has_skill_state(from) {
                SkillUtilityTransfer::Transferred
            } else {
                SkillUtilityTransfer::SourceMissing
            };
        }
        if !self.has_skill_state(from) {
            return SkillUtilityTransfer::SourceMissing;
        }
        if self.has_skill_state(&to) {
            return SkillUtilityTransfer::TargetExists;
        }

        if let Some(utility) = self.utilities.remove(from) {
            self.utilities.insert(to.clone(), utility);
        }
        if let Some(stats) = self.stats.remove(from) {
            self.stats.insert(to, stats);
        }
        SkillUtilityTransfer::Transferred
    }

    /// Removes all utility and use counters for a skill.
    pub fn remove_skill(&mut self, skill: &SkillName) -> bool {
        let removed_utility = self.utilities.remove(skill).is_some();
        let removed_stats = self.stats.remove(skill).is_some();
        removed_utility || removed_stats
    }

    fn has_skill_state(&self, skill: &SkillName) -> bool {
        self.utilities.contains_key(skill) || self.stats.contains_key(skill)
    }

    fn total_retrievals(&self) -> u64 {
        self.stats
            .values()
            .map(|stats| stats.retrievals)
            .fold(0_u64, u64::saturating_add)
    }
}

fn ema(before: FiniteF64, observation: FiniteF64, smoothing: SkillUtilitySmoothing) -> FiniteF64 {
    let weight = smoothing.as_f64();
    let next = before.as_f64() + (weight * (observation.as_f64() - before.as_f64()));
    FiniteF64::new(next).expect("EMA over finite utility values remains finite")
}

fn exploration_bonus(total_retrievals: u64, skill_retrievals: u64) -> FiniteF64 {
    let total = retrieval_count_scale(total_retrievals);
    let seen = retrieval_count_scale(skill_retrievals);
    FiniteF64::new((total.ln() / seen).sqrt()).expect("UCB exploration bonus is finite")
}

fn retrieval_count_scale(count: u64) -> f64 {
    let capped = u32::try_from(count.saturating_add(1)).unwrap_or(u32::MAX);
    f64::from(capped)
}
