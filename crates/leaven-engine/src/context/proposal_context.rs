//! Proposer context.

use leaven_core::OptimizationProblem;
use leaven_kernel::BudgetSnapshot;

use crate::{BudgetHandle, MaterializeContext, ReadScope, RunGraphView};

pub struct ProposalContext<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
    budget: BudgetHandle<'a>,
    read_scope: ReadScope,
}

impl<'a, P: OptimizationProblem> ProposalContext<'a, P> {
    pub(crate) fn new(
        graph: RunGraphView<'a, P>,
        budget: BudgetHandle<'a>,
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
    pub fn read_scope(&self) -> &ReadScope {
        &self.read_scope
    }

    #[must_use]
    pub fn budget(&self) -> BudgetSnapshot {
        self.budget.snapshot()
    }

    pub fn budget_handle(&mut self) -> &mut BudgetHandle<'a> {
        &mut self.budget
    }

    #[must_use]
    pub fn materialize_context(&self) -> MaterializeContext<'a, P> {
        MaterializeContext::new(
            self.graph.clone(),
            self.budget.snapshot(),
            self.read_scope.clone(),
        )
    }
}
