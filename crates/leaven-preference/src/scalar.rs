//! Scalar preference relations.

use leaven_core::Preference;
use leaven_evidence::ScalarEvidence;

pub struct HigherScoreIsBetter;
pub struct LowerScoreIsBetter;

impl HigherScoreIsBetter {
    #[must_use]
    pub fn prefer_scores(left: ScalarEvidence, right: ScalarEvidence) -> Preference {
        match left.cmp(&right) {
            std::cmp::Ordering::Greater => Preference::LeftBetter,
            std::cmp::Ordering::Less => Preference::RightBetter,
            std::cmp::Ordering::Equal => Preference::Equivalent,
        }
    }
}

impl LowerScoreIsBetter {
    #[must_use]
    pub fn prefer_scores(left: ScalarEvidence, right: ScalarEvidence) -> Preference {
        HigherScoreIsBetter::prefer_scores(right, left)
    }
}
