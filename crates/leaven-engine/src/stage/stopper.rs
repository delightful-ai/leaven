//! Stopper trait.

use leaven_core::OptimizationProblem;

use crate::RunGraphView;

/// Static run-stopping predicate.
pub trait Stopper<P: OptimizationProblem>: Send + Sync {
    /// Decide whether the run should stop based on a read-only graph view.
    fn should_stop(&self, graph: RunGraphView<'_, P>) -> bool;
}

/// Object-safe stopper for heterogeneous stopper lists.
pub trait DynStopper<P: OptimizationProblem>: Send + Sync {
    /// Dispatch the stopping predicate through an object-safe trait.
    fn should_stop_dyn(&self, graph: RunGraphView<'_, P>) -> bool;
}

impl<P, T> DynStopper<P> for T
where
    P: OptimizationProblem,
    T: Stopper<P>,
{
    fn should_stop_dyn(&self, graph: RunGraphView<'_, P>) -> bool {
        self.should_stop(graph)
    }
}
