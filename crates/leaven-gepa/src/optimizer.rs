//! GEPA optimizer state.

use std::marker::PhantomData;

use leaven_core::OptimizationProblem;
use leaven_engine::RunGraphView;
use leaven_kernel::CandidateId;
use leaven_surface::{EditSurface, SurfaceError};

use crate::{
    CandidateSelector, Gate, ParetoFrequencyWeighted, PartSelector, RoundRobinPart,
    StrictImprovement,
};

/// GEPA state over an explicit edit surface.
///
/// Strategy slots are statically typed with defaults. Users can swap candidate
/// selection, part selection, and gate policy without changing the engine or
/// the artifact trait.
#[derive(Clone, Debug)]
pub struct Gepa<
    P,
    S,
    Pop,
    CandSel = ParetoFrequencyWeighted,
    PartSel = RoundRobinPart,
    GatePol = StrictImprovement,
> where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    surface: S,
    population: Pop,
    candidate_selector: CandSel,
    part_selector: PartSel,
    gate: GatePol,
    _problem: PhantomData<P>,
}

impl<P, S, Pop> Gepa<P, S, Pop>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    /// Build GEPA with standard deterministic strategies.
    #[must_use]
    pub fn new(surface: S, population: Pop) -> Self {
        Self {
            surface,
            population,
            candidate_selector: ParetoFrequencyWeighted,
            part_selector: RoundRobinPart::new(),
            gate: StrictImprovement,
            _problem: PhantomData,
        }
    }
}

impl<P, S, Pop, CandSel, PartSel, GatePol> Gepa<P, S, Pop, CandSel, PartSel, GatePol>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    /// Build GEPA with explicit strategy values.
    #[must_use]
    pub fn with_strategies(
        surface: S,
        population: Pop,
        candidate_selector: CandSel,
        part_selector: PartSel,
        gate: GatePol,
    ) -> Self {
        Self {
            surface,
            population,
            candidate_selector,
            part_selector,
            gate,
            _problem: PhantomData,
        }
    }

    /// Surface owned by this optimizer.
    #[must_use]
    pub const fn surface(&self) -> &S {
        &self.surface
    }

    /// Population state owned by this optimizer.
    #[must_use]
    pub const fn population(&self) -> &Pop {
        &self.population
    }

    /// Mutable population state owned by this optimizer.
    #[must_use]
    pub const fn population_mut(&mut self) -> &mut Pop {
        &mut self.population
    }

    /// Mutable gate policy.
    #[must_use]
    pub const fn gate_mut(&mut self) -> &mut GatePol {
        &mut self.gate
    }

    /// Select the next candidate to mutate.
    pub fn select_candidate(&mut self, graph: RunGraphView<'_, P>) -> Option<CandidateId>
    where
        CandSel: CandidateSelector<P, Pop, Selection = Option<CandidateId>>,
    {
        self.candidate_selector.select(&self.population, graph)
    }

    /// Select the next surface part to mutate.
    pub fn select_part(&mut self, artifact: &P::Artifact) -> Result<S::PartId, SurfaceError>
    where
        PartSel: PartSelector<P::Artifact, S>,
    {
        self.part_selector.select_part(artifact, &self.surface)
    }

    /// Lower a surface-native edit into an artifact-native change.
    pub fn change_part(
        &self,
        artifact: &P::Artifact,
        part: S::PartId,
        edit: S::Edit,
    ) -> Result<<P::Artifact as leaven_core::Artifact>::Change, SurfaceError> {
        self.surface.change_part(artifact, part, edit)
    }

    /// Apply the configured gate to two scalar screening scores.
    pub fn decide(&mut self, parent_score: f64, candidate_score: f64) -> crate::GateDecision
    where
        GatePol: Gate,
    {
        self.gate.decide(parent_score, candidate_score)
    }
}

/// Builder placeholder for future ergonomic construction.
#[derive(Clone, Debug, Default)]
pub struct GepaBuilder;

/// GEPA configuration placeholder.
#[derive(Clone, Debug, Default)]
pub struct GepaConfig;

/// Merge scheduling placeholder.
#[derive(Clone, Debug, Default)]
pub struct MergeScheduler;
