//! `ProposalContext` — what a `Proposer` sees while running.
//!
//! Carries a trust-scoped [`crate::graph::view::RunGraphView`], a
//! [`BudgetHandle`] tagged with the proposer's stage, and the
//! evidence / render handles the trust policy permits.
//!
//! Full implementation lands with the engine. Stub here.

use std::marker::PhantomData;

use crate::problem::OptimizationProblem;

pub struct ProposalContext<'e, P: OptimizationProblem> {
    pub(crate) _marker: PhantomData<&'e P>,
}
