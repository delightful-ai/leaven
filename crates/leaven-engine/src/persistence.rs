//! Run persistence capability.

use leaven_core::OptimizationProblem;

use crate::RunGraph;

/// Capability for durable run graph checkpoints.
///
/// Persistence adapters own the world-facing details. The engine only depends
/// on this capability and its structured refusal modes.
pub trait RunPersistence<P: OptimizationProblem>: Send + Sync {
    /// Persist a checkpoint of the current run graph.
    fn checkpoint(&self, graph: &RunGraph<P>) -> Result<(), RunPersistenceError>;
}

/// Failures a run persistence adapter can report.
#[derive(Debug, thiserror::Error)]
pub enum RunPersistenceError {
    /// The backend cannot currently accept checkpoint requests.
    #[error("run persistence backend is unavailable: {reason}")]
    Unavailable { reason: String },
    /// The backend received the checkpoint request but refused to commit it.
    #[error("run persistence checkpoint was refused: {reason}")]
    CheckpointFailed {
        reason: String,
        retryable: Option<bool>,
    },
}
