use std::cmp::Ordering;
use std::collections::BTreeSet;

use leaven_artifact_skill::SkillName;
use leaven_kernel::FiniteF64;
use serde::{Deserialize, Serialize};

use super::{SkillUseStats, SkillUtilityState, exploration_bonus};

/// One skill in a capacity-bounded pruning pool.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkillPruningCandidate {
    skill: SkillName,
    created_step: u64,
}

impl SkillPruningCandidate {
    /// Build a pruning candidate with the step at which the skill entered the pool.
    #[must_use]
    pub const fn new(skill: SkillName, created_step: u64) -> Self {
        Self {
            skill,
            created_step,
        }
    }

    /// Candidate skill.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Training step at which the skill entered the pool.
    #[must_use]
    pub const fn created_step(&self) -> u64 {
        self.created_step
    }
}

/// Configuration for utility-guided skill pruning.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityPruningConfig {
    capacity: usize,
    current_step: u64,
    protected_window: u64,
    exploration_weight: FiniteF64,
}

impl SkillUtilityPruningConfig {
    /// Build pruning configuration.
    ///
    /// # Errors
    ///
    /// Returns [`SkillUtilityPruningError`] when the exploration weight is negative.
    pub fn new(
        capacity: usize,
        current_step: u64,
        protected_window: u64,
        exploration_weight: FiniteF64,
    ) -> Result<Self, SkillUtilityPruningError> {
        if exploration_weight.as_f64() < 0.0 {
            return Err(SkillUtilityPruningError::NegativeExplorationWeight {
                value: exploration_weight,
            });
        }
        Ok(Self {
            capacity,
            current_step,
            protected_window,
            exploration_weight,
        })
    }

    /// Maximum desired pool size.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Current training step.
    #[must_use]
    pub const fn current_step(&self) -> u64 {
        self.current_step
    }

    /// Age window during which new skills are excluded from eviction.
    #[must_use]
    pub const fn protected_window(&self) -> u64 {
        self.protected_window
    }

    /// Weight applied to the UCB-style exploration bonus.
    #[must_use]
    pub const fn exploration_weight(&self) -> FiniteF64 {
        self.exploration_weight
    }
}

/// Utility-guided skill pool pruner.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityPruner {
    config: SkillUtilityPruningConfig,
}

impl SkillUtilityPruner {
    /// Build a pruner from explicit configuration.
    #[must_use]
    pub const fn new(config: SkillUtilityPruningConfig) -> Self {
        Self { config }
    }

    /// Pruning configuration.
    #[must_use]
    pub const fn config(&self) -> SkillUtilityPruningConfig {
        self.config
    }

    /// Plans evictions for one active skill pool.
    ///
    /// The eviction score is `utility + eta * exploration_bonus`; unprotected
    /// candidates with the lowest scores are evicted until capacity is met, or
    /// until no evictable candidates remain.
    pub fn plan(
        &self,
        state: &SkillUtilityState,
        candidates: impl IntoIterator<Item = SkillPruningCandidate>,
    ) -> Result<SkillUtilityPrunePlan, SkillUtilityPruningError> {
        let candidates = candidates.into_iter().collect::<Vec<_>>();
        ensure_unique_pruning_candidates(&candidates)?;
        let total_retrievals = active_pool_retrievals(state, &candidates);
        let mut ranked = candidates
            .into_iter()
            .map(|candidate| self.rank_candidate(state, total_retrievals, candidate))
            .collect::<Vec<_>>();

        let overflow = ranked.len().saturating_sub(self.config.capacity());
        let evictable = ranked.iter().filter(|rank| !rank.is_protected()).count();
        let evict_count = overflow.min(evictable);

        let mut eviction_order = ranked
            .iter()
            .enumerate()
            .filter(|(_, rank)| !rank.is_protected())
            .collect::<Vec<_>>();
        eviction_order.sort_by(|(_, left), (_, right)| pruning_eviction_order(left, right));
        let evicted_skills = eviction_order
            .into_iter()
            .take(evict_count)
            .map(|(_, rank)| rank.skill().clone())
            .collect::<BTreeSet<_>>();

        ranked.sort_by(pruning_keep_order);
        let (kept, evicted): (Vec<_>, Vec<_>) = ranked
            .into_iter()
            .partition(|rank| !evicted_skills.contains(rank.skill()));
        let mut evicted = evicted;
        evicted.sort_by(pruning_eviction_order);

        Ok(SkillUtilityPrunePlan {
            capacity: self.config.capacity(),
            kept,
            evicted,
        })
    }

    fn rank_candidate(
        &self,
        state: &SkillUtilityState,
        total_retrievals: u64,
        candidate: SkillPruningCandidate,
    ) -> SkillUtilityPruningRank {
        let utility = state.utility(&candidate.skill);
        let stats = state.stats(&candidate.skill);
        let exploration_bonus = exploration_bonus(total_retrievals, stats.retrievals);
        let eviction_score = FiniteF64::new(
            utility.as_f64()
                + (self.config.exploration_weight().as_f64() * exploration_bonus.as_f64()),
        )
        .expect("utility pruning score remains finite");
        let is_protected = self
            .config
            .current_step()
            .saturating_sub(candidate.created_step())
            < self.config.protected_window();

        SkillUtilityPruningRank {
            skill: candidate.skill,
            created_step: candidate.created_step,
            utility,
            stats,
            exploration_bonus,
            eviction_score,
            is_protected,
        }
    }
}

/// Ranked pruning candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityPruningRank {
    skill: SkillName,
    created_step: u64,
    utility: FiniteF64,
    stats: SkillUseStats,
    exploration_bonus: FiniteF64,
    eviction_score: FiniteF64,
    is_protected: bool,
}

impl SkillUtilityPruningRank {
    /// Ranked skill.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Training step at which the skill entered the pool.
    #[must_use]
    pub const fn created_step(&self) -> u64 {
        self.created_step
    }

    /// Stored utility used for pruning.
    #[must_use]
    pub const fn utility(&self) -> FiniteF64 {
        self.utility
    }

    /// Skill utility/use counters used for pruning.
    #[must_use]
    pub const fn stats(&self) -> SkillUseStats {
        self.stats
    }

    /// UCB-style exploration bonus before weighting.
    #[must_use]
    pub const fn exploration_bonus(&self) -> FiniteF64 {
        self.exploration_bonus
    }

    /// Final eviction score. Lower unprotected scores are evicted first.
    #[must_use]
    pub const fn eviction_score(&self) -> FiniteF64 {
        self.eviction_score
    }

    /// Whether this skill is inside the protected creation window.
    #[must_use]
    pub const fn is_protected(&self) -> bool {
        self.is_protected
    }
}

/// Planned skill pruning result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityPrunePlan {
    capacity: usize,
    kept: Vec<SkillUtilityPruningRank>,
    evicted: Vec<SkillUtilityPruningRank>,
}

impl SkillUtilityPrunePlan {
    /// Skills retained in deterministic retention order.
    #[must_use]
    pub fn kept(&self) -> &[SkillUtilityPruningRank] {
        &self.kept
    }

    /// Skills evicted in deterministic eviction order.
    #[must_use]
    pub fn evicted(&self) -> &[SkillUtilityPruningRank] {
        &self.evicted
    }

    /// Whether the returned retained set satisfies the configured capacity.
    #[must_use]
    pub fn capacity_satisfied(&self) -> bool {
        self.kept.len() <= self.capacity
    }
}

/// Refusal reasons for skill utility pruning.
#[derive(Clone, Debug, PartialEq)]
pub enum SkillUtilityPruningError {
    /// Exploration weight was negative.
    NegativeExplorationWeight {
        /// Rejected value.
        value: FiniteF64,
    },
    /// A skill appeared more than once in the pruning pool.
    DuplicateCandidate {
        /// Repeated skill identity.
        skill: SkillName,
    },
}

impl std::fmt::Display for SkillUtilityPruningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NegativeExplorationWeight { value } => {
                write!(
                    f,
                    "skill pruning exploration weight must be non-negative, got {}",
                    value.as_f64()
                )
            }
            Self::DuplicateCandidate { skill } => {
                write!(
                    f,
                    "skill pruning candidate `{skill}` appeared more than once"
                )
            }
        }
    }
}

impl std::error::Error for SkillUtilityPruningError {}

fn ensure_unique_pruning_candidates(
    candidates: &[SkillPruningCandidate],
) -> Result<(), SkillUtilityPruningError> {
    let mut seen = BTreeSet::new();
    for candidate in candidates {
        if !seen.insert(candidate.skill().clone()) {
            return Err(SkillUtilityPruningError::DuplicateCandidate {
                skill: candidate.skill().clone(),
            });
        }
    }
    Ok(())
}

fn active_pool_retrievals(state: &SkillUtilityState, candidates: &[SkillPruningCandidate]) -> u64 {
    candidates
        .iter()
        .map(|candidate| state.stats(candidate.skill()).retrievals)
        .fold(0_u64, u64::saturating_add)
}

fn pruning_eviction_order(
    left: &SkillUtilityPruningRank,
    right: &SkillUtilityPruningRank,
) -> Ordering {
    left.eviction_score()
        .partial_cmp(&right.eviction_score())
        .expect("skill pruning scores are finite")
        .then_with(|| left.skill().cmp(right.skill()))
}

fn pruning_keep_order(left: &SkillUtilityPruningRank, right: &SkillUtilityPruningRank) -> Ordering {
    right
        .is_protected()
        .cmp(&left.is_protected())
        .then_with(|| {
            right
                .eviction_score()
                .partial_cmp(&left.eviction_score())
                .expect("skill pruning scores are finite")
        })
        .then_with(|| left.skill().cmp(right.skill()))
}
