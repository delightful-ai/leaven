//! leaven-population crate skeleton.

mod beam;
mod keep_best;
mod map_elites;
mod no_population;
mod novelty;
mod pareto;
mod pareto_frontier;
mod plackett_luce;
mod top_k_frontier;
mod tournament;
mod tournament_config;

pub use beam::BeamPopulation;
pub use keep_best::KeepBest;
pub use map_elites::{MapElites, NicheDescriptor};
pub use no_population::NoPopulation;
pub use novelty::NoveltyPopulation;
pub use pareto::LenientParetoFrontier;
pub use pareto_frontier::{ParetoFrontier, ParetoFrontierBuilder, PartitionFilter};
pub use plackett_luce::PlackettLuceFit;
pub use top_k_frontier::{TopKFrontier, TopKParentSelectionPolicy, TopKParentSelector};
pub use tournament::{BradleyTerryFit, TournamentPopulation};
pub use tournament_config::TournamentConfig;

pub mod prelude {
    pub use crate::{
        BeamPopulation, BradleyTerryFit, KeepBest, LenientParetoFrontier, MapElites,
        NicheDescriptor, NoPopulation, NoveltyPopulation, ParetoFrontier, ParetoFrontierBuilder,
        PartitionFilter, PlackettLuceFit, TopKFrontier, TopKParentSelectionPolicy,
        TopKParentSelector, TournamentPopulation,
    };
}
