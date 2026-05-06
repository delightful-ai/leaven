//! Error model.
//!
//! There are three layers, following the philosophy in
//! `docs/philosophy/error_design.md`:
//!
//! 1. Leaf error types defined in capability modules (`ApplyProposalError`,
//!    `EvaluationError`, …). They keep the typed story local.
//! 2. Capability error enums exposed at trait boundaries.
//! 3. [`ErrorRecord`] — the **graph-durable normalization**. Optimizers
//!    fail in many specific ways; the run graph stores a single
//!    structured shape so the durable record is searchable, queryable,
//!    and forward-compatible. Conversion happens at the
//!    [`crate::context::RunContext`] boundary.
//!
//! `ErrorRecord` is deliberately not `dyn Error`. Storing trait objects
//! is what lets stringly-typed metadata leak into truth.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::{
    ApplyAttemptId, AssessmentId, CandidateId, EvaluationRequestId, ProposalBatchId, ProposalId,
};
use crate::metadata::MetadataBag;

/// Coarse classification of a failed operation. Used to route policy
/// (retry? alert? stop the run?) and to surface error rates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum ErrorKind {
    /// The proposer failed to produce a batch.
    Proposal,
    /// Applying a proposal to a target candidate failed.
    Apply,
    /// `Artifact::apply` itself errored, or validation rejected the
    /// new artifact.
    Artifact,
    /// An evaluator failed.
    Evaluation,
    /// A renderer failed (e.g. could not write the workspace).
    Render,
    /// A budget cap was exceeded.
    Budget,
    /// A trust policy refused the operation.
    Trust,
    /// The evaluation cache rejected an operation.
    Cache,
    /// An evidence store, run store, or other persistence failed.
    Store,
    /// A user-supplied callback failed.
    Callback,
    /// A graph invariant was violated; the run should usually stop.
    GraphInvariant,
    /// Internal bug or unreachable state.
    Internal,
}

/// Graph-durable normalization of any error.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub kind: ErrorKind,
    pub message: String,
    pub debug: Option<String>,
    /// Each entry is the rendered form of one link in the source chain
    /// (outermost first). We store rendered strings because trait
    /// objects are not durable.
    pub source_chain: Vec<String>,
    /// `None` = unknown / unspecified.
    pub retryable: Option<bool>,
    pub metadata: MetadataBag,
}

impl ErrorRecord {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            debug: None,
            source_chain: Vec::new(),
            retryable: None,
            metadata: MetadataBag::default(),
        }
    }

    /// Build an `ErrorRecord` from any `std::error::Error` value. The
    /// source chain is captured up to a small depth.
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
            retryable: None,
            metadata: MetadataBag::default(),
        }
    }
}

/// Implementors can be normalized into the durable record shape.
pub trait IntoErrorRecord {
    fn into_error_record(self) -> ErrorRecord;
}

impl IntoErrorRecord for ErrorRecord {
    fn into_error_record(self) -> ErrorRecord {
        self
    }
}

/// Errors raised while applying a proposal to the run graph. These are
/// the typed shape that [`crate::context::RunContext::apply_proposal`]
/// surfaces; the graph itself stores the [`ErrorRecord`] form.
#[derive(Debug, Error)]
pub enum ApplyProposalError {
    #[error("unknown candidate: {0}")]
    UnknownCandidate(CandidateId),

    #[error("unknown proposal: {0}")]
    UnknownProposal(ProposalId),

    #[error("proposal already applied: {0}")]
    ProposalAlreadyApplied(ProposalId),

    #[error("invalid proposal provenance: {0}")]
    InvalidProvenance(String),

    #[error("artifact apply failed: {message}")]
    Artifact {
        message: String,
        record: ErrorRecord,
    },

    #[error("artifact validation failed: {message}")]
    Validation {
        message: String,
        record: ErrorRecord,
    },

    #[error("graph invariant violation: {message}")]
    GraphInvariant {
        message: String,
        record: ErrorRecord,
    },
}

impl IntoErrorRecord for ApplyProposalError {
    fn into_error_record(self) -> ErrorRecord {
        match self {
            Self::UnknownCandidate(_)
            | Self::UnknownProposal(_)
            | Self::ProposalAlreadyApplied(_)
            | Self::InvalidProvenance(_) => {
                ErrorRecord::new(ErrorKind::GraphInvariant, self.to_string())
            }
            Self::Artifact { record, .. }
            | Self::Validation { record, .. }
            | Self::GraphInvariant { record, .. } => record,
        }
    }
}

/// Errors at the run-graph layer that are not specific to one
/// operation: missing IDs, duplicate inserts, invariant breaches.
#[derive(Debug, Error)]
pub enum GraphError {
    #[error("candidate does not exist: {0}")]
    MissingCandidate(CandidateId),

    #[error("proposal does not exist: {0}")]
    MissingProposal(ProposalId),

    #[error("proposal batch does not exist: {0}")]
    MissingProposalBatch(ProposalBatchId),

    #[error("apply attempt does not exist: {0}")]
    MissingApplyAttempt(ApplyAttemptId),

    #[error("evaluation request does not exist: {0}")]
    MissingEvaluationRequest(EvaluationRequestId),

    #[error("assessment does not exist: {0}")]
    MissingAssessment(AssessmentId),

    #[error("duplicate id inserted: {0}")]
    DuplicateId(String),

    #[error("graph invariant violation: {0}")]
    Invariant(String),
}

impl IntoErrorRecord for GraphError {
    fn into_error_record(self) -> ErrorRecord {
        ErrorRecord::new(ErrorKind::GraphInvariant, self.to_string())
    }
}
