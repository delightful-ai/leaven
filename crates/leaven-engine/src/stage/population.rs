//! Population stage traits.

use leaven_core::OptimizationProblem;
use leaven_kernel::{AssessmentId, CandidateId, FiniteF64, PopulationId};

use crate::{Arity, RunGraphView};

pub trait Population<P: OptimizationProblem>: Send {
    fn id(&self) -> PopulationId;

    fn insert_seed(
        &mut self,
        candidate: CandidateId,
        graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent>;

    fn select_candidates(&mut self, arity: Arity, graph: RunGraphView<'_, P>) -> Vec<CandidateId>;

    fn observe_candidate(
        &mut self,
        _candidate: CandidateId,
        _graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        Vec::new()
    }

    fn observe_assessment(
        &mut self,
        _assessment: AssessmentId,
        _graph: RunGraphView<'_, P>,
    ) -> Vec<PopulationEvent> {
        Vec::new()
    }

    fn best(&self, graph: RunGraphView<'_, P>) -> Option<CandidateId>;

    fn view(&self) -> PopulationView<'_>;
}

pub struct PopulationView<'a> {
    _private: std::marker::PhantomData<&'a ()>,
}

#[derive(Clone, Debug)]
pub enum PopulationEvent {
    Inserted {
        population: PopulationId,
        candidate: CandidateId,
        reason: String,
    },
    Replaced {
        population: PopulationId,
        old: CandidateId,
        new: CandidateId,
        reason: String,
    },
    Removed {
        population: PopulationId,
        candidate: CandidateId,
        reason: String,
    },
    Ignored {
        population: PopulationId,
        candidate: CandidateId,
        reason: String,
    },
    Reweighted {
        population: PopulationId,
        candidate: CandidateId,
        weight: FiniteF64,
        reason: String,
    },
}
