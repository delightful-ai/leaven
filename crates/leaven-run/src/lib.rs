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
pub use leaven_evidence::FeedbackAttachment;
pub use result::{OptimizationReport, OptimizeResult};
pub use store::{IntoOptimizeStore, OptimizeStore};

pub mod prelude {
    //! Common public-run imports.

    pub use crate::{
        FeedbackAttachment, IntoOptimizeStore, OptimizationReport, OptimizeBuilder, OptimizeError,
        OptimizeResult, OptimizeStore, RunOutput, RunProblem, Score, ScoreContext, ScoreError,
        optimize,
    };
}
