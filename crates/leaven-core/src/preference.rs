//! `PreferenceRelation` — interprets evidence to answer "is left
//! preferred to right?".
//!
//! The trait surface is a stub at this stage; a fuller version will
//! land alongside the population subsystem. Stateless aggregators
//! (Copeland, Borda, scalar greater-than, lexicographic) implement
//! this trait directly. Stateful / fitted preferences (Bradley-Terry
//! over accumulated pairwise judgments) live on populations
//! (`TournamentPopulation<BradleyTerryFit>`); the population fits
//! its model in `observe_assessment` and exposes a derived
//! `PreferenceRelation` view.

use serde::{Deserialize, Serialize};

/// Strict three-valued preference.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Preference {
    Left,
    Right,
    Tie,
    /// Available evidence does not decide. Optimizers may treat this
    /// as a request for more evaluation, not as a tie.
    Indeterminate,
}
