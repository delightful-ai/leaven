//! Materializer context.

use leaven_core::OptimizationProblem;
use leaven_kernel::BudgetSnapshot;

use crate::{ReadScope, RunGraphView};

pub struct MaterializeContext<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
    budget: BudgetSnapshot,
    read_scope: ReadScope,
}

impl<P: OptimizationProblem> Clone for MaterializeContext<'_, P> {
    fn clone(&self) -> Self {
        Self {
            graph: self.graph.clone(),
            budget: self.budget.clone(),
            read_scope: self.read_scope.clone(),
        }
    }
}

impl<'a, P: OptimizationProblem> MaterializeContext<'a, P> {
    pub(crate) const fn new(
        graph: RunGraphView<'a, P>,
        budget: BudgetSnapshot,
        read_scope: ReadScope,
    ) -> Self {
        Self {
            graph,
            budget,
            read_scope,
        }
    }

    #[must_use]
    pub fn graph(&self) -> &RunGraphView<'a, P> {
        &self.graph
    }

    #[must_use]
    pub const fn budget(&self) -> &BudgetSnapshot {
        &self.budget
    }

    #[must_use]
    pub const fn read_scope(&self) -> &ReadScope {
        &self.read_scope
    }
}
