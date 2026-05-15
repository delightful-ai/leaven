//! Public product builder for ordinary Leaven optimization runs.

mod builder;
mod compatibility;
mod error;
mod evaluator;
mod evidence;
mod result;
mod store;

pub use builder::{OptimizeBuilder, RunProblem, optimize};
pub use compatibility::{
    RunCompatibilityManifest, RuntimeFingerprint, RuntimeKind, ScoringEvaluatorIdentity,
};
pub use error::OptimizeError;
pub use evaluator::ScoringEvaluator;
pub use evidence::{
    RunCase, RunOutput, Score, ScoreCase, ScoreContext, ScoreError, ScoreMetadataView,
};
pub use result::{
    BestCandidate, OptimizationStopReason, Optimized, RunEventSummary, RunStorage,
    StandardRunSummary,
};
pub use store::{IntoOptimizeStore, OptimizeStore};

pub mod prelude {
    //! Common public-run imports.

    pub use crate::{
        BestCandidate, IntoOptimizeStore, OptimizationStopReason, OptimizeBuilder, OptimizeError,
        OptimizeStore, Optimized, RunCase, RunEventSummary, RunOutput, RunProblem, RunStorage,
        Score, ScoreCase, ScoreContext, ScoreError, ScoreMetadataView, StandardRunSummary,
        optimize,
    };
}
