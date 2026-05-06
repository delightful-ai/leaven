use leaven_core::Preference;
use leaven_evidence::ScalarEvidence;
use leaven_preference::{HigherScoreIsBetter, LowerScoreIsBetter};

#[test]
fn higher_score_is_better_prefers_larger_score() {
    assert_eq!(
        HigherScoreIsBetter::prefer_scores(
            ScalarEvidence::new(2.0).unwrap(),
            ScalarEvidence::new(1.0).unwrap()
        ),
        Preference::LeftBetter
    );
    assert_eq!(
        HigherScoreIsBetter::prefer_scores(
            ScalarEvidence::new(1.0).unwrap(),
            ScalarEvidence::new(2.0).unwrap()
        ),
        Preference::RightBetter
    );
}

#[test]
fn higher_score_is_better_ties_equal_scores() {
    assert_eq!(
        HigherScoreIsBetter::prefer_scores(
            ScalarEvidence::new(2.0).unwrap(),
            ScalarEvidence::new(2.0).unwrap()
        ),
        Preference::Equivalent
    );
}

#[test]
fn scalar_preference_never_sees_non_finite_scores() {
    assert!(ScalarEvidence::new(f64::NAN).is_err());
}

#[test]
fn lower_score_is_better_reverses_the_ordering() {
    assert_eq!(
        LowerScoreIsBetter::prefer_scores(
            ScalarEvidence::new(1.0).unwrap(),
            ScalarEvidence::new(2.0).unwrap()
        ),
        Preference::LeftBetter
    );
}
