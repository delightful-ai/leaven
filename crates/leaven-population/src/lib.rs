//! leaven-population crate skeleton.

mod keep_best;
mod pareto_frontier;
mod tournament;

pub struct BeamPopulation;
pub struct LenientParetoFrontier;
pub struct MapElites;
pub struct NicheDescriptor;
pub struct NoPopulation;
pub struct NoveltyPopulation;
pub struct PlackettLuceFit;
pub struct TournamentConfig;
pub use keep_best::KeepBest;
pub use pareto_frontier::{ParetoFrontier, ParetoFrontierBuilder, PartitionFilter};
pub use tournament::{BradleyTerryFit, TournamentPopulation};

pub mod prelude {
    pub use crate::{
        BeamPopulation, BradleyTerryFit, KeepBest, LenientParetoFrontier, MapElites,
        NicheDescriptor, NoPopulation, NoveltyPopulation, ParetoFrontier, ParetoFrontierBuilder,
        PartitionFilter, PlackettLuceFit, TournamentPopulation,
    };
}
