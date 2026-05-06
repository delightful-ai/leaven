//! Preference relation trait.

use leaven_core::{OptimizationProblem, Preference};
use leaven_kernel::CandidateId;

use crate::RunGraphView;

/// Static preference relation over two candidates in a run graph.
pub trait PreferenceRelation<P: OptimizationProblem>: Send + Sync {
    /// Compare two candidates using the graph evidence visible to the caller.
    fn prefer(
        &self,
        left: CandidateId,
        right: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Preference;
}

/// Object-safe preference relation for heterogeneous preference registries.
pub trait DynPreferenceRelation<P: OptimizationProblem>: Send + Sync {
    /// Compare two candidates through the object-safe relation.
    fn prefer_dyn(
        &self,
        left: CandidateId,
        right: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Preference;
}

impl<P, T> DynPreferenceRelation<P> for T
where
    P: OptimizationProblem,
    T: PreferenceRelation<P>,
{
    fn prefer_dyn(
        &self,
        left: CandidateId,
        right: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Preference {
        self.prefer(left, right, graph)
    }
}
