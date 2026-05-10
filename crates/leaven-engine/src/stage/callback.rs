//! Callback trait.

use leaven_core::OptimizationProblem;

use crate::{RunEvent, RunGraphView};

pub trait Callback<P: OptimizationProblem>: Send {
    fn on_event(&mut self, event: &RunEvent, graph: RunGraphView<'_, P>);
}

impl<P> Callback<P> for Box<dyn Callback<P>>
where
    P: OptimizationProblem,
{
    fn on_event(&mut self, event: &RunEvent, graph: RunGraphView<'_, P>) {
        self.as_mut().on_event(event, graph);
    }
}

pub trait DynCallback<P: OptimizationProblem>: Send {
    fn on_event_dyn(&mut self, event: &RunEvent, graph: RunGraphView<'_, P>);
}

impl<P, T> DynCallback<P> for T
where
    P: OptimizationProblem,
    T: Callback<P>,
{
    fn on_event_dyn(&mut self, event: &RunEvent, graph: RunGraphView<'_, P>) {
        self.on_event(event, graph);
    }
}
