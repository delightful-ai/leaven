//! Optimizer trait.

use leaven_core::OptimizationProblem;
use leaven_kernel::{BlobRef, CandidateId, Fingerprint};
use serde::{Serialize, de::DeserializeOwned};

use crate::RunGraphView;

#[allow(async_fn_in_trait)]
pub trait Optimizer<P: OptimizationProblem>: Send {
    async fn initialize(
        &mut self,
        _ctx: &mut crate::RunContext<'_, P>,
    ) -> Result<(), OptimizerError> {
        Ok(())
    }

    async fn step(
        &mut self,
        ctx: &mut crate::RunContext<'_, P>,
    ) -> Result<StepStatus, OptimizerError>;

    fn best_candidate(&self, graph: RunGraphView<'_, P>) -> Option<CandidateId>;
}

/// Optimizer capability for durable private-state checkpointing.
///
/// The run graph records public truth. Optimizers that keep additional private
/// state must expose it explicitly through this trait so restore can either
/// rebuild the optimizer faithfully or fail before continuing.
pub trait CheckpointableOptimizer<P>: Optimizer<P>
where
    P: OptimizationProblem,
{
    type State: Serialize + DeserializeOwned;

    /// Describes whether private state is derived from graph truth or must be
    /// snapshotted explicitly.
    fn private_state_policy(&self) -> PrivateStatePolicy;

    /// Captures the optimizer's current private state.
    fn checkpoint_state(
        &self,
        ctx: CheckpointContext<'_, P>,
    ) -> Result<Self::State, CheckpointError>;

    /// Restores a previously captured private state.
    fn restore_state(
        &mut self,
        state: Self::State,
        ctx: RestoreContext<'_, P>,
    ) -> Result<(), CheckpointError>;
}

/// Format metadata for serialized private state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
pub enum StateFormat {
    Json,
    Postcard,
    Custom(String),
}

/// How a stage's private state participates in checkpoint/restore.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
pub enum PrivateStatePolicy {
    DerivedFromGraph,
    ExplicitSnapshot {
        schema: Fingerprint,
        format: StateFormat,
    },
}

/// Stored reference to optimizer private state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
pub struct OptimizerStateSnapshot {
    pub optimizer: Fingerprint,
    pub schema: Fingerprint,
    pub format: StateFormat,
    pub bytes: BlobRef,
}

/// Read-only context passed while capturing private state.
pub struct CheckpointContext<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
}

impl<'a, P: OptimizationProblem> CheckpointContext<'a, P> {
    #[must_use]
    pub fn new(graph: RunGraphView<'a, P>) -> Self {
        Self { graph }
    }

    #[must_use]
    pub fn graph(&self) -> RunGraphView<'a, P> {
        self.graph.clone()
    }
}

/// Read-only context passed while restoring private state.
pub struct RestoreContext<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
}

impl<'a, P: OptimizationProblem> RestoreContext<'a, P> {
    #[must_use]
    pub fn new(graph: RunGraphView<'a, P>) -> Self {
        Self { graph }
    }

    #[must_use]
    pub fn graph(&self) -> RunGraphView<'a, P> {
        self.graph.clone()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum StepStatus {
    Continue,
    Done,
}

#[derive(Debug, thiserror::Error)]
pub enum OptimizerError {
    #[error("optimizer failed: {0}")]
    Message(String),
    #[error("optimizer failed: {message}")]
    WithSource {
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

/// Failures while capturing or restoring optimizer private state.
#[derive(Debug, thiserror::Error)]
pub enum CheckpointError {
    #[error("checkpoint state is unavailable: {reason}")]
    StateUnavailable { reason: String },
    #[error("checkpoint state is incompatible: {reason}")]
    IncompatibleState { reason: String },
    #[error("checkpoint state references missing graph entity: {reason}")]
    MissingGraphTruth { reason: String },
    #[error("checkpoint operation failed: {message}")]
    WithSource {
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl OptimizerError {
    #[must_use]
    pub fn with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::WithSource {
            message: message.into(),
            source: Box::new(source),
        }
    }
}
