//! Run persistence capability.

use std::{collections::BTreeMap, sync::Arc};

use bytes::Bytes;
use leaven_core::OptimizationProblem;
use leaven_kernel::{
    BlobRef, BudgetSnapshot, EvidenceRef, Fingerprint, PopulationId, RunId, StageId, Timestamp, now,
};
use leaven_store::{BlobStore, BlobWrite, CheckpointBytes, CheckpointStore, StoreError};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    BudgetLedger, EvaluationCache, EvaluationCacheSnapshot, OptimizerStateReader,
    OptimizerStateSnapshot, RunGraph, RunGraphRestoreError, RunGraphSnapshot, StateFormat,
};

/// Capability for durable run checkpoints.
///
/// Persistence adapters own the world-facing details. The engine only depends
/// on this capability and its structured refusal modes.
pub trait RunPersistence<P: OptimizationProblem>: Send + Sync {
    /// Persist a checkpoint at a clean run boundary.
    fn checkpoint(&self, request: RunCheckpointRequest<'_, P>) -> Result<(), RunPersistenceError>;
}

impl<P, T> RunPersistence<P> for Arc<T>
where
    P: OptimizationProblem,
    T: RunPersistence<P> + ?Sized,
{
    fn checkpoint(&self, request: RunCheckpointRequest<'_, P>) -> Result<(), RunPersistenceError> {
        self.as_ref().checkpoint(request)
    }
}

/// [`RunPersistence`] implementation backed by Leaven's blob and checkpoint
/// store capabilities.
///
/// Graph truth and cache indexes are serialized as blobs; the checkpoint
/// envelope stores references to those blobs. This keeps checkpoint records
/// small while preserving enough durable state for resume/replay paths.
#[derive(Clone, Debug)]
pub struct StoreRunPersistence<S> {
    store: S,
}

impl<S> StoreRunPersistence<S> {
    #[must_use]
    pub fn new(store: S) -> Self {
        Self { store }
    }

    #[must_use]
    pub fn store(&self) -> &S {
        &self.store
    }
}

impl<S> StoreRunPersistence<S>
where
    S: BlobStore + CheckpointStore,
{
    pub fn latest_checkpoint<P>(&self) -> Result<Option<RestoredRunState<P>>, RunPersistenceError>
    where
        P: OptimizationProblem,
        P::Artifact: DeserializeOwned,
        <P::Artifact as leaven_core::Artifact>::Change: DeserializeOwned,
        P::ProposalAnnotations: DeserializeOwned,
    {
        let Some(id) =
            CheckpointStore::latest(&self.store).map_err(|source| RunPersistenceError::Store {
                operation: "read latest checkpoint pointer",
                source,
            })?
        else {
            return Ok(None);
        };
        self.load_checkpoint(id).map(Some)
    }

    pub fn load_checkpoint<P>(
        &self,
        id: leaven_kernel::CheckpointId,
    ) -> Result<RestoredRunState<P>, RunPersistenceError>
    where
        P: OptimizationProblem,
        P::Artifact: DeserializeOwned,
        <P::Artifact as leaven_core::Artifact>::Change: DeserializeOwned,
        P::ProposalAnnotations: DeserializeOwned,
    {
        let checkpoint_bytes =
            CheckpointStore::get(&self.store, id).map_err(|source| RunPersistenceError::Store {
                operation: "read checkpoint envelope",
                source,
            })?;
        let checkpoint: RunCheckpoint =
            serde_json::from_slice(&checkpoint_bytes.0).map_err(|err| {
                RunPersistenceError::Serialization {
                    state: "run checkpoint envelope",
                    reason: err.to_string(),
                }
            })?;
        let graph_bytes =
            BlobStore::get(&self.store, &checkpoint.graph_snapshot.bytes).map_err(|source| {
                RunPersistenceError::Store {
                    operation: "read graph snapshot blob",
                    source,
                }
            })?;
        let graph_snapshot: RunGraphSnapshot<P> =
            serde_json::from_slice(&graph_bytes).map_err(|err| {
                RunPersistenceError::Serialization {
                    state: "graph snapshot",
                    reason: err.to_string(),
                }
            })?;
        let graph = RunGraph::from_snapshot(graph_snapshot)
            .map_err(|source| RunPersistenceError::RestoreGraph { source })?;
        let cache = checkpoint
            .cache_index
            .as_ref()
            .map(|cache_ref| {
                let cache_bytes =
                    BlobStore::get(&self.store, &cache_ref.bytes).map_err(|source| {
                        RunPersistenceError::Store {
                            operation: "read evaluation cache blob",
                            source,
                        }
                    })?;
                let snapshot: EvaluationCacheSnapshot = serde_json::from_slice(&cache_bytes)
                    .map_err(|err| RunPersistenceError::Serialization {
                        state: "evaluation cache index",
                        reason: err.to_string(),
                    })?;
                Ok(EvaluationCache::from_snapshot(snapshot))
            })
            .transpose()?;
        let budget = BudgetLedger::from_snapshot(checkpoint.budget_ledger.clone());

        Ok(RestoredRunState {
            checkpoint,
            graph,
            budget,
            cache,
        })
    }

    pub fn load_optimizer_state<T>(
        &self,
        checkpoint: &RunCheckpoint,
        optimizer: Fingerprint,
        schema: Fingerprint,
    ) -> Result<Option<T>, RunPersistenceError>
    where
        T: DeserializeOwned,
    {
        let Some(state) = &checkpoint.optimizer_state else {
            return Ok(None);
        };
        if state.optimizer != optimizer {
            return Err(RunPersistenceError::IncompatibleState {
                state: "optimizer state",
                reason: format!(
                    "checkpoint optimizer fingerprint {:?} does not match requested {:?}",
                    state.optimizer, optimizer
                ),
            });
        }
        if state.schema != schema {
            return Err(RunPersistenceError::IncompatibleState {
                state: "optimizer state",
                reason: format!(
                    "checkpoint optimizer state schema {:?} does not match requested {:?}",
                    state.schema, schema
                ),
            });
        }
        if state.format != StateFormat::Json {
            return Err(RunPersistenceError::IncompatibleState {
                state: "optimizer state",
                reason: format!("unsupported state format {:?}", state.format),
            });
        }
        let bytes = BlobStore::get(&self.store, &state.bytes).map_err(|source| {
            RunPersistenceError::Store {
                operation: "read optimizer state blob",
                source,
            }
        })?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|err| RunPersistenceError::Serialization {
                state: "optimizer state",
                reason: err.to_string(),
            })
    }
}

impl<S> OptimizerStateReader for StoreRunPersistence<S>
where
    S: BlobStore + CheckpointStore,
{
    fn load_optimizer_state<T>(
        &self,
        checkpoint: &RunCheckpoint,
        optimizer: Fingerprint,
        schema: Fingerprint,
    ) -> Result<Option<T>, RunPersistenceError>
    where
        T: DeserializeOwned,
    {
        Self::load_optimizer_state(self, checkpoint, optimizer, schema)
    }
}

pub struct RestoredRunState<P: OptimizationProblem> {
    pub checkpoint: RunCheckpoint,
    pub graph: RunGraph<P>,
    pub budget: BudgetLedger,
    pub cache: Option<EvaluationCache>,
}

impl<P, S> RunPersistence<P> for StoreRunPersistence<S>
where
    P: OptimizationProblem,
    P::Artifact: Serialize,
    <P::Artifact as leaven_core::Artifact>::Change: Serialize,
    P::ProposalAnnotations: Serialize,
    S: BlobStore + CheckpointStore,
{
    fn checkpoint(&self, request: RunCheckpointRequest<'_, P>) -> Result<(), RunPersistenceError> {
        let graph_snapshot = request.graph.snapshot();
        let graph_ref = put_json_blob(
            &self.store,
            "graph snapshot",
            &graph_snapshot,
            RUN_GRAPH_SNAPSHOT_SCHEMA,
        )?;
        let mut checkpoint = RunCheckpoint::new(
            request.run_id(),
            now(),
            GraphSnapshotRef {
                schema: RUN_GRAPH_SNAPSHOT_SCHEMA,
                format: StateFormat::Json,
                bytes: graph_ref,
            },
            request.budget.snapshot(),
        );

        if let Some(state) = request.optimizer_state {
            let bytes = BlobStore::put(
                &self.store,
                BlobWrite {
                    bytes: state.bytes,
                    content_type: Some(state.content_type),
                },
            )
            .map_err(|source| RunPersistenceError::Store {
                operation: "write optimizer state blob",
                source,
            })?;
            checkpoint.optimizer_state = Some(OptimizerStateSnapshot {
                optimizer: state.optimizer,
                schema: state.schema,
                format: state.format,
                bytes,
            });
        }

        if let Some(cache) = request.cache {
            if !cache.is_empty() {
                let cache_ref = put_json_blob(
                    &self.store,
                    "evaluation cache index",
                    &cache.snapshot(),
                    EVALUATION_CACHE_SCHEMA,
                )?;
                checkpoint.cache_index = Some(CacheIndexSnapshot {
                    schema: EVALUATION_CACHE_SCHEMA,
                    format: StateFormat::Json,
                    bytes: cache_ref,
                });
            }
        }

        let bytes = serde_json::to_vec_pretty(&checkpoint).map_err(|err| {
            RunPersistenceError::Serialization {
                state: "run checkpoint envelope",
                reason: err.to_string(),
            }
        })?;
        CheckpointStore::put(&self.store, CheckpointBytes(Bytes::from(bytes)))
            .map(|_| ())
            .map_err(|source| RunPersistenceError::Store {
                operation: "write checkpoint envelope",
                source,
            })
    }
}

fn put_json_blob<S, T>(
    store: &S,
    state: &'static str,
    value: &T,
    _schema: Fingerprint,
) -> Result<BlobRef, RunPersistenceError>
where
    S: BlobStore,
    T: Serialize,
{
    let bytes = serde_json::to_vec(value).map_err(|err| RunPersistenceError::Serialization {
        state,
        reason: err.to_string(),
    })?;
    store
        .put(BlobWrite {
            bytes: Bytes::from(bytes),
            content_type: Some("application/json".to_owned()),
        })
        .map_err(|source| RunPersistenceError::Store {
            operation: "write checkpoint blob",
            source,
        })
}

/// Borrowed state available to a persistence adapter at a clean boundary.
///
/// The request borrows instead of cloning so persistence backends can decide
/// what to serialize, deduplicate, or ignore without forcing hot-path copies on
/// runs that do not use persistence.
#[derive(Clone)]
pub struct RunCheckpointRequest<'a, P: OptimizationProblem> {
    pub graph: &'a RunGraph<P>,
    pub budget: &'a BudgetLedger,
    pub cache: Option<&'a EvaluationCache>,
    pub optimizer_state: Option<OptimizerStateWrite>,
}

const RUN_GRAPH_SNAPSHOT_SCHEMA: Fingerprint = Fingerprint::from_bytes([11; 32]);
const EVALUATION_CACHE_SCHEMA: Fingerprint = Fingerprint::from_bytes([12; 32]);

impl<'a, P: OptimizationProblem> RunCheckpointRequest<'a, P> {
    #[must_use]
    pub fn new(
        graph: &'a RunGraph<P>,
        budget: &'a BudgetLedger,
        cache: Option<&'a EvaluationCache>,
    ) -> Self {
        Self {
            graph,
            budget,
            cache,
            optimizer_state: None,
        }
    }

    #[must_use]
    pub fn with_optimizer_state(mut self, state: OptimizerStateWrite) -> Self {
        self.optimizer_state = Some(state);
        self
    }

    #[must_use]
    pub fn run_id(&self) -> RunId {
        self.graph.run_id
    }
}

/// Private optimizer state to persist with the next run checkpoint.
#[derive(Clone, Debug)]
pub struct OptimizerStateWrite {
    pub optimizer: Fingerprint,
    pub schema: Fingerprint,
    pub format: StateFormat,
    pub bytes: Bytes,
    pub content_type: String,
}

impl OptimizerStateWrite {
    pub fn json<T>(
        optimizer: Fingerprint,
        schema: Fingerprint,
        value: &T,
    ) -> Result<Self, RunPersistenceError>
    where
        T: Serialize,
    {
        let bytes =
            serde_json::to_vec(value).map_err(|err| RunPersistenceError::Serialization {
                state: "optimizer state",
                reason: err.to_string(),
            })?;
        Ok(Self {
            optimizer,
            schema,
            format: StateFormat::Json,
            bytes: Bytes::from(bytes),
            content_type: "application/json".to_owned(),
        })
    }
}

/// Durable checkpoint envelope for one clean run boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunCheckpoint {
    pub format_version: u32,
    pub run_id: RunId,
    pub created_at: Timestamp,
    pub graph_snapshot: GraphSnapshotRef,
    pub optimizer_state: Option<OptimizerStateSnapshot>,
    pub population_states: BTreeMap<PopulationId, StageStateSnapshot>,
    pub selector_states: BTreeMap<StageId, StageStateSnapshot>,
    pub admission_states: BTreeMap<StageId, StageStateSnapshot>,
    pub budget_ledger: BudgetSnapshot,
    pub cache_index: Option<CacheIndexSnapshot>,
    pub artifact_refs: Vec<BlobRef>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub stage_journal: StageJournalSnapshot,
    pub workspace_journal: WorkspaceJournalSnapshot,
}

impl RunCheckpoint {
    pub const FORMAT_VERSION: u32 = 1;

    #[must_use]
    pub fn new(
        run_id: RunId,
        created_at: Timestamp,
        graph_snapshot: GraphSnapshotRef,
        budget_ledger: BudgetSnapshot,
    ) -> Self {
        Self {
            format_version: Self::FORMAT_VERSION,
            run_id,
            created_at,
            graph_snapshot,
            optimizer_state: None,
            population_states: BTreeMap::new(),
            selector_states: BTreeMap::new(),
            admission_states: BTreeMap::new(),
            budget_ledger,
            cache_index: None,
            artifact_refs: Vec::new(),
            evidence_refs: Vec::new(),
            stage_journal: StageJournalSnapshot::default(),
            workspace_journal: WorkspaceJournalSnapshot::default(),
        }
    }
}

/// Blob reference for serialized graph truth.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GraphSnapshotRef {
    pub schema: Fingerprint,
    pub format: StateFormat,
    pub bytes: BlobRef,
}

/// Blob reference for private stage state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageStateSnapshot {
    pub stage: StageId,
    pub schema: Fingerprint,
    pub format: StateFormat,
    pub bytes: BlobRef,
}

/// Blob reference for a serialized cache index.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CacheIndexSnapshot {
    pub schema: Fingerprint,
    pub format: StateFormat,
    pub bytes: BlobRef,
}

/// Durable stage-boundary journal reference.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageJournalSnapshot {
    pub entries: Vec<BlobRef>,
}

/// Durable workspace lifecycle journal reference.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceJournalSnapshot {
    pub entries: Vec<BlobRef>,
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
    /// A checkpoint payload could not be serialized.
    #[error("run persistence could not serialize {state}: {reason}")]
    Serialization { state: &'static str, reason: String },
    /// A stored private-state payload does not match the requesting component.
    #[error("run persistence restored incompatible {state}: {reason}")]
    IncompatibleState { state: &'static str, reason: String },
    /// The backing store refused a persistence operation.
    #[error("run persistence store refused {operation}")]
    Store {
        operation: &'static str,
        #[source]
        source: StoreError,
    },
    /// Stored graph truth could not be restored into a valid run graph.
    #[error("run persistence restored invalid graph truth")]
    RestoreGraph {
        #[source]
        source: RunGraphRestoreError,
    },
}

#[cfg(test)]
mod tests {
    use leaven_kernel::{BlobRef, Budget, BudgetSnapshot, Fingerprint, RunId, now};

    use crate::{GraphSnapshotRef, RunCheckpoint, StateFormat};

    #[test]
    fn run_checkpoint_envelope_round_trips_as_json() {
        let checkpoint = RunCheckpoint::new(
            RunId::new(),
            now(),
            GraphSnapshotRef {
                schema: Fingerprint::from_bytes([1; 32]),
                format: StateFormat::Json,
                bytes: BlobRef {
                    store: "file".to_owned(),
                    key: "graph.json".to_owned(),
                },
            },
            BudgetSnapshot {
                limit: Budget::unlimited(),
                ..BudgetSnapshot::default()
            },
        );

        let encoded = serde_json::to_vec(&checkpoint).unwrap();
        let decoded: RunCheckpoint = serde_json::from_slice(&encoded).unwrap();

        assert_eq!(decoded.format_version, RunCheckpoint::FORMAT_VERSION);
        assert_eq!(decoded.graph_snapshot.bytes.key, "graph.json");
        assert!(decoded.optimizer_state.is_none());
    }
}
