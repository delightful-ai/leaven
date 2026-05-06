//! Store errors.

use leaven_kernel::{BlobRef, EvidenceRef};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("store `{store}` is unavailable: {reason}")]
    Unavailable { store: String, reason: String },
    #[error("blob reference was not found: {0:?}")]
    BlobNotFound(BlobRef),
    #[error("evidence reference was not found: {0:?}")]
    EvidenceNotFound(EvidenceRef),
    #[error("serialization failed: {0}")]
    Serialization(String),
    #[error("store `{store}` refused {operation}: {reason}")]
    OperationFailed {
        store: String,
        operation: &'static str,
        reason: String,
        retryable: Option<bool>,
    },
}
