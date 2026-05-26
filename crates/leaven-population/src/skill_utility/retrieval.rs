use std::cmp::Ordering;
use std::collections::BTreeMap;

use leaven_artifact_skill::{SkillName, SkillRoutePool, SkillRouteRegistry};
use leaven_kernel::FiniteF64;
use serde::{Deserialize, Serialize};

mod ranking;

pub use ranking::{
    SkillUtilityRank, SkillUtilityRanker, SkillUtilityRankingWeights,
    SkillUtilityRankingWeightsError,
};

use super::SkillUtilityState;

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

fn similarity_order(left: &SkillSimilarityRank, right: &SkillSimilarityRank) -> Ordering {
    right
        .similarity()
        .partial_cmp(&left.similarity())
        .expect("skill similarity scores are finite")
        .then_with(|| left.skill().cmp(right.skill()))
}
