//! Optimizer trait.

use std::{any::Any, sync::Arc};

use leaven_core::OptimizationProblem;
use leaven_kernel::{BlobRef, CandidateId, Fingerprint};
use serde::{Serialize, de::DeserializeOwned};

use crate::{OptimizerStateWrite, RunCheckpoint, RunGraphView, RunPersistenceError, StopReason};

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

    /// Optional typed optimizer-specific report captured by product result
    /// facades after a run finishes.
    fn optimizer_report(&self) -> Option<OptimizerReportPayload> {
        None
    }

    /// Observe a clean engine-owned stop before another optimizer step runs.
    ///
    /// This covers stops decided by the engine loop itself, such as a restored
    /// run whose budget is already exhausted before `step()` is called.
    fn on_engine_stop(&mut self, _reason: StopReason) -> Result<(), OptimizerError> {
        Ok(())
    }

    /// Captures serialized optimizer continuation state for a clean run
    /// checkpoint.
    ///
    /// Optimizers with no future-affecting private state return `None`.
    /// Optimizers with explicit private state should delegate to their
    /// checkpoint contract so ordinary durable runs preserve the next-decision
    /// state without callers remembering a second checkpoint path.
    fn checkpoint_state_write(
        &self,
        _ctx: CheckpointContext<'_, P>,
    ) -> Result<Option<OptimizerStateWrite>, OptimizerError> {
        Ok(None)
    }

    /// Restores optimizer continuation state from a durable run checkpoint.
    ///
    /// Stateless or graph-derived optimizers can use the default when the
    /// checkpoint has no private optimizer payload. If a checkpoint contains
    /// private state, the optimizer must override this and restore or reject it
    /// before the engine continues.
    fn restore_checkpoint_state<R>(
        &mut self,
        checkpoint: &RunCheckpoint,
        _reader: &R,
        _ctx: RestoreContext<'_, P>,
    ) -> Result<(), OptimizerError>
    where
        R: OptimizerStateReader,
    {
        if checkpoint.optimizer_state.is_some() {
            return Err(OptimizerError::Message(
                "stored run includes optimizer private state, but this optimizer does not implement resume restore".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Type-erased optimizer-specific report payload.
pub type OptimizerReportPayload = Arc<dyn OptimizerReport>;

/// Optimizer-specific report payload that can be downcast by product facades.
pub trait OptimizerReport: std::fmt::Debug + Send + Sync + 'static {
    /// Type-erased view for downcasting.
    fn as_any(&self) -> &(dyn Any + 'static);
}

impl<T> OptimizerReport for T
where
    T: std::fmt::Debug + Send + Sync + 'static,
{
    fn as_any(&self) -> &(dyn Any + 'static) {
        self
    }
}

/// Capability for reading typed optimizer continuation payloads referenced by
/// a run checkpoint.
pub trait OptimizerStateReader {
    /// Load and validate a JSON optimizer state payload.
    fn load_optimizer_state<T>(
        &self,
        checkpoint: &RunCheckpoint,
        optimizer: Fingerprint,
        schema: Fingerprint,
    ) -> Result<Option<T>, RunPersistenceError>
    where
        T: DeserializeOwned;
}

/// Restores a [`CheckpointableOptimizer`] from a stored run checkpoint.
pub fn restore_checkpointable_optimizer_state<P, O, R>(
    optimizer: &mut O,
    checkpoint: &RunCheckpoint,
    reader: &R,
    ctx: RestoreContext<'_, P>,
) -> Result<(), OptimizerError>
where
    P: OptimizationProblem,
    O: CheckpointableOptimizer<P>,
    R: OptimizerStateReader,
{
    match optimizer.private_state_policy() {
        PrivateStatePolicy::DerivedFromGraph => {
            if checkpoint.optimizer_state.is_some() {
                return Err(OptimizerError::with_source(
                    "optimizer private state restore failed",
                    CheckpointError::IncompatibleState {
                        reason:
                            "checkpoint contains private optimizer state for a graph-derived optimizer"
                                .to_owned(),
                    },
                ));
            }
            Ok(())
        }
        PrivateStatePolicy::ExplicitSnapshot { schema, format } => {
            if format != StateFormat::Json {
                return Err(OptimizerError::with_source(
                    "optimizer private state restore failed",
                    CheckpointError::IncompatibleState {
                        reason: format!(
                            "run persistence currently restores JSON optimizer state, got {format:?}"
                        ),
                    },
                ));
            }
            let state = reader
                .load_optimizer_state::<O::State>(
                    checkpoint,
                    optimizer.optimizer_fingerprint(),
                    schema,
                )
                .map_err(|source| {
                    OptimizerError::with_source("optimizer private state restore failed", source)
                })?
                .ok_or_else(|| {
                    OptimizerError::with_source(
                        "optimizer private state restore failed",
                        CheckpointError::StateUnavailable {
                            reason: "checkpoint does not contain optimizer private state"
                                .to_owned(),
                        },
                    )
                })?;
            optimizer.restore_state(state, ctx).map_err(|source| {
                OptimizerError::with_source("optimizer private state restore failed", source)
            })
        }
    }
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

    /// Stable optimizer identity used to validate restored continuation state.
    fn optimizer_fingerprint(&self) -> Fingerprint;

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

    /// Serializes checkpoint state for a run persistence adapter.
    fn checkpoint_state_write(
        &self,
        ctx: CheckpointContext<'_, P>,
    ) -> Result<Option<OptimizerStateWrite>, OptimizerError> {
        match self.private_state_policy() {
            PrivateStatePolicy::DerivedFromGraph => Ok(None),
            PrivateStatePolicy::ExplicitSnapshot { schema, format } => {
                if format != StateFormat::Json {
                    return Err(OptimizerError::with_source(
                        "optimizer private state checkpoint failed",
                        CheckpointError::IncompatibleState {
                            reason: format!(
                                "run persistence currently writes JSON optimizer state, got {format:?}"
                            ),
                        },
                    ));
                }
                let state = self.checkpoint_state(ctx).map_err(|source| {
                    OptimizerError::with_source("optimizer private state checkpoint failed", source)
                })?;
                OptimizerStateWrite::json(self.optimizer_fingerprint(), schema, &state)
                    .map(Some)
                    .map_err(|source| {
                        OptimizerError::with_source(
                            "optimizer private state serialization failed",
                            source,
                        )
                    })
            }
        }
    }
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
    Stopped(crate::StopReason),
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
