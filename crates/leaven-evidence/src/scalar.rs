//! Scalar evidence for single-objective evaluation.

use leaven_core::Evidence;
use ordered_float::NotNan;

/// Finite scalar score evidence for single-objective evaluation.
///
/// The score is finite by construction so preference and population code never
/// has to decide what `NaN` or infinity should mean.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ScalarEvidence {
    score: NotNan<f64>,
}

impl ScalarEvidence {
    /// Build scalar evidence from a finite score.
    pub fn new(score: f64) -> Result<Self, ScalarEvidenceError> {
        if !score.is_finite() {
            return Err(ScalarEvidenceError::NonFinite { score });
        }
        Ok(Self {
            score: NotNan::new(score).map_err(|_| ScalarEvidenceError::NonFinite { score })?,
        })
    }

    /// Return the finite score as a plain `f64` for arithmetic or display.
    #[must_use]
    pub fn score(&self) -> f64 {
        self.score.into_inner()
    }
}

impl Evidence for ScalarEvidence {}

/// Refusal reasons for scalar evidence construction.
#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum ScalarEvidenceError {
    /// The score was `NaN`, positive infinity, or negative infinity.
    #[error("scalar evidence score must be finite: {score}")]
    NonFinite { score: f64 },
}
