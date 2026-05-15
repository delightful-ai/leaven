//! Case assessment evidence that preserves generated output, score, and feedback.

use leaven_core::Evidence;
use serde::{Deserialize, Serialize};

use crate::{OutputRecord, ScalarEvidence};

/// One case assessment outcome.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseAssessmentEvidence {
    score: ScalarEvidence,
    output: OutputRecord,
    feedback: String,
}

impl CaseAssessmentEvidence {
    /// Builds case assessment evidence.
    #[must_use]
    pub fn new(score: ScalarEvidence, output: OutputRecord, feedback: impl Into<String>) -> Self {
        Self {
            score,
            output,
            feedback: feedback.into(),
        }
    }

    /// Comparable scalar score.
    #[must_use]
    pub const fn score(&self) -> ScalarEvidence {
        self.score
    }

    /// Generated output that was scored.
    #[must_use]
    pub const fn output(&self) -> &OutputRecord {
        &self.output
    }

    /// Natural-language feedback attached to the score.
    #[must_use]
    pub fn feedback(&self) -> &str {
        &self.feedback
    }
}

impl Evidence for CaseAssessmentEvidence {}
