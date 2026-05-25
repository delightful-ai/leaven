use leaven_core::Preference;
use leaven_evidence::ScalarEvidence;
use leaven_preference::{HigherScoreIsBetter, LowerScoreIsBetter};

const REMOVED_PLACEHOLDER_PREFERENCES: &[&str] = &[
    "BordaPreference",
    "CopelandPreference",
    "LexicographicPreference",
    "ParetoPreference",
];

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

#[test]
fn crate_root_does_not_export_algorithm_free_placeholder_preferences() {
    let lib = std::fs::read_to_string("src/lib.rs").expect("read preference crate root");

    for symbol in REMOVED_PLACEHOLDER_PREFERENCES {
        assert!(
            !lib.contains(symbol),
            "`{symbol}` must not be reintroduced without an algorithm and contract tests"
        );
    }
}
