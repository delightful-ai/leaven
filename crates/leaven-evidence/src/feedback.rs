//! Score evidence that preserves textual feedback and execution traces.

use leaven_core::Evidence;
use serde::{Deserialize, Serialize};

use crate::ScalarEvidence;

/// One scored evaluation outcome plus proposer-readable feedback.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScoredFeedbackEvidence {
    score: ScalarEvidence,
    feedback: String,
    trace: Vec<String>,
}

impl ScoredFeedbackEvidence {
    /// Builds a scored feedback record.
    #[must_use]
    pub fn new(score: ScalarEvidence, feedback: impl Into<String>, trace: Vec<String>) -> Self {
        Self {
            score,
            feedback: feedback.into(),
            trace,
        }
    }

    /// Comparable scalar score.
    #[must_use]
    pub const fn score(&self) -> ScalarEvidence {
        self.score
    }

    /// Natural-language feedback attached to the score.
    #[must_use]
    pub fn feedback(&self) -> &str {
        &self.feedback
    }

    /// Execution trace lines captured while producing the score.
    #[must_use]
    pub fn trace(&self) -> &[String] {
        &self.trace
    }
}

impl Evidence for ScoredFeedbackEvidence {}
