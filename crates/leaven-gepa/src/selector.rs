//! Candidate selection strategies for GEPA.

use leaven_core::OptimizationProblem;
use leaven_engine::RunGraphView;
use leaven_kernel::CandidateId;
use leaven_population::{KeepBest, ParetoFrontier, TournamentPopulation};

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

/// Deterministic selector that returns the population's current best candidate.
#[derive(Clone, Debug, Default)]
pub struct SelectBestCandidate;

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
