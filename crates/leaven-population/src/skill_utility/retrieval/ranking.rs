use std::cmp::Ordering;

use leaven_artifact_skill::SkillName;
use leaven_kernel::FiniteF64;
use serde::{Deserialize, Serialize};

use crate::skill_utility::{SkillUseStats, SkillUtilityState, exploration_bonus};

use super::SkillRetrievalCandidate;

/// Non-negative weights for utility-aware skill ranking.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityRankingWeights {
    relevance: FiniteF64,
    utility: FiniteF64,
    exploration: FiniteF64,
}

impl SkillUtilityRankingWeights {
    /// Constructs ranking weights.
    ///
    /// # Errors
    ///
    /// Returns [`SkillUtilityRankingWeightsError`] when relevance or
    /// exploration weights are negative. Utility may be negative so callers can
    /// deliberately penalize high-utility skills in ablations.
    pub fn new(
        relevance: FiniteF64,
        utility: FiniteF64,
        exploration: FiniteF64,
    ) -> Result<Self, SkillUtilityRankingWeightsError> {
        if relevance.as_f64() < 0.0 {
            return Err(SkillUtilityRankingWeightsError::NegativeRelevanceWeight {
                value: relevance,
            });
        }
        if exploration.as_f64() < 0.0 {
            return Err(SkillUtilityRankingWeightsError::NegativeExplorationWeight {
                value: exploration,
            });
        }
        Ok(Self {
            relevance,
            utility,
            exploration,
        })
    }

    /// Weight applied to caller-provided relevance.
    #[must_use]
    pub const fn relevance(&self) -> FiniteF64 {
        self.relevance
    }

    /// Weight applied to stored utility.
    #[must_use]
    pub const fn utility(&self) -> FiniteF64 {
        self.utility
    }

    /// Weight applied to UCB-style exploration bonus.
    #[must_use]
    pub const fn exploration(&self) -> FiniteF64 {
        self.exploration
    }
}

impl Default for SkillUtilityRankingWeights {
    fn default() -> Self {
        Self::new(
            FiniteF64::new(1.0).expect("default relevance weight is finite"),
            FiniteF64::ZERO,
            FiniteF64::ZERO,
        )
        .expect("default weights are valid")
    }
}

/// Error returned when constructing [`SkillUtilityRankingWeights`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SkillUtilityRankingWeightsError {
    /// Relevance weight was negative.
    NegativeRelevanceWeight {
        /// Rejected value.
        value: FiniteF64,
    },
    /// Exploration weight was negative.
    NegativeExplorationWeight {
        /// Rejected value.
        value: FiniteF64,
    },
}

impl std::fmt::Display for SkillUtilityRankingWeightsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NegativeRelevanceWeight { value } => {
                write!(
                    f,
                    "skill relevance weight must be non-negative, got {}",
                    value.as_f64()
                )
            }
            Self::NegativeExplorationWeight { value } => {
                write!(
                    f,
                    "skill exploration weight must be non-negative, got {}",
                    value.as_f64()
                )
            }
        }
    }
}

impl std::error::Error for SkillUtilityRankingWeightsError {}

/// Ranked skill retrieval candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityRank {
    skill: SkillName,
    relevance: FiniteF64,
    utility: FiniteF64,
    exploration_bonus: FiniteF64,
    score: FiniteF64,
    stats: SkillUseStats,
}

impl SkillUtilityRank {
    /// Ranked skill.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Caller-provided relevance component.
    #[must_use]
    pub const fn relevance(&self) -> FiniteF64 {
        self.relevance
    }

    /// Stored utility component.
    #[must_use]
    pub const fn utility(&self) -> FiniteF64 {
        self.utility
    }

    /// UCB-style exploration bonus before weighting.
    #[must_use]
    pub const fn exploration_bonus(&self) -> FiniteF64 {
        self.exploration_bonus
    }

    /// Final weighted ranking score.
    #[must_use]
    pub const fn score(&self) -> FiniteF64 {
        self.score
    }

    /// Skill utility/use counters used for ranking.
    #[must_use]
    pub const fn stats(&self) -> SkillUseStats {
        self.stats
    }
}

/// Utility-aware ranker over caller-provided skill relevance scores.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityRanker {
    weights: SkillUtilityRankingWeights,
}

impl SkillUtilityRanker {
    /// Constructs a ranker with explicit weights.
    pub const fn new(weights: SkillUtilityRankingWeights) -> Self {
        Self { weights }
    }

    /// Ranking weights.
    #[must_use]
    pub const fn weights(&self) -> SkillUtilityRankingWeights {
        self.weights
    }

    /// Ranks candidates by weighted relevance, utility, and exploration bonus.
    pub fn rank(
        &self,
        state: &SkillUtilityState,
        candidates: impl IntoIterator<Item = SkillRetrievalCandidate>,
    ) -> Vec<SkillUtilityRank> {
        let total_retrievals = state.total_retrievals();
        self.rank_with_total_retrievals(state, total_retrievals, candidates)
    }

    pub(super) fn rank_with_total_retrievals(
        &self,
        state: &SkillUtilityState,
        total_retrievals: u64,
        candidates: impl IntoIterator<Item = SkillRetrievalCandidate>,
    ) -> Vec<SkillUtilityRank> {
        let mut ranked = candidates
            .into_iter()
            .map(|candidate| self.score_candidate(state, total_retrievals, candidate))
            .collect::<Vec<_>>();
        ranked.sort_by(rank_order);
        ranked
    }

    /// Returns the first `k` ranked candidates.
    pub fn top_k(
        &self,
        state: &SkillUtilityState,
        candidates: impl IntoIterator<Item = SkillRetrievalCandidate>,
        k: std::num::NonZeroUsize,
    ) -> Vec<SkillUtilityRank> {
        let mut ranked = self.rank(state, candidates);
        ranked.truncate(k.get());
        ranked
    }

    fn score_candidate(
        &self,
        state: &SkillUtilityState,
        total_retrievals: u64,
        candidate: SkillRetrievalCandidate,
    ) -> SkillUtilityRank {
        let utility = state.utility(&candidate.skill);
        let stats = state.stats(&candidate.skill);
        let exploration_bonus = exploration_bonus(total_retrievals, stats.retrievals);
        let score = weighted_score(
            self.weights,
            candidate.relevance,
            utility,
            exploration_bonus,
        );
        SkillUtilityRank {
            skill: candidate.skill,
            relevance: candidate.relevance,
            utility,
            exploration_bonus,
            score,
            stats,
        }
    }
}

impl Default for SkillUtilityRanker {
    fn default() -> Self {
        Self::new(SkillUtilityRankingWeights::default())
    }
}

fn weighted_score(
    weights: SkillUtilityRankingWeights,
    relevance: FiniteF64,
    utility: FiniteF64,
    exploration_bonus: FiniteF64,
) -> FiniteF64 {
    let score = (weights.relevance().as_f64() * relevance.as_f64())
        + (weights.utility().as_f64() * utility.as_f64())
        + (weights.exploration().as_f64() * exploration_bonus.as_f64());
    FiniteF64::new(score).expect("weighted finite score remains finite")
}

fn rank_order(left: &SkillUtilityRank, right: &SkillUtilityRank) -> Ordering {
    right
        .score()
        .partial_cmp(&left.score())
        .expect("skill ranking scores are finite")
        .then_with(|| left.skill().cmp(right.skill()))
}
