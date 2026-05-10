//! GEPA gate policies.

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Gate result for a proposed candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateDecision {
    /// Admit the candidate.
    Accept,
    /// Reject the candidate.
    Reject,
}

impl GateDecision {
    /// Whether the candidate is accepted.
    #[must_use]
    pub const fn is_accept(self) -> bool {
        matches!(self, Self::Accept)
    }
}

/// Decides whether a screened candidate should enter population state.
pub trait Gate {
    /// Decide from parent and candidate scalar scores.
    fn decide(&mut self, parent_score: f64, candidate_score: f64) -> GateDecision;
}

/// Private gate state that must survive GEPA checkpoint/restore.
pub trait CheckpointGate {
    /// Serializable state shape.
    type State: Serialize + DeserializeOwned;

    /// Capture gate state.
    fn checkpoint_state(&self) -> Self::State;

    /// Restore gate state.
    fn restore_state(&mut self, state: Self::State);
}

/// Accept only strictly better candidates.
#[derive(Clone, Debug, Default)]
pub struct StrictImprovement;

impl CheckpointGate for StrictImprovement {
    type State = ();

    fn checkpoint_state(&self) -> Self::State {}

    fn restore_state(&mut self, _state: Self::State) {}
}

impl Gate for StrictImprovement {
    fn decide(&mut self, parent_score: f64, candidate_score: f64) -> GateDecision {
        if candidate_score > parent_score {
            GateDecision::Accept
        } else {
            GateDecision::Reject
        }
    }
}

/// Accept equal or better candidates.
#[derive(Clone, Debug, Default)]
pub struct ImprovementOrEqual;

impl CheckpointGate for ImprovementOrEqual {
    type State = ();

    fn checkpoint_state(&self) -> Self::State {}

    fn restore_state(&mut self, _state: Self::State) {}
}

impl Gate for ImprovementOrEqual {
    fn decide(&mut self, parent_score: f64, candidate_score: f64) -> GateDecision {
        if candidate_score >= parent_score {
            GateDecision::Accept
        } else {
            GateDecision::Reject
        }
    }
}

/// Accept candidates that do not regress.
#[derive(Clone, Debug, Default)]
pub struct NoRegression;

impl CheckpointGate for NoRegression {
    type State = ();

    fn checkpoint_state(&self) -> Self::State {}

    fn restore_state(&mut self, _state: Self::State) {}
}

impl Gate for NoRegression {
    fn decide(&mut self, parent_score: f64, candidate_score: f64) -> GateDecision {
        ImprovementOrEqual.decide(parent_score, candidate_score)
    }
}
