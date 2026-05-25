//! Reusable population and frontier implementations for Leaven.

mod keep_best;
mod pareto_frontier;
mod skill_utility;
mod top_k_frontier;
mod tournament;

pub use keep_best::KeepBest;
pub use pareto_frontier::{ParetoFrontier, ParetoFrontierBuilder, PartitionFilter};
pub use skill_utility::{
    SkillPairedRolloutUtilityInput, SkillPairedRolloutUtilityInputError,
    SkillPairedRolloutUtilityUpdates, SkillPruningCandidate, SkillRetrievalCandidate,
    SkillSimilarityCandidate, SkillSimilarityCandidateError, SkillSimilarityRank,
    SkillStepTrajectoryOutcome, SkillStepTrajectoryOutcomeError, SkillTwoStageRetrievalConfig,
    SkillTwoStageRetrievalConfigError, SkillTwoStageRetrievalError, SkillTwoStageRetrievalPlan,
    SkillTwoStageRetriever, SkillUseStats, SkillUtilityCredit, SkillUtilityPrunePlan,
    SkillUtilityPruner, SkillUtilityPruningConfig, SkillUtilityPruningError,
    SkillUtilityPruningRank, SkillUtilityRank, SkillUtilityRanker, SkillUtilityRankingWeights,
    SkillUtilityRankingWeightsError, SkillUtilitySmoothing, SkillUtilitySmoothingError,
    SkillUtilityState, SkillUtilityTransfer, SkillUtilityUpdate,
};
pub use top_k_frontier::{TopKFrontier, TopKParentSelectionPolicy, TopKParentSelector};
pub use tournament::{BradleyTerryFit, TournamentPopulation};

pub mod prelude {
    pub use crate::{
        BradleyTerryFit, KeepBest, ParetoFrontier, ParetoFrontierBuilder, PartitionFilter,
        SkillPairedRolloutUtilityInput, SkillPairedRolloutUtilityInputError,
        SkillPairedRolloutUtilityUpdates, SkillPruningCandidate, SkillRetrievalCandidate,
        SkillSimilarityCandidate, SkillSimilarityCandidateError, SkillSimilarityRank,
        SkillStepTrajectoryOutcome, SkillStepTrajectoryOutcomeError, SkillTwoStageRetrievalConfig,
        SkillTwoStageRetrievalConfigError, SkillTwoStageRetrievalError, SkillTwoStageRetrievalPlan,
        SkillTwoStageRetriever, SkillUseStats, SkillUtilityCredit, SkillUtilityPrunePlan,
        SkillUtilityPruner, SkillUtilityPruningConfig, SkillUtilityPruningError,
        SkillUtilityPruningRank, SkillUtilityRank, SkillUtilityRanker, SkillUtilityRankingWeights,
        SkillUtilityRankingWeightsError, SkillUtilitySmoothing, SkillUtilitySmoothingError,
        SkillUtilityState, SkillUtilityTransfer, SkillUtilityUpdate, TopKFrontier,
        TopKParentSelectionPolicy, TopKParentSelector, TournamentPopulation,
    };
}
