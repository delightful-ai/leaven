use leaven_kernel::{FiniteF64, MetadataValue};
use proptest::prelude::*;

proptest! {
    #[test]
    fn finite_values_round_trip_through_serde(value in -1_000_000i32..1_000_000i32) {
        let value = f64::from(value) / 10.0;
        let finite = FiniteF64::new(value).unwrap();
        let encoded = serde_json::to_string(&finite).unwrap();
        let decoded: FiniteF64 = serde_json::from_str(&encoded).unwrap();

        prop_assert_eq!(decoded, finite);
    }
}

#[test]
fn finite_f64_preserves_negative_values() {
    let value = FiniteF64::new(-0.25).unwrap();

    assert_eq!(value, FiniteF64::new(-0.25).unwrap());
}

#[test]
fn finite_f64_refuses_nan_and_infinities() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(FiniteF64::new(value).is_err());
    }
}

#[test]
fn finite_f64_refuses_non_finite_deserialization() {
    assert!(serde_json::from_str::<FiniteF64>("1e999").is_err());
}

#[test]
fn metadata_float_values_are_finite_by_construction() {
    let value = MetadataValue::F64(FiniteF64::new(1.25).unwrap());

    assert!(matches!(
        value,
        MetadataValue::F64(number) if number == FiniteF64::new(1.25).unwrap()
    ));
}
