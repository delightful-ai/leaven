//! Engine shape (declaration only).
//!
//! The runnable engine — owning the run graph, evaluator registry,
//! budget ledger, evaluation cache, callbacks, stoppers, trust policy,
//! and run store — lives in `leaven-engine`. This module declares the
//! `Optimizer` trait surface that the engine drives, and a
//! placeholder `OptimizerError`.
//!
//! The full async surface comes with the engine; this is the cold
//! marker so optimizer authors can already see "what they implement".

use crate::graph::events::StepStatus;
use crate::ids::CandidateId;

#[derive(Debug, thiserror::Error)]
#[error("optimizer error: {message}")]
pub struct OptimizerError {
    pub message: String,
}

impl OptimizerError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// What the engine drives. Full async signature lands with the
/// engine; this trait declares the synchronous skeleton so optimizer
/// authors can see the surface.
pub trait Optimizer: Send + Sync {
    fn name(&self) -> &str;

    /// Optional: choose a final candidate at the end of a run.
    fn best_candidate(&self) -> Option<CandidateId> {
        None
    }

    /// Step status hint (for documentation; the real signature takes
    /// a `RunContext` and returns `Result`).
    fn last_status(&self) -> StepStatus {
        StepStatus::Continue
    }
}
