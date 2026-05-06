//! Context surfaces: the only allowed graph-mutation paths.
//!
//! [`run_context::RunContext`] is the engine-owned root context. It
//! constructs scoped sub-contexts for proposers
//! ([`proposal_context::ProposalContext`]) and evaluators
//! ([`evaluation_context::EvaluationContext`]), each carrying a
//! trust-scoped graph view, a budget handle, and the evidence/render
//! capabilities the actor is allowed to use.
//!
//! The full implementations land alongside the engine. This module
//! currently declares the shapes so dependent surfaces compile.

pub mod evaluation_context;
pub mod proposal_context;
pub mod run_context;
pub mod trust;

pub use evaluation_context::EvaluationContext;
pub use proposal_context::ProposalContext;
pub use run_context::{Actor, RunContext};
pub use trust::{EvidenceVisibility, ReadScope, TrustPolicy};
