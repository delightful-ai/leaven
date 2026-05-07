use leaven_evidence::{PairwiseJudgment, PairwiseJudgmentEvidence};
use leaven_kernel::FiniteF64;

#[test]
fn pairwise_judgment_is_first_class() {
    let left = PairwiseJudgmentEvidence::new(PairwiseJudgment::Left);
    let right = PairwiseJudgmentEvidence::new(PairwiseJudgment::Right);
    let tie = PairwiseJudgmentEvidence::new(PairwiseJudgment::Tie);

    assert_eq!(left.judgment(), PairwiseJudgment::Left);
    assert_eq!(right.judgment(), PairwiseJudgment::Right);
    assert_eq!(tie.judgment(), PairwiseJudgment::Tie);
}

#[test]
fn confidence_is_finite_by_construction() {
    let evidence = PairwiseJudgmentEvidence::with_confidence(
        PairwiseJudgment::Left,
        FiniteF64::new(0.75).unwrap(),
    );

    assert_eq!(evidence.confidence().map(FiniteF64::as_f64), Some(0.75));
    assert!(FiniteF64::new(f64::NAN).is_err());
    assert!(FiniteF64::new(f64::INFINITY).is_err());
    assert!(FiniteF64::new(f64::NEG_INFINITY).is_err());
}

#[test]
fn rationale_is_optional_context() {
    let evidence = PairwiseJudgmentEvidence::with_rationale(
        PairwiseJudgment::Right,
        "right candidate preserves the required invariant",
    );

    assert_eq!(evidence.judgment(), PairwiseJudgment::Right);
    assert_eq!(
        evidence.rationale(),
        Some("right candidate preserves the required invariant")
    );
    assert_eq!(evidence.confidence(), None);
}
