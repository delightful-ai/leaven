//! Public run-builder refusal modes.

use leaven_engine::{OptimizerError, RunContextError};

/// Errors returned by the high-level optimization builder.
#[derive(Debug, thiserror::Error)]
pub enum OptimizeError {
    /// The caller did not choose a budget.
    #[error("explicit budget is required; call `.budget(...)` or `.budget(Budget::unlimited())`")]
    MissingBudget,
    /// The caller did not install a scoring function.
    #[error("score function is required")]
    MissingScore,
    /// Validation or test cases were supplied without train cases.
    #[error("validation/test cases require at least one train case")]
    HeldOutWithoutTrain,
    /// The seed artifact could not be inserted into graph truth.
    #[error("seed insertion failed")]
    SeedInsertion {
        #[source]
        source: RunContextError,
    },
    /// The engine or optimizer refused the run.
    #[error(transparent)]
    Optimizer(#[from] OptimizerError),
}
