//! Evidence adapters used by GEPA scoring.

use leaven_evidence::{CaseAssessmentEvidence, ScalarEvidence};

/// One assessment-row evidence shape GEPA can compare as a scalar score.
pub trait GepaCaseEvidence: leaven_core::Evidence {
    /// Project the comparable scalar score for this case row.
    fn scalar_score(&self) -> Option<ScalarEvidence>;
}

impl GepaCaseEvidence for ScalarEvidence {
    fn scalar_score(&self) -> Option<ScalarEvidence> {
        Some(*self)
    }
}

impl GepaCaseEvidence for CaseAssessmentEvidence {
    fn scalar_score(&self) -> Option<ScalarEvidence> {
        Some(self.score())
    }
}
