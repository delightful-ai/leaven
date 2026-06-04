//! Rust-owned local run inspection export.

use std::path::Path;

use leaven_engine::RunCheckpoint;
use leaven_kernel::{BlobRef, CheckpointId, RunId};
use leaven_store::{BlobStore, CheckpointStore};
use leaven_store_file::FileStore;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

/// Schema name for the Rust-owned run inspection export.
pub const RUN_INSPECTION_EXPORT_SCHEMA: &str = "leaven.run_inspection_export.v1";

/// Rust-owned inspection export for a local Leaven run directory.
#[derive(Clone, Debug, Serialize)]
pub struct RustRunInspectionExport {
    /// Export schema.
    pub schema_version: &'static str,
    /// Run id from the latest checkpoint envelope.
    pub run_id: RunId,
    /// Latest checkpoint id resolved by the local store.
    pub latest_checkpoint: CheckpointId,
    /// Checkpoint envelope facts.
    pub checkpoint: CheckpointInspection,
    /// Resolved graph snapshot blob facts.
    pub graph: GraphInspection,
}

/// Checkpoint envelope facts needed by external-language inspection.
#[derive(Clone, Debug, Serialize)]
pub struct CheckpointInspection {
    /// Checkpoint format version.
    pub format_version: u32,
    /// Serialized graph snapshot reference.
    pub graph_snapshot: BlobInspectionRef,
    /// Number of artifact blob refs carried by the checkpoint envelope.
    pub artifact_ref_count: usize,
    /// Number of evidence refs carried by the checkpoint envelope.
    pub evidence_ref_count: usize,
    /// Number of stage journal blob refs carried by the checkpoint envelope.
    pub stage_journal_ref_count: usize,
    /// Number of workspace journal blob refs carried by the checkpoint envelope.
    pub workspace_journal_ref_count: usize,
    /// Whether optimizer private state exists in the checkpoint envelope.
    pub has_optimizer_state: bool,
    /// Whether an evaluation cache index exists in the checkpoint envelope.
    pub has_cache_index: bool,
}

/// Resolved blob reference facts.
#[derive(Clone, Debug, Serialize)]
pub struct BlobInspectionRef {
    /// Blob store name.
    pub store: String,
    /// Blob key.
    pub key: String,
    /// Schema fingerprint bound to the blob payload.
    pub schema: String,
    /// State format recorded by the checkpoint.
    pub format: String,
}

/// Graph snapshot facts decoded from the graph snapshot blob.
#[derive(Clone, Debug, Serialize)]
pub struct GraphInspection {
    /// Graph snapshot blob reference.
    pub blob: BlobInspectionRef,
    /// Number of bytes read through Rust's blob store.
    pub bytes: usize,
    /// Run id recorded inside the graph snapshot JSON, if present.
    pub run_id: Option<String>,
    /// Number of candidate records in the graph snapshot.
    pub candidate_count: usize,
    /// Number of proposal-batch records in the graph snapshot.
    pub proposal_batch_count: usize,
    /// Number of proposal records in the graph snapshot.
    pub proposal_count: usize,
    /// Number of apply-attempt records in the graph snapshot.
    pub apply_attempt_count: usize,
    /// Number of evaluation-request records in the graph snapshot.
    pub evaluation_request_count: usize,
    /// Number of assessment records in the graph snapshot.
    pub assessment_count: usize,
    /// Number of event records in the graph snapshot.
    pub event_count: usize,
}

/// Export a Rust-owned inspection artifact from a local run directory.
///
/// This reads the latest checkpoint through [`FileStore`] and resolves the
/// checkpoint's graph snapshot blob through Rust's blob-store capability. It
/// intentionally does not parse problem-specific artifact payloads.
pub fn export_local_run_inspection(
    run_dir: impl AsRef<Path>,
) -> Result<RustRunInspectionExport, RunInspectionExportError> {
    let store =
        FileStore::open(run_dir.as_ref()).map_err(|source| RunInspectionExportError::Store {
            operation: "open local run store",
            source,
        })?;
    let latest_checkpoint = store
        .latest()
        .map_err(|source| RunInspectionExportError::Store {
            operation: "read latest checkpoint pointer",
            source,
        })?
        .ok_or(RunInspectionExportError::MissingLatestCheckpoint)?;
    let checkpoint_bytes = CheckpointStore::get(&store, latest_checkpoint).map_err(|source| {
        RunInspectionExportError::Store {
            operation: "read latest checkpoint envelope",
            source,
        }
    })?;
    let checkpoint: RunCheckpoint =
        serde_json::from_slice(&checkpoint_bytes.0).map_err(|source| {
            RunInspectionExportError::Decode {
                state: "checkpoint envelope",
                source,
            }
        })?;
    let graph_bytes =
        BlobStore::get(&store, &checkpoint.graph_snapshot.bytes).map_err(|source| {
            RunInspectionExportError::Store {
                operation: "read graph snapshot blob",
                source,
            }
        })?;
    let graph_json: Value = serde_json::from_slice(&graph_bytes).map_err(|source| {
        RunInspectionExportError::Decode {
            state: "graph snapshot blob",
            source,
        }
    })?;
    let checkpoint_inspection = CheckpointInspection {
        format_version: checkpoint.format_version,
        graph_snapshot: blob_ref(&checkpoint.graph_snapshot.bytes, &checkpoint.graph_snapshot),
        artifact_ref_count: checkpoint.artifact_refs.len(),
        evidence_ref_count: checkpoint.evidence_refs.len(),
        stage_journal_ref_count: checkpoint.stage_journal.entries.len(),
        workspace_journal_ref_count: checkpoint.workspace_journal.entries.len(),
        has_optimizer_state: checkpoint.optimizer_state.is_some(),
        has_cache_index: checkpoint.cache_index.is_some(),
    };
    let graph = GraphInspection {
        blob: checkpoint_inspection.graph_snapshot.clone(),
        bytes: graph_bytes.len(),
        run_id: graph_json
            .get("run_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        candidate_count: array_len(&graph_json, "candidates"),
        proposal_batch_count: array_len(&graph_json, "proposal_batches"),
        proposal_count: array_len(&graph_json, "proposals"),
        apply_attempt_count: array_len(&graph_json, "apply_attempts"),
        evaluation_request_count: array_len(&graph_json, "evaluation_requests"),
        assessment_count: array_len(&graph_json, "assessments"),
        event_count: array_len(&graph_json, "events"),
    };
    Ok(RustRunInspectionExport {
        schema_version: RUN_INSPECTION_EXPORT_SCHEMA,
        run_id: checkpoint.run_id,
        latest_checkpoint,
        checkpoint: checkpoint_inspection,
        graph,
    })
}

fn blob_ref(reference: &BlobRef, graph: &leaven_engine::GraphSnapshotRef) -> BlobInspectionRef {
    BlobInspectionRef {
        store: reference.store.clone(),
        key: reference.key.clone(),
        schema: graph.schema.to_hex(),
        format: format!("{:?}", graph.format),
    }
}

fn array_len(value: &Value, field: &str) -> usize {
    value
        .get(field)
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

/// Errors returned while exporting Rust-owned run inspection.
#[derive(Debug, Error)]
pub enum RunInspectionExportError {
    /// The local run store has no latest checkpoint pointer.
    #[error("local run store has no latest checkpoint")]
    MissingLatestCheckpoint,
    /// File store operation failed.
    #[error("run inspection export failed during {operation}")]
    Store {
        /// Operation label.
        operation: &'static str,
        /// Store failure.
        #[source]
        source: leaven_store::StoreError,
    },
    /// Stored JSON could not be decoded.
    #[error("run inspection export could not decode {state}")]
    Decode {
        /// State label.
        state: &'static str,
        /// Decode failure.
        #[source]
        source: serde_json::Error,
    },
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use leaven_engine::{GraphSnapshotRef, RunCheckpoint, StateFormat};
    use leaven_kernel::{BudgetSnapshot, Fingerprint, now};
    use leaven_store::{BlobWrite, CheckpointBytes};

    use super::*;

    #[test]
    fn export_local_run_inspection_reads_latest_checkpoint_and_graph_blob() {
        let run_dir = test_run_dir("graph-readback");
        let store = FileStore::open(&run_dir).unwrap();
        let graph_json = br#"{
            "run_id":"run_export",
            "candidates":[{},{}],
            "proposal_batches":[{}],
            "proposals":[{}],
            "apply_attempts":[{}],
            "evaluation_requests":[{}],
            "assessments":[{},{}],
            "events":[{},{},{}]
        }"#;
        let graph_blob = BlobStore::put(
            &store,
            BlobWrite {
                bytes: Bytes::from_static(graph_json),
                content_type: Some("application/json".to_owned()),
            },
        )
        .unwrap();
        let checkpoint = RunCheckpoint::new(
            leaven_kernel::RunId::new(),
            now(),
            GraphSnapshotRef {
                schema: Fingerprint::from_bytes([7; 32]),
                format: StateFormat::Json,
                bytes: graph_blob.clone(),
            },
            BudgetSnapshot::default(),
        );
        let checkpoint_bytes = serde_json::to_vec(&checkpoint).unwrap();
        let checkpoint_id =
            CheckpointStore::put(&store, CheckpointBytes(Bytes::from(checkpoint_bytes))).unwrap();
        CheckpointStore::mark_latest(&store, checkpoint_id).unwrap();

        let export = export_local_run_inspection(&run_dir).unwrap();

        assert_eq!(export.schema_version, RUN_INSPECTION_EXPORT_SCHEMA);
        assert_eq!(export.latest_checkpoint, checkpoint_id);
        assert_eq!(export.run_id, checkpoint.run_id);
        assert_eq!(export.checkpoint.graph_snapshot.key, graph_blob.key);
        assert_eq!(export.graph.bytes, graph_json.len());
        assert_eq!(export.graph.run_id.as_deref(), Some("run_export"));
        assert_eq!(export.graph.candidate_count, 2);
        assert_eq!(export.graph.proposal_batch_count, 1);
        assert_eq!(export.graph.proposal_count, 1);
        assert_eq!(export.graph.apply_attempt_count, 1);
        assert_eq!(export.graph.evaluation_request_count, 1);
        assert_eq!(export.graph.assessment_count, 2);
        assert_eq!(export.graph.event_count, 3);

        std::fs::remove_dir_all(run_dir).unwrap();
    }

    #[test]
    fn export_local_run_inspection_refuses_missing_latest_checkpoint() {
        let run_dir = test_run_dir("missing-latest");
        FileStore::open(&run_dir).unwrap();

        let error = export_local_run_inspection(&run_dir).unwrap_err();

        assert!(matches!(
            error,
            RunInspectionExportError::MissingLatestCheckpoint
        ));
        std::fs::remove_dir_all(run_dir).unwrap();
    }

    fn test_run_dir(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "leaven-run-inspection-{label}-{}",
            uuid::Uuid::new_v4()
        ))
    }
}
