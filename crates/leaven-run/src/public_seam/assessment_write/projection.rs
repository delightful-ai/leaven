use leaven_store::StoreError;
use serde_json::{Value, json};
use thiserror::Error;

mod output;
mod refs;

pub(super) use output::{assessment_plan_entry, project_assessment_evidence_rows};
pub(super) use refs::{evaluation_request_ref, sorted_assessment_refs};

/// Errors raised while projecting `RunContext` assessment writes into V1 receipts.
#[derive(Debug, Error)]
pub enum PublicAssessmentWriteReceiptProjectionError {
    /// The context did not include receipt timing.
    #[error("assessment write projection requires receipt timing")]
    MissingTiming,
    /// The evaluation request from the report is not visible in the graph.
    #[error("assessment write projection requires a graph-visible evaluation request")]
    RequestNotInGraph,
    /// The report did not include any assessment ids.
    #[error("assessment write projection requires at least one assessment")]
    EmptyAssessmentBatch,
    /// A reported assessment is not visible in the graph.
    #[error("assessment write projection requires graph-visible assessments")]
    AssessmentNotInGraph,
    /// A reported assessment belongs to a different evaluation request.
    #[error("assessment write projection assessment request mismatch")]
    AssessmentRequestMismatch,
    /// Stored case-assessment evidence could not be loaded.
    #[error("assessment write projection could not load assessment evidence")]
    EvidenceLoad {
        /// Store-layer failure while loading the evidence payload.
        #[source]
        source: StoreError,
    },
    /// Stored case-assessment evidence did not carry source read receipts.
    #[error("assessment write projection requires real evidence source read receipts")]
    MissingEvidenceSourceReceipts,
    /// The projection does not yet support the assessment shape.
    #[error("assessment write projection does not support this assessment shape")]
    UnsupportedAssessmentShape,
    /// The projection does not yet support this output record shape.
    #[error(
        "assessment write projection requires candidate/artifact inline output or audited blob output"
    )]
    UnsupportedScoreOutput,
    /// JCS/SHA-256 fingerprint computation failed.
    #[error("assessment write fingerprinting failed: {message}")]
    Fingerprint {
        /// Human-readable fingerprinting error.
        message: String,
    },
}

pub(super) fn prefixed_jcs(
    prefix: &str,
    value: &Value,
) -> Result<String, PublicAssessmentWriteReceiptProjectionError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value).map_err(|error| {
        PublicAssessmentWriteReceiptProjectionError::Fingerprint {
            message: error.to_string(),
        }
    })?;
    Ok(format!("{prefix}{digest}"))
}

pub(super) fn plan_write_result_hash(
    name: &str,
    value: &Value,
) -> Result<String, PublicAssessmentWriteReceiptProjectionError> {
    prefixed_jcs(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_result.v1",
            "name": name,
            "value": value
        }),
    )
}
