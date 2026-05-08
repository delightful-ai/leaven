//! Run persistence capability.

use std::collections::BTreeMap;

use leaven_core::OptimizationProblem;
use leaven_kernel::{
    BlobRef, BudgetSnapshot, EvidenceRef, Fingerprint, PopulationId, RunId, StageId, Timestamp,
};
use serde::{Deserialize, Serialize};

use crate::{BudgetLedger, EvaluationCache, OptimizerStateSnapshot, RunGraph, StateFormat};

/// Capability for durable run checkpoints.
///
/// Persistence adapters own the world-facing details. The engine only depends
/// on this capability and its structured refusal modes.
pub trait RunPersistence<P: OptimizationProblem>: Send + Sync {
    /// Persist a checkpoint at a clean run boundary.
    fn checkpoint(&self, request: RunCheckpointRequest<'_, P>) -> Result<(), RunPersistenceError>;
}

/// Borrowed state available to a persistence adapter at a clean boundary.
///
/// The request borrows instead of cloning so persistence backends can decide
/// what to serialize, deduplicate, or ignore without forcing hot-path copies on
/// runs that do not use persistence.
#[derive(Clone, Copy)]
pub struct RunCheckpointRequest<'a, P: OptimizationProblem> {
    pub graph: &'a RunGraph<P>,
    pub budget: &'a BudgetLedger,
    pub cache: Option<&'a EvaluationCache>,
}

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
        }
    }

    #[must_use]
    pub fn run_id(&self) -> RunId {
        self.graph.run_id
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
