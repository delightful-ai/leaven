//! Internal plumbing: public only so sibling crates and contract tests can
//! reach it.
//!
//! Nothing here is part of the supported import experience. External callers
//! must never depend on this module: it carries no stability promise and may
//! change with any release. The names live here because Rust has no
//! "visible to the workspace" visibility, not because they are product API.
//!
//! Every entry names the concrete consumer that forces it to be public. The
//! `public_surface_contract` test enforces that justification.

// --- Content addressing and identity internals. ---

/// Engine cache keys are built from `CacheIdentity`; reached by cache internals.
pub use leaven_core::CacheIdentity;
/// Derive-macro output and content-store internals call `ContentAddressed`.
pub use leaven_core::ContentAddressed;
/// Content-store and fingerprint internals key blobs by `ContentId`.
pub use leaven_kernel::ContentId;
/// Surface and engine fingerprint plumbing compares `Fingerprint` values.
pub use leaven_kernel::Fingerprint;

// --- Finite-number and budget internals. ---

/// Cost and score arithmetic internals construct `Amount` values.
pub use leaven_kernel::Amount;
/// Cost arithmetic internals surface `AmountError` on overflow.
pub use leaven_kernel::AmountError;
/// Engine budget-ledger internals snapshot remaining budget as `BudgetSnapshot`.
pub use leaven_kernel::BudgetSnapshot;
/// Score and cost internals construct checked `FiniteF64` values.
pub use leaven_kernel::FiniteF64;
/// `FiniteF64` construction internals surface `FiniteF64Error` on non-finite input.
pub use leaven_kernel::FiniteF64Error;

// --- Error, metadata, and id record internals. ---

/// Durable error plumbing serializes failures as `ErrorRecord`.
pub use leaven_kernel::ErrorRecord;
/// Graph and evidence record plumbing carries arbitrary `MetadataBag` metadata.
pub use leaven_kernel::MetadataBag;
/// Engine graph plumbing identifies proposal nodes with `ProposalId`.
pub use leaven_kernel::ProposalId;
