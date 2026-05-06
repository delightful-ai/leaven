//! `Proposer` — produces typed [`crate::proposal::ProposalBatch`]es.
//!
//! Stub trait surface: each proposer carries an associated `Request`
//! type so reflective mutation, merge, MIPRO acquisition, etc. all use
//! request shapes that match their needs. The full async signature
//! lands with the engine; this module currently declares the marker
//! and the [`Arity`] hint.

use crate::ids::ProposerId;

/// What kind of input the optimizer should provide to this proposer.
///
/// Used when the optimizer drives parent selection. **Hint**, not law:
/// a proposer may emit fewer or more proposals than `Arity` suggests,
/// and may set causal inputs differently per-proposal.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Arity {
    /// Authored from scratch; no causal predecessor required.
    None,
    Single,
    Pair,
    /// Variable; the proposer accepts a list.
    Variadic,
}

/// Marker trait until the full async surface lands.
pub trait Proposer: Send + Sync {
    fn id(&self) -> ProposerId;
    fn arity(&self) -> Arity {
        Arity::Single
    }
}
