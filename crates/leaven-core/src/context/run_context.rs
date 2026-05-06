//! `RunContext` — the only public mutation path into a `RunGraph`.
//!
//! The full surface (propose / apply / evaluate / charge / emit) lands
//! alongside the engine. This module currently declares the type so
//! the rest of the crate (and downstream crates) can reference it.

use std::marker::PhantomData;

use crate::ids::{EvaluatorId, IterationId, ProposerId, RendererId};
use crate::problem::OptimizationProblem;

#[allow(dead_code)] // fields populated when the engine lands; stub today.
pub struct RunContext<'e, P: OptimizationProblem> {
    pub(crate) iteration: Option<IterationId>,
    pub(crate) actor: Actor,
    pub(crate) _marker: PhantomData<&'e P>,
}

/// Who is currently driving the run loop.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Actor {
    Optimizer,
    Proposer(ProposerId),
    Evaluator(EvaluatorId),
    Renderer(RendererId),
    Callback,
}
