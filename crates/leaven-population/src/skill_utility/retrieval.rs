use std::cmp::Ordering;
use std::collections::BTreeMap;

use leaven_artifact_skill::{SkillName, SkillRoutePool, SkillRouteRegistry};
use leaven_kernel::FiniteF64;
use serde::{Deserialize, Serialize};

use super::{SkillUseStats, SkillUtilityState, exploration_bonus};

/// Candidate relevance emitted by a paper/router-specific retrieval layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillRetrievalCandidate {
    skill: SkillName,
    relevance: FiniteF64,
}

impl SkillRetrievalCandidate {
    /// Constructs a candidate with a caller-provided finite relevance score.
    pub fn new(skill: SkillName, relevance: FiniteF64) -> Self {
        Self { skill, relevance }
    }

    /// Candidate skill.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Caller-provided relevance score.
    #[must_use]
    pub const fn relevance(&self) -> FiniteF64 {
        self.relevance
    }
}

/// Similarity evidence for one routed skill candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillSimilarityCandidate {
    skill: SkillName,
    similarity: FiniteF64,
    relevance: FiniteF64,
}

impl SkillSimilarityCandidate {
    /// Constructs a similarity candidate.
    ///
    /// # Errors
    ///
    /// Returns [`SkillSimilarityCandidateError`] when normalized relevance is
    /// outside `D2Skill`'s `[0, 1]` scoring range.
    pub fn new(
        skill: SkillName,
        similarity: FiniteF64,
        relevance: FiniteF64,
    ) -> Result<Self, SkillSimilarityCandidateError> {
        if !(0.0..=1.0).contains(&relevance.as_f64()) {
            return Err(SkillSimilarityCandidateError::RelevanceOutOfRange { value: relevance });
        }
        Ok(Self {
            skill,
            similarity,
            relevance,
        })
    }

    /// Candidate skill.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Raw similarity score used for first-stage thresholding and top-m.
    #[must_use]
    pub const fn similarity(&self) -> FiniteF64 {
        self.similarity
    }

    /// Normalized relevance score used in utility-aware second-stage ranking.
    #[must_use]
    pub const fn relevance(&self) -> FiniteF64 {
        self.relevance
    }
}

/// Error returned when constructing [`SkillSimilarityCandidate`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SkillSimilarityCandidateError {
    /// Normalized relevance was outside `[0, 1]`.
    RelevanceOutOfRange {
        /// Rejected value.
        value: FiniteF64,
    },
}

impl std::fmt::Display for SkillSimilarityCandidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RelevanceOutOfRange { value } => {
                write!(
                    f,
                    "skill retrieval relevance must be in [0, 1], got {}",
                    value.as_f64()
                )
            }
        }
    }
}

impl std::error::Error for SkillSimilarityCandidateError {}

/// First-stage routed retrieval candidate retained after similarity filtering.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillSimilarityRank {
    skill: SkillName,
    similarity: FiniteF64,
    relevance: FiniteF64,
}

impl SkillSimilarityRank {
    fn from_candidate(candidate: &SkillSimilarityCandidate) -> Self {
        Self {
            skill: candidate.skill.clone(),
            similarity: candidate.similarity,
            relevance: candidate.relevance,
        }
    }

    /// Candidate skill.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Raw similarity score used for first-stage thresholding and top-m.
    #[must_use]
    pub const fn similarity(&self) -> FiniteF64 {
        self.similarity
    }

    /// Normalized relevance score used in utility-aware second-stage ranking.
    #[must_use]
    pub const fn relevance(&self) -> FiniteF64 {
        self.relevance
    }
}

/// D2Skill-style routed two-stage retrieval configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillTwoStageRetrievalConfig {
    pool: SkillRoutePool,
    similarity_threshold: FiniteF64,
    top_m: std::num::NonZeroUsize,
    top_k: std::num::NonZeroUsize,
    weights: SkillUtilityRankingWeights,
}

impl SkillTwoStageRetrievalConfig {
    /// Constructs two-stage retrieval configuration.
    ///
    /// # Errors
    ///
    /// Returns [`SkillTwoStageRetrievalConfigError`] when final top-k selection
    /// exceeds the first-stage top-m candidate set size.
    pub fn new(
        pool: SkillRoutePool,
        similarity_threshold: FiniteF64,
        top_m: std::num::NonZeroUsize,
        top_k: std::num::NonZeroUsize,
        weights: SkillUtilityRankingWeights,
    ) -> Result<Self, SkillTwoStageRetrievalConfigError> {
        if top_k.get() > top_m.get() {
            return Err(SkillTwoStageRetrievalConfigError::TopKExceedsTopM {
                top_m: top_m.get(),
                top_k: top_k.get(),
            });
        }
        Ok(Self {
            pool,
            similarity_threshold,
            top_m,
            top_k,
            weights,
        })
    }

    /// Active route pool.
    #[must_use]
    pub const fn pool(&self) -> &SkillRoutePool {
        &self.pool
    }

    /// Minimum raw similarity required for first-stage candidacy.
    #[must_use]
    pub const fn similarity_threshold(&self) -> FiniteF64 {
        self.similarity_threshold
    }

    /// First-stage candidate cap.
    #[must_use]
    pub const fn top_m(&self) -> std::num::NonZeroUsize {
        self.top_m
    }

    /// Final selected skill cap.
    #[must_use]
    pub const fn top_k(&self) -> std::num::NonZeroUsize {
        self.top_k
    }

    /// Utility ranking weights.
    #[must_use]
    pub const fn weights(&self) -> SkillUtilityRankingWeights {
        self.weights
    }
}

/// Error returned when constructing [`SkillTwoStageRetrievalConfig`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillTwoStageRetrievalConfigError {
    /// Final top-k selection exceeds first-stage top-m.
    TopKExceedsTopM {
        /// First-stage candidate cap.
        top_m: usize,
        /// Final selected skill cap.
        top_k: usize,
    },
}

impl std::fmt::Display for SkillTwoStageRetrievalConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TopKExceedsTopM { top_m, top_k } => {
                write!(
                    f,
                    "two-stage skill retrieval top-k ({top_k}) exceeds top-m ({top_m})"
                )
            }
        }
    }
}

impl std::error::Error for SkillTwoStageRetrievalConfigError {}

/// D2Skill-style routed two-stage skill retriever.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillTwoStageRetriever {
    config: SkillTwoStageRetrievalConfig,
}

impl SkillTwoStageRetriever {
    /// Constructs a retriever from explicit configuration.
    pub const fn new(config: SkillTwoStageRetrievalConfig) -> Self {
        Self { config }
    }

    /// Retrieval configuration.
    #[must_use]
    pub const fn config(&self) -> &SkillTwoStageRetrievalConfig {
        &self.config
    }

    /// Plans retrieval for one active route pool.
    ///
    /// Similarity candidates are supplied by the caller after embedding the
    /// query key and route keys. This function owns `D2Skill`'s generic
    /// threshold/top-m/top-k mechanics and utility/exploration scoring.
    pub fn retrieve(
        &self,
        registry: &SkillRouteRegistry,
        state: &SkillUtilityState,
        similarities: impl IntoIterator<Item = SkillSimilarityCandidate>,
    ) -> Result<SkillTwoStageRetrievalPlan, SkillTwoStageRetrievalError> {
        let mut similarities_by_skill = BTreeMap::new();
        for candidate in similarities {
            let skill = candidate.skill().clone();
            if registry.get(&skill).is_none() {
                return Err(SkillTwoStageRetrievalError::UnknownSimilaritySkill { skill });
            }
            if similarities_by_skill
                .insert(skill.clone(), candidate)
                .is_some()
            {
                return Err(SkillTwoStageRetrievalError::DuplicateSimilarity { skill });
            }
        }

        let active_entries = registry.by_pool(self.config.pool());
        let active_pool_retrievals = active_entries
            .iter()
            .map(|entry| state.stats(entry.skill()).retrievals)
            .fold(0_u64, u64::saturating_add);
        let mut first_stage = Vec::new();
        for entry in active_entries {
            let candidate = similarities_by_skill.get(entry.skill()).ok_or_else(|| {
                SkillTwoStageRetrievalError::MissingSimilarity {
                    skill: entry.skill().clone(),
                }
            })?;
            if candidate.similarity() >= self.config.similarity_threshold() {
                first_stage.push(SkillSimilarityRank::from_candidate(candidate));
            }
        }
        first_stage.sort_by(similarity_order);
        first_stage.truncate(self.config.top_m().get());

        let ranker = SkillUtilityRanker::new(self.config.weights());
        let ranked_candidates = first_stage
            .iter()
            .map(|candidate| {
                SkillRetrievalCandidate::new(candidate.skill().clone(), candidate.relevance())
            })
            .collect::<Vec<_>>();
        let mut selected =
            ranker.rank_with_total_retrievals(state, active_pool_retrievals, ranked_candidates);
        selected.truncate(self.config.top_k().get());

        Ok(SkillTwoStageRetrievalPlan {
            pool: self.config.pool().clone(),
            first_stage,
            selected,
        })
    }
}

/// Planned two-stage routed retrieval output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillTwoStageRetrievalPlan {
    pool: SkillRoutePool,
    first_stage: Vec<SkillSimilarityRank>,
    selected: Vec<SkillUtilityRank>,
}

impl SkillTwoStageRetrievalPlan {
    /// Active route pool.
    #[must_use]
    pub const fn pool(&self) -> &SkillRoutePool {
        &self.pool
    }

    /// First-stage candidates after thresholding and top-m similarity ordering.
    #[must_use]
    pub fn first_stage(&self) -> &[SkillSimilarityRank] {
        &self.first_stage
    }

    /// Final top-k selected skills after utility-aware ranking.
    #[must_use]
    pub fn selected(&self) -> &[SkillUtilityRank] {
        &self.selected
    }

    /// Records selected-skill retrieval counts into utility state.
    pub fn record_selected_retrievals(&self, state: &mut SkillUtilityState) {
        for rank in &self.selected {
            state.record_retrieval(rank.skill().clone());
        }
    }
}

/// Refusal reasons for two-stage routed retrieval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillTwoStageRetrievalError {
    /// A similarity score referenced a skill absent from the route registry.
    UnknownSimilaritySkill {
        /// Unknown skill.
        skill: SkillName,
    },
    /// A skill had more than one similarity score.
    DuplicateSimilarity {
        /// Repeated skill.
        skill: SkillName,
    },
    /// An active pool entry did not receive a similarity score.
    MissingSimilarity {
        /// Missing skill.
        skill: SkillName,
    },
}

impl std::fmt::Display for SkillTwoStageRetrievalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownSimilaritySkill { skill } => {
                write!(
                    f,
                    "similarity score references unknown routed skill `{skill}`"
                )
            }
            Self::DuplicateSimilarity { skill } => {
                write!(
                    f,
                    "similarity score for routed skill `{skill}` appeared more than once"
                )
            }
            Self::MissingSimilarity { skill } => {
                write!(
                    f,
                    "active routed skill `{skill}` is missing a similarity score"
                )
            }
        }
    }
}

impl std::error::Error for SkillTwoStageRetrievalError {}

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

fn similarity_order(left: &SkillSimilarityRank, right: &SkillSimilarityRank) -> Ordering {
    right
        .similarity()
        .partial_cmp(&left.similarity())
        .expect("skill similarity scores are finite")
        .then_with(|| left.skill().cmp(right.skill()))
}
