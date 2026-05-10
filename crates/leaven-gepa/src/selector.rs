//! Candidate selection strategies for GEPA.

use leaven_core::OptimizationProblem;
use leaven_engine::RunGraphView;
use leaven_kernel::CandidateId;
use leaven_population::{KeepBest, ParetoFrontier, TournamentPopulation};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Population capability needed by simple GEPA selectors.
pub trait HasBestCandidate {
    /// Return the population's current best candidate.
    fn best_candidate(&self) -> Option<CandidateId>;
}

impl HasBestCandidate for KeepBest {
    fn best_candidate(&self) -> Option<CandidateId> {
        self.best()
    }
}

impl HasBestCandidate for ParetoFrontier {
    fn best_candidate(&self) -> Option<CandidateId> {
        self.best()
    }
}

impl HasBestCandidate for TournamentPopulation {
    fn best_candidate(&self) -> Option<CandidateId> {
        self.best()
    }
}

/// Chooses which candidate GEPA should mutate next.
pub trait CandidateSelector<P: OptimizationProblem, Pop> {
    /// Selection result shape.
    type Selection;

    /// Select from population state and a read-only graph view.
    fn select(&mut self, population: &Pop, graph: RunGraphView<'_, P>) -> Self::Selection;
}

/// Private selector state that must survive GEPA checkpoint/restore.
pub trait CheckpointCandidateSelector {
    /// Serializable state shape.
    type State: Serialize + DeserializeOwned;

    /// Capture selector state.
    fn checkpoint_state(&self) -> Self::State;

    /// Restore selector state.
    fn restore_state(&mut self, state: Self::State);
}

/// Deterministic selector that returns the population's current best candidate.
#[derive(Clone, Debug, Default)]
pub struct SelectBestCandidate;

impl CheckpointCandidateSelector for SelectBestCandidate {
    type State = ();

    fn checkpoint_state(&self) -> Self::State {}

    fn restore_state(&mut self, _state: Self::State) {}
}

impl<P, Pop> CandidateSelector<P, Pop> for SelectBestCandidate
where
    P: OptimizationProblem,
    Pop: HasBestCandidate,
{
    type Selection = Option<CandidateId>;

    fn select(&mut self, population: &Pop, _graph: RunGraphView<'_, P>) -> Self::Selection {
        population.best_candidate()
    }
}

/// Pareto-frequency selector.
///
/// The current deterministic implementation selects the best candidate exposed
/// by the population. Future stochastic weighting belongs here, not in the
/// engine.
#[derive(Clone, Debug, Default)]
pub struct ParetoFrequencyWeighted;

impl CheckpointCandidateSelector for ParetoFrequencyWeighted {
    type State = ();

    fn checkpoint_state(&self) -> Self::State {}

    fn restore_state(&mut self, _state: Self::State) {}
}

impl<P, Pop> CandidateSelector<P, Pop> for ParetoFrequencyWeighted
where
    P: OptimizationProblem,
    Pop: HasBestCandidate,
{
    type Selection = Option<CandidateId>;

    fn select(&mut self, population: &Pop, _graph: RunGraphView<'_, P>) -> Self::Selection {
        population.best_candidate()
    }
}
