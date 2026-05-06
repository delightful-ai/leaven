//! `EvaluationContext` — what an `Evaluator` sees while running.
//!
//! Stub: full implementation lands with the engine.

use std::marker::PhantomData;

use crate::problem::OptimizationProblem;

pub struct EvaluationContext<'e, P: OptimizationProblem> {
    pub(crate) _marker: PhantomData<&'e P>,
}
