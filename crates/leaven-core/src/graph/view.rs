//! Read-only, trust-scoped view of the run graph.
//!
//! Strategy authors receive `RunGraphView`s from
//! [`crate::context::RunContext::graph`]. The view honours
//! [`crate::context::trust::ReadScope`]: hidden partitions and
//! evidence visibility filters apply transparently.
//!
//! Implementation is deferred to a later pass; this module currently
//! only declares the type so dependent surfaces can compile.

use std::marker::PhantomData;

use crate::context::trust::ReadScope;
use crate::problem::OptimizationProblem;

use super::storage::RunGraph;

#[allow(dead_code)] // graph field exercised once view methods land.
pub struct RunGraphView<'g, P: OptimizationProblem> {
    pub(crate) graph: &'g RunGraph<P>,
    pub(crate) read_scope: ReadScope,
    pub(crate) _marker: PhantomData<&'g ()>,
}

impl<'g, P: OptimizationProblem> RunGraphView<'g, P> {
    #[must_use]
    pub fn new(graph: &'g RunGraph<P>, read_scope: ReadScope) -> Self {
        Self {
            graph,
            read_scope,
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn read_scope(&self) -> &ReadScope {
        &self.read_scope
    }
}
