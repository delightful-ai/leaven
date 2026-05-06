//! `Population` and frontier types.
//!
//! Populations are optimizer-owned live state: keep-best, Pareto
//! frontier, MAP-Elites archive, tournament. They observe assessments,
//! emit [`PopulationEvent`]s, and answer "what should I work on
//! next?" queries. The full trait surface is intentionally deferred to
//! a later iteration; this module currently defines only the durable
//! event shape recorded into the run graph.

use serde::{Deserialize, Serialize};

use crate::ids::{AssessmentId, CandidateId, PopulationId};

/// A typed update to a population. The graph stores the event log so
/// the population's history can be replayed without re-running the
/// algorithm.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PopulationEvent {
    /// Candidate entered the population (e.g. added to a frontier).
    Inserted {
        population: PopulationId,
        candidate: CandidateId,
    },

    /// Candidate left the population (e.g. dominated and evicted from
    /// a frontier, lost a tournament round, evicted from a niche).
    Removed {
        population: PopulationId,
        candidate: CandidateId,
        reason: PopulationRemovalReason,
    },

    /// The population observed an assessment that updated its state
    /// (e.g. a tournament fit step). No insertion or removal.
    Observed {
        population: PopulationId,
        assessment: AssessmentId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum PopulationRemovalReason {
    Dominated,
    Replaced,
    Evicted,
    LostTournament,
    PolicyExpired,
}
