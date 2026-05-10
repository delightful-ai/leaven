//! Public product builder for ordinary Leaven optimization runs.

mod builder;
mod evaluator;
mod evidence;
mod result;

pub use builder::{OptimizeBuilder, RunProblem, optimize};
pub use evaluator::ScoringEvaluator;
pub use evidence::{RunOutput, Score, ScoreContext};
pub use result::{OptimizationReport, OptimizeResult};

pub mod prelude {
    //! Common public-run imports.

    pub use crate::{
        OptimizationReport, OptimizeBuilder, OptimizeResult, RunOutput, RunProblem, Score,
        ScoreContext, optimize,
    };
}
