//! Universal mechanical primitives for Leaven.
//!
//! `leaven-kernel` sits below the optimization algebra. It defines the
//! identifiers, cost-accounting types, durable error records, fingerprints,
//! metadata bags, and timestamps that every other Leaven crate composes with.
//! Nothing in here knows what an [`Artifact`], [`Proposal`], or evaluator is —
//! that vocabulary lives in `leaven-core`.
//!
//! # Why split this out
//!
//! Optimizer algebra and run mechanics evolve at different rates. Algebra is
//! load-bearing for users writing optimizers; mechanics are load-bearing for
//! the engine, store, workspace, and any tooling that has to talk about
//! identity, cost, or durable errors *without* importing the algebra. Keeping
//! the kernel free of optimizer concepts lets storage backends, workspace
//! adapters, and CI/observability tools depend on it without pulling in the
//! cold-core algebra.
//!
//! # What lives here
//!
//! - [`ids`] — typed identifiers ([`CandidateId`], [`ProposalId`],
//!   [`ContentId`], [`StageId`], etc.). Identity is mechanical, not domain.
//! - [`cost`] — [`Cost`], [`Amount`], [`Budget`], [`Metered`]. Stage-level
//!   spending and the bookkeeping that surrounds it.
//! - [`error`] — [`ErrorRecord`]: the durable, serializable shadow of an
//!   `std::error::Error` that can land in a run graph and be read back later.
//! - [`fingerprint`] — content/behavior fingerprints used for cache keys and
//!   resolution stability.
//! - [`metadata`] — operational metadata bags. Always non-semantic; semantic
//!   payloads belong in typed annotations in `leaven-core`.
//! - [`time`] — UTC [`Timestamp`]s for graph entries.
//!
//! [`Artifact`]: https://docs.rs/leaven-core/latest/leaven_core/trait.Artifact.html
//! [`Proposal`]: https://docs.rs/leaven-core/latest/leaven_core/struct.Proposal.html

pub mod cost;
pub mod error;
pub mod fingerprint;
pub mod finite;
pub mod ids;
pub mod metadata;
pub mod stage;
pub mod time;

pub use cost::{
    Amount, AmountError, Budget, BudgetDimension, BudgetExceeded, BudgetSnapshot, Cost, CostAxis,
    CostUnit, Metered,
};
pub use error::{ErrorKind, ErrorRecord, IntoErrorRecord, Retryability};
pub use fingerprint::{Fingerprint, FingerprintBuilder};
pub use finite::{FiniteF64, FiniteF64Error};
pub use ids::{
    AgentId, AgentRuntimeId, AgentSessionId, ApplyAttemptId, AssessmentId, BlobRef, CandidateId,
    CaseId, CaseRunId, CheckpointId, ContentId, EvaluationRequestId, EvaluationSetId, EvaluatorId,
    EvidenceRef, IterationId, PopulationId, ProposalBatchId, ProposalId, ProposerId, RenderId,
    RendererId, ResolvedEvaluationSetId, RunId, StageAttemptReceiptId, StageCallId, StageId,
    StageQueryId, StopperId, TraceRef, WorkspaceEntryId, WorkspaceId,
};
pub use metadata::{MetadataBag, MetadataKey, MetadataValue};
pub use stage::{
    StageAttemptFailure, StageAttemptOutcome, StageAttemptReceiptRef, StageRole, StageRoleError,
};
pub use time::{Timestamp, now};

pub mod prelude {
    //! Common kernel imports.
    //!
    //! Pull in this prelude when you need the bulk of the mechanical
    //! vocabulary — the IDs plus the cost/error/fingerprint/metadata
    //! primitives — without enumerating each item.

    pub use crate::ids::*;
    pub use crate::{
        Amount, AmountError, Budget, BudgetExceeded, BudgetSnapshot, Cost, CostUnit, ErrorKind,
        ErrorRecord, Fingerprint, FiniteF64, FiniteF64Error, MetadataBag, Metered,
        StageAttemptFailure, StageAttemptOutcome, StageAttemptReceiptRef, StageRole,
    };
}
