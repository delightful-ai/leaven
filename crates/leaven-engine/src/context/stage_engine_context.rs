//! Engine-owned context for optimizer-stage agent workspaces.

use leaven_core::OptimizationProblem;
use leaven_kernel::{BudgetSnapshot, StageCallId};

use crate::{ReadScope, ScopedRunGraphView};

/// Read and accounting context handed from a proposer call to the stage layer.
///
/// The context carries a scoped graph wrapper instead of `RunGraphView` so the
/// stage layer cannot reach unscoped graph APIs by convenience.
pub struct StageEngineContext<'a, P: OptimizationProblem> {
    graph: ScopedRunGraphView<'a, P>,
    read_scope: ReadScope,
    budget: BudgetSnapshot,
    stage_call_id: StageCallId,
}

impl<P: OptimizationProblem> Clone for StageEngineContext<'_, P> {
    fn clone(&self) -> Self {
        Self {
            graph: self.graph.clone(),
            read_scope: self.read_scope.clone(),
            budget: self.budget.clone(),
            stage_call_id: self.stage_call_id,
        }
    }
}

impl<'a, P: OptimizationProblem> StageEngineContext<'a, P> {
    #[must_use]
    pub(crate) fn new(
        graph: ScopedRunGraphView<'a, P>,
        read_scope: ReadScope,
        budget: BudgetSnapshot,
        stage_call_id: StageCallId,
    ) -> Self {
        Self {
            graph,
            read_scope,
            budget,
            stage_call_id,
        }
    }

    #[must_use]
    pub const fn graph(&self) -> &ScopedRunGraphView<'a, P> {
        &self.graph
    }

    #[must_use]
    pub const fn read_scope(&self) -> &ReadScope {
        &self.read_scope
    }

    #[must_use]
    pub const fn budget(&self) -> &BudgetSnapshot {
        &self.budget
    }

    #[must_use]
    pub const fn stage_call_id(&self) -> StageCallId {
        self.stage_call_id
    }
}
