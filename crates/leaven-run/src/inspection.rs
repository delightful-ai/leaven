//! Rust-owned local run inspection export.

use std::path::Path;

use base64::{Engine as _, engine::general_purpose};
use leaven_engine::RunCheckpoint;
use leaven_kernel::{BlobRef, CheckpointId, EvidenceRef, RunId};
use leaven_store::{BlobStore, CheckpointStore};
use leaven_store_file::FileStore;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Schema name for the Rust-owned run inspection export.
pub const RUN_INSPECTION_EXPORT_SCHEMA: &str = "leaven.run_inspection_export.v1";

/// Schema name for the Rust-owned run blob export.
pub const RUN_BLOB_EXPORT_SCHEMA: &str = "leaven.run_blob_export.v1";

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
    /// Artifact blob refs carried by the checkpoint envelope.
    pub artifact_refs: Vec<BlobByteReadbackRef>,
    /// Number of artifact blob refs carried by the checkpoint envelope.
    pub artifact_ref_count: usize,
    /// Evidence refs carried by the checkpoint envelope.
    pub evidence_refs: Vec<EvidenceReadbackRef>,
    /// Number of evidence refs carried by the checkpoint envelope.
    pub evidence_ref_count: usize,
    /// Stage journal blob refs carried by the checkpoint envelope.
    pub stage_journal_refs: Vec<BlobByteReadbackRef>,
    /// Number of stage journal blob refs carried by the checkpoint envelope.
    pub stage_journal_ref_count: usize,
    /// Workspace journal blob refs carried by the checkpoint envelope.
    pub workspace_journal_refs: Vec<BlobByteReadbackRef>,
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
    /// Best candidate recorded by the completed run event, if present.
    pub best_candidate_id: Option<String>,
    /// Candidate records projected from Rust-owned graph snapshot JSON.
    pub candidates: Vec<CandidateReadback>,
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

/// Candidate facts projected from the Rust-owned graph snapshot.
#[derive(Clone, Debug, Serialize)]
pub struct CandidateReadback {
    /// Graph-local candidate id.
    pub id: String,
    /// Parent candidate id for proposal-created candidates.
    pub parent_id: Option<String>,
    /// Serialized problem artifact payload stored in the graph snapshot.
    pub artifact: Value,
}

/// Rust-owned byte export for one blob in a local Leaven run directory.
#[derive(Clone, Debug, Serialize)]
pub struct RustRunBlobExport {
    /// Export schema.
    pub schema_version: &'static str,
    /// Blob reference resolved through Rust's blob-store capability.
    pub blob: BlobByteReadbackRef,
    /// Number of bytes read from the store.
    pub bytes: usize,
    /// SHA-256 digest of the retrieved bytes.
    pub sha256: String,
    /// Base64-encoded blob contents.
    pub content_base64: String,
}

/// Blob reference facts for byte readback.
#[derive(Clone, Debug, Serialize)]
pub struct BlobByteReadbackRef {
    /// Blob store name.
    pub store: String,
    /// Blob key.
    pub key: String,
}

/// Evidence reference facts exposed by checkpoint readback.
#[derive(Clone, Debug, Serialize)]
pub struct EvidenceReadbackRef {
    /// Evidence store name.
    pub store: String,
    /// Evidence key.
    pub key: String,
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
        artifact_refs: checkpoint.artifact_refs.iter().map(blob_byte_ref).collect(),
        artifact_ref_count: checkpoint.artifact_refs.len(),
        evidence_refs: checkpoint.evidence_refs.iter().map(evidence_ref).collect(),
        evidence_ref_count: checkpoint.evidence_refs.len(),
        stage_journal_refs: checkpoint
            .stage_journal
            .entries
            .iter()
            .map(blob_byte_ref)
            .collect(),
        stage_journal_ref_count: checkpoint.stage_journal.entries.len(),
        workspace_journal_refs: checkpoint
            .workspace_journal
            .entries
            .iter()
            .map(blob_byte_ref)
            .collect(),
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
        best_candidate_id: best_candidate_id(&graph_json),
        candidates: candidate_readbacks(&graph_json),
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

/// Export bytes for one blob from a local run store.
///
/// This is the Rust-owned byte readback companion to
/// [`export_local_run_inspection`]. External-language inspection uses it after
/// Rust has exposed a blob store/key pair; callers must not infer paths or read
/// the store layout directly.
pub fn export_local_run_blob(
    run_dir: impl AsRef<Path>,
    store_name: impl Into<String>,
    key: impl Into<String>,
) -> Result<RustRunBlobExport, RunInspectionExportError> {
    let store_name = store_name.into();
    let key = key.into();
    let store = FileStore::open_named(store_name.clone(), run_dir.as_ref()).map_err(|source| {
        RunInspectionExportError::Store {
            operation: "open local run store",
            source,
        }
    })?;
    let reference = BlobRef {
        store: store_name,
        key,
    };
    let bytes =
        BlobStore::get(&store, &reference).map_err(|source| RunInspectionExportError::Store {
            operation: "read run blob",
            source,
        })?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    Ok(RustRunBlobExport {
        schema_version: RUN_BLOB_EXPORT_SCHEMA,
        blob: BlobByteReadbackRef {
            store: reference.store,
            key: reference.key,
        },
        bytes: bytes.len(),
        sha256,
        content_base64: general_purpose::STANDARD.encode(&bytes),
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

fn blob_byte_ref(reference: &BlobRef) -> BlobByteReadbackRef {
    BlobByteReadbackRef {
        store: reference.store.clone(),
        key: reference.key.clone(),
    }
}

fn evidence_ref(reference: &EvidenceRef) -> EvidenceReadbackRef {
    EvidenceReadbackRef {
        store: reference.store.clone(),
        key: reference.key.clone(),
    }
}

fn best_candidate_id(graph: &Value) -> Option<String> {
    graph
        .get("events")?
        .as_array()?
        .iter()
        .rev()
        .find_map(|event| {
            event
                .get("OptimizationEnded")
                .and_then(|ended| ended.get("best"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
}

fn candidate_readbacks(graph: &Value) -> Vec<CandidateReadback> {
    graph
        .get("candidates")
        .and_then(Value::as_array)
        .map(|candidates| {
            candidates
                .iter()
                .filter_map(|candidate| candidate_readback(graph, candidate))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn candidate_readback(graph: &Value, candidate: &Value) -> Option<CandidateReadback> {
    Some(CandidateReadback {
        id: candidate.get("id")?.as_str()?.to_owned(),
        parent_id: candidate_parent_id(graph, candidate),
        artifact: candidate.get("artifact")?.clone(),
    })
}

fn candidate_parent_id(graph: &Value, candidate: &Value) -> Option<String> {
    candidate
        .get("origin")?
        .get("Proposal")?
        .get("proposal_id")
        .and_then(Value::as_str)
        .and_then(|proposal_id| proposal_parent_id(graph, proposal_id))
}

fn proposal_parent_id(graph: &Value, proposal_id: &str) -> Option<String> {
    graph
        .get("proposals")?
        .as_array()?
        .iter()
        .find(|proposal| proposal.get("id").and_then(Value::as_str) == Some(proposal_id))?
        .get("effect")?
        .get("Change")?
        .get("target")?
        .as_str()
        .map(str::to_owned)
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
    use leaven_engine::{
        GraphSnapshotRef, RunCheckpoint, StageJournalSnapshot, StateFormat,
        WorkspaceJournalSnapshot,
    };
    use leaven_kernel::{BudgetSnapshot, Fingerprint, now};
    use leaven_store::{BlobWrite, CheckpointBytes};

    use super::*;

    #[test]
    fn export_local_run_inspection_reads_latest_checkpoint_and_graph_blob() {
        let run_dir = test_run_dir("graph-readback");
        let store = FileStore::open(&run_dir).unwrap();
        let graph_json = br#"{
            "run_id":"run_export",
            "candidates":[
                {
                    "id":"cand_seed",
                    "identity":"seed",
                    "artifact":{"template":"seed"},
                    "origin":{"Seed":{"seed_index":0}},
                    "created_at":"2026-06-04T00:00:00Z"
                },
                {
                    "id":"cand_child",
                    "identity":"child",
                    "artifact":{"template":"child"},
                    "origin":{"Proposal":{"proposal_id":"prop_child","apply_attempt_id":"apply_child"}},
                    "created_at":"2026-06-04T00:00:01Z"
                }
            ],
            "proposal_batches":[{}],
            "proposals":[{"id":"prop_child","effect":{"Change":{"target":"cand_seed","change":{}}}}],
            "apply_attempts":[{}],
            "evaluation_requests":[{}],
            "assessments":[{},{}],
            "events":[
                {},
                {"OptimizationEnded":{"run_id":"run_export","best":"cand_child","budget":{}}},
                {}
            ]
        }"#;
        let graph_blob = BlobStore::put(
            &store,
            BlobWrite {
                bytes: Bytes::from_static(graph_json),
                content_type: Some("application/json".to_owned()),
            },
        )
        .unwrap();
        let artifact_ref = BlobRef {
            store: "file".to_owned(),
            key: "artifact.blob".to_owned(),
        };
        let evidence_ref = EvidenceRef {
            store: "evidence".to_owned(),
            key: "evidence.json".to_owned(),
        };
        let stage_ref = BlobRef {
            store: "file".to_owned(),
            key: "stage-journal.blob".to_owned(),
        };
        let workspace_ref = BlobRef {
            store: "file".to_owned(),
            key: "workspace-journal.blob".to_owned(),
        };
        let mut checkpoint = RunCheckpoint::new(
            leaven_kernel::RunId::new(),
            now(),
            GraphSnapshotRef {
                schema: Fingerprint::from_bytes([7; 32]),
                format: StateFormat::Json,
                bytes: graph_blob.clone(),
            },
            BudgetSnapshot::default(),
        );
        checkpoint.artifact_refs.push(artifact_ref.clone());
        checkpoint.evidence_refs.push(evidence_ref.clone());
        checkpoint.stage_journal = StageJournalSnapshot {
            entries: vec![stage_ref.clone()],
        };
        checkpoint.workspace_journal = WorkspaceJournalSnapshot {
            entries: vec![workspace_ref.clone()],
        };
        let checkpoint_bytes = serde_json::to_vec(&checkpoint).unwrap();
        let checkpoint_id =
            CheckpointStore::put(&store, CheckpointBytes(Bytes::from(checkpoint_bytes))).unwrap();
        CheckpointStore::mark_latest(&store, checkpoint_id).unwrap();

        let export = export_local_run_inspection(&run_dir).unwrap();

        assert_eq!(export.schema_version, RUN_INSPECTION_EXPORT_SCHEMA);
        assert_eq!(export.latest_checkpoint, checkpoint_id);
        assert_eq!(export.run_id, checkpoint.run_id);
        assert_eq!(export.checkpoint.graph_snapshot.key, graph_blob.key);
        assert_eq!(export.checkpoint.artifact_refs[0].key, artifact_ref.key);
        assert_eq!(export.checkpoint.artifact_ref_count, 1);
        assert_eq!(export.checkpoint.evidence_refs[0].key, evidence_ref.key);
        assert_eq!(export.checkpoint.evidence_ref_count, 1);
        assert_eq!(export.checkpoint.stage_journal_refs[0].key, stage_ref.key);
        assert_eq!(export.checkpoint.stage_journal_ref_count, 1);
        assert_eq!(
            export.checkpoint.workspace_journal_refs[0].key,
            workspace_ref.key
        );
        assert_eq!(export.checkpoint.workspace_journal_ref_count, 1);
        assert_eq!(export.graph.bytes, graph_json.len());
        assert_eq!(export.graph.run_id.as_deref(), Some("run_export"));
        assert_eq!(
            export.graph.best_candidate_id.as_deref(),
            Some("cand_child")
        );
        assert_eq!(export.graph.candidates[0].id, "cand_seed");
        assert_eq!(export.graph.candidates[0].parent_id, None);
        assert_eq!(export.graph.candidates[0].artifact["template"], "seed");
        assert_eq!(export.graph.candidates[1].id, "cand_child");
        assert_eq!(
            export.graph.candidates[1].parent_id.as_deref(),
            Some("cand_seed")
        );
        assert_eq!(export.graph.candidates[1].artifact["template"], "child");
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

    #[test]
    fn export_local_run_blob_reads_bytes_through_blob_store() {
        let run_dir = test_run_dir("blob-readback");
        let store = FileStore::open(&run_dir).unwrap();
        let reference = BlobStore::put(
            &store,
            BlobWrite {
                bytes: Bytes::from_static(b"durable blob bytes\n"),
                content_type: Some("text/plain".to_owned()),
            },
        )
        .unwrap();

        let export =
            export_local_run_blob(&run_dir, reference.store.clone(), reference.key.clone())
                .unwrap();

        assert_eq!(export.schema_version, RUN_BLOB_EXPORT_SCHEMA);
        assert_eq!(export.blob.store, reference.store);
        assert_eq!(export.blob.key, reference.key);
        assert_eq!(export.bytes, "durable blob bytes\n".len());
        assert_eq!(
            export.sha256,
            "cab11e0c83798e18f101ec99395ac4ebbf38c1739abe06a70ec8264954bf0bd8"
        );
        assert_eq!(export.content_base64, "ZHVyYWJsZSBibG9iIGJ5dGVzCg==");

        std::fs::remove_dir_all(run_dir).unwrap();
    }

    fn test_run_dir(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "leaven-run-inspection-{label}-{}",
            uuid::Uuid::new_v4()
        ))
    }
}
