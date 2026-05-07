//! Pairwise judgment evidence.

use leaven_core::Evidence;
use leaven_kernel::FiniteF64;

/// Winner selected by a pairwise evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairwiseJudgment {
    /// The left candidate won.
    Left,
    /// The right candidate won.
    Right,
    /// Neither candidate won.
    Tie,
}

/// Evidence produced by a pairwise judge.
///
/// Confidence is finite by construction. Rationale is debug/human context only;
/// algorithms should route on [`judgment`](Self::judgment) and optional
/// confidence, not require prose to be present.
#[derive(Clone, Debug, PartialEq)]
pub struct PairwiseJudgmentEvidence {
    judgment: PairwiseJudgment,
    confidence: Option<FiniteF64>,
    rationale: Option<String>,
}

impl PairwiseJudgmentEvidence {
    /// Build judgment evidence with no confidence or rationale.
    #[must_use]
    pub const fn new(judgment: PairwiseJudgment) -> Self {
        Self {
            judgment,
            confidence: None,
            rationale: None,
        }
    }

    /// Build judgment evidence with finite confidence.
    #[must_use]
    pub const fn with_confidence(judgment: PairwiseJudgment, confidence: FiniteF64) -> Self {
        Self {
            judgment,
            confidence: Some(confidence),
            rationale: None,
        }
    }

    /// Build judgment evidence with rationale text.
    pub fn with_rationale(judgment: PairwiseJudgment, rationale: impl Into<String>) -> Self {
        Self {
            judgment,
            confidence: None,
            rationale: Some(rationale.into()),
        }
    }

    /// Build judgment evidence with confidence and rationale text.
    pub fn with_confidence_and_rationale(
        judgment: PairwiseJudgment,
        confidence: FiniteF64,
        rationale: impl Into<String>,
    ) -> Self {
        Self {
            judgment,
            confidence: Some(confidence),
            rationale: Some(rationale.into()),
        }
    }

    /// Return the pairwise outcome.
    #[must_use]
    pub const fn judgment(&self) -> PairwiseJudgment {
        self.judgment
    }

    /// Return judge confidence, when supplied.
    #[must_use]
    pub const fn confidence(&self) -> Option<FiniteF64> {
        self.confidence
    }

    /// Return rationale text, when supplied.
    #[must_use]
    pub fn rationale(&self) -> Option<&str> {
        self.rationale.as_deref()
    }
}

impl Evidence for PairwiseJudgmentEvidence {}
