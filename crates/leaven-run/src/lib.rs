//! Public product builder for ordinary Leaven optimization runs.

mod builder;
mod error;
mod evaluator;
mod evidence;
mod result;
mod store;

pub use builder::{OptimizeBuilder, RunProblem, optimize};
pub use error::OptimizeError;
pub use evaluator::ScoringEvaluator;
pub use evidence::{RunOutput, Score, ScoreContext, ScoreError};
pub use result::{
    BestCandidate, OptimizationStopReason, Optimized, RunEventSummary, RunStorage,
    StandardRunSummary,
};
pub use store::{IntoOptimizeStore, OptimizeStore};

pub mod prelude {
    //! Common public-run imports.

    pub use crate::{
        BestCandidate, IntoOptimizeStore, OptimizationStopReason, OptimizeBuilder, OptimizeError,
        OptimizeStore, Optimized, RunEventSummary, RunOutput, RunProblem, RunStorage, Score,
        ScoreContext, ScoreError, StandardRunSummary, optimize,
    };
}
