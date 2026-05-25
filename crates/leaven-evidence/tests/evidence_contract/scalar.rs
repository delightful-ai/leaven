use leaven_evidence::ScalarEvidence;

#[test]
fn scalar_evidence_preserves_score() {
    let evidence = ScalarEvidence::new(7.5).unwrap();

    assert!((evidence.score() - 7.5).abs() < f64::EPSILON);
}

#[test]
fn scalar_evidence_refuses_non_finite_scores() {
    assert!(ScalarEvidence::new(f64::NAN).is_err());
    assert!(ScalarEvidence::new(f64::INFINITY).is_err());
    assert!(ScalarEvidence::new(f64::NEG_INFINITY).is_err());
}
