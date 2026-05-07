use leaven_evidence::Attribution;
use leaven_kernel::FiniteF64;

#[test]
fn attribution_weight_preserves_signed_finite_values() {
    let attribution = Attribution {
        key: "part:summary",
        weight: Some(FiniteF64::new(-0.5).unwrap()),
        note: Some("hurt aggregate score".to_owned()),
    };

    assert_eq!(attribution.weight.map(FiniteF64::as_f64), Some(-0.5));
    assert_eq!(attribution.note.as_deref(), Some("hurt aggregate score"));
}

#[test]
fn attribution_weight_refuses_non_finite_values() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(FiniteF64::new(value).is_err());
    }
}
