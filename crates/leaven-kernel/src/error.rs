//! Durable error records for the run graph.
//!
//! The Rust ecosystem's standard error story (`std::error::Error` and
//! source chains) is borrow-shaped and lifetime-tangled — fine for in-process
//! recovery, wrong for persistence. Once a run has finished, anything still
//! holding a `Box<dyn Error>` is unprintable and unsuited to telemetry.
//!
//! [`ErrorRecord`] is the durable, serializable shadow that lands in the run
//! graph, telemetry, checkpoints, and stop reasons. It captures:
//!
//! - the message at the point of failure,
//! - the source chain as flat strings (so cycles and lifetime restrictions
//!   stop mattering),
//! - a coarse [`ErrorKind`] for indexing,
//! - a [`Retryability`] hint so consumers know whether re-running is meaningful,
//! - a [`MetadataBag`] for stage-specific structured context.
//!
//! Stages map their internal errors into [`ErrorRecord`] via
//! [`IntoErrorRecord`] before the record crosses the engine boundary.

use serde::{Deserialize, Serialize};

use crate::MetadataBag;

/// Coarse stage or subsystem classification for durable run errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum ErrorKind {
    /// A proposer refused to produce a proposal batch.
    Proposal,
    /// Proposal application refused to mutate the graph.
    Apply,
    /// Artifact validation or change application failed.
    Artifact,
    /// An evaluator refused an evaluation request.
    Evaluation,
    /// The optimizer loop or optimizer stage failed.
    Optimizer,
    /// Rendering failed.
    Render,
    /// Budget charging was refused.
    Budget,
    /// Trust or visibility policy refused a read/request.
    Trust,
    /// Evaluation-cache behavior failed.
    Cache,
    /// Storage failed.
    Store,
    /// Callback dispatch failed.
    Callback,
    /// A graph invariant was violated or protected from violation.
    GraphInvariant,
    /// Internal failure that has not been mapped to a narrower kind.
    Internal,
}

/// Retry classification for durable run errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Retryability {
    /// Retrying the same operation may succeed.
    Retryable,
    /// Retrying the same operation should not be expected to succeed.
    NotRetryable,
    /// Retry behavior is not known at this boundary.
    Unknown,
}

/// Durable, serializable error record stored in the run graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorRecord {
    /// Coarse error kind used by run-level consumers.
    pub kind: ErrorKind,
    /// Human-readable root message.
    pub message: String,
    /// Debug representation of the root error when one was preserved.
    pub debug: Option<String>,
    /// Human-readable source chain, outer source first.
    pub source_chain: Vec<String>,
    /// Whether retrying is meaningful.
    pub retryability: Retryability,
    /// Additional structured context.
    pub metadata: MetadataBag,
}

impl ErrorRecord {
    /// Constructs a fresh record from a kind and a message.
    ///
    /// Use this when the failure is a plain string and there's no
    /// source-chain to walk. `debug` defaults to `None`, `source_chain`
    /// to empty, `retryability` to [`Retryability::Unknown`], and
    /// `metadata` to empty — fill any of those in via direct field
    /// assignment if needed.
    #[must_use]
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            debug: None,
            source_chain: Vec::new(),
            retryability: Retryability::Unknown,
            metadata: MetadataBag::default(),
        }
    }

    /// Constructs a record from an [`std::error::Error`], capturing the
    /// `Display`, `Debug`, and source-chain projections.
    ///
    /// The source chain is walked iteratively and capped at 16 entries to
    /// guard against pathological cyclic chains. Capping silently rather
    /// than erroring is deliberate — the record is a *report*, not a
    /// load-bearing value, and a truncated chain beats a panic at the
    /// observability boundary.
    #[must_use]
    pub fn from_error<E: std::error::Error + ?Sized>(kind: ErrorKind, err: &E) -> Self {
        let mut chain = Vec::new();
        let mut current: Option<&dyn std::error::Error> = err.source();
        while let Some(src) = current {
            chain.push(src.to_string());
            current = src.source();
            if chain.len() > 16 {
                break;
            }
        }
        Self {
            kind,
            message: err.to_string(),
            debug: Some(format!("{err:?}")),
            source_chain: chain,
            retryability: Retryability::Unknown,
            metadata: MetadataBag::default(),
        }
    }
}

/// Conversion into a durable [`ErrorRecord`].
///
/// Stage-internal error types implement this so the engine can record
/// failures uniformly without knowing each stage's error shape. The
/// blanket impl for `ErrorRecord` itself is the identity case so callers
/// that already produce records can pass them through unchanged.
pub trait IntoErrorRecord {
    /// Converts `self` into a durable [`ErrorRecord`].
    fn into_error_record(self) -> ErrorRecord;
}

impl IntoErrorRecord for ErrorRecord {
    fn into_error_record(self) -> ErrorRecord {
        self
    }
}
