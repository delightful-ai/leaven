//! Evidence — the opaque payload an evaluator returns.
//!
//! Evidence is whatever the run wants it to be: scalar scores, pairwise
//! judgments, agent trajectories, structured rubrics, calibration logs.
//! The cold core does not interpret it. Preference relations,
//! populations, and renderers do.
//!
//! Evidence is stored externally via [`EvidenceStore`] and the run
//! graph keeps only [`EvidenceRef`]s — large evidence blobs do not
//! inflate the durable graph.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Marker trait for run-wide evidence types. Implementors are normally
/// problem-specific enums. Bound for thread-safety only.
pub trait Evidence: Send + Sync + 'static {}

/// Reference into a configured evidence store.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub store: String,
    pub key: String,
}

/// Storage capability for run evidence. The default implementation in
/// `leaven-engine` is in-memory; persistent backends live there too.
pub trait EvidenceStore<E: Evidence>: Send + Sync {
    /// Persist evidence and return a reference.
    fn put(&self, evidence: E) -> Result<EvidenceRef, StoreError>;

    /// Reload evidence by reference. Required to be a value-returning
    /// API: stores may rehydrate from disk, network, etc.
    fn get(&self, reference: &EvidenceRef) -> Result<E, StoreError>;
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("evidence store `{store}` is unavailable: {reason}")]
    Unavailable { store: String, reason: String },

    #[error("evidence reference {0:?} was not found")]
    NotFound(EvidenceRef),

    #[error("evidence serialization failed: {0}")]
    Serialization(String),

    #[error("evidence store error: {0}")]
    Other(String),
}
