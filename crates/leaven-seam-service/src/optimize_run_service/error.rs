use leaven_seam_runtime::SeamServiceError;

/// Failure raised while executing a `leaven/optimize.run` dispatch.
///
/// The host lowers the locked optimize-run request into the `leaven-run`
/// builder, drives the real GEPA loop, and projects the durable result back
/// into the locked result document. Every variant names what is supported so a
/// client never gets a silent downgrade.
#[derive(Debug, thiserror::Error)]
pub enum OptimizeRunHostError {
    /// The request asked for a config value the V1 host does not execute. The
    /// message always names the supported value.
    #[error("optimize.run is unsupported: {0}")]
    Unsupported(String),

    /// No stage worker argv is configured, so the host cannot dispatch
    /// runner/scorer stages. This mirrors the method-unavailable style.
    #[error("optimize.run requires a configured stage worker: {0}")]
    WorkerUnavailable(String),

    /// Lowering the validated request into the builder failed.
    #[error("optimize.run request lowering failed: {0}")]
    Lowering(String),

    /// The optimizer loop itself failed.
    #[error("optimize.run optimization failed: {0}")]
    Optimization(String),

    /// Projecting the durable result into the locked document failed.
    #[error("optimize.run result projection failed: {0}")]
    Projection(String),
}

impl OptimizeRunHostError {
    pub(super) fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported(message.into())
    }

    pub(super) fn worker_unavailable(message: impl Into<String>) -> Self {
        Self::WorkerUnavailable(message.into())
    }

    pub(super) fn lowering(message: impl Into<String>) -> Self {
        Self::Lowering(message.into())
    }

    pub(super) fn optimization(message: impl Into<String>) -> Self {
        Self::Optimization(message.into())
    }

    pub(super) fn projection(message: impl Into<String>) -> Self {
        Self::Projection(message.into())
    }
}

impl From<OptimizeRunHostError> for SeamServiceError {
    fn from(error: OptimizeRunHostError) -> Self {
        match error {
            // A missing worker argv is a configuration gap, not a runtime
            // failure, so it maps to the method-unavailable style like other
            // unwired service capabilities.
            OptimizeRunHostError::WorkerUnavailable(_) => Self::unavailable("leaven/optimize.run"),
            other => Self::execution(other.to_string()),
        }
    }
}
