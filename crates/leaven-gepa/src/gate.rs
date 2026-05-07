//! GEPA gate policies.

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

/// Accept only strictly better candidates.
#[derive(Clone, Debug, Default)]
pub struct StrictImprovement;

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

impl Gate for NoRegression {
    fn decide(&mut self, parent_score: f64, candidate_score: f64) -> GateDecision {
        ImprovementOrEqual.decide(parent_score, candidate_score)
    }
}
