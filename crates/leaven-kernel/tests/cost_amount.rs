use leaven_kernel::{Amount, Budget, Cost};
use proptest::prelude::*;

proptest! {
    #[test]
    fn valid_amounts_round_trip_through_serde(value in 0u32..1_000_000u32) {
        let value = f64::from(value);
        let amount = Amount::new(value).unwrap();
        let encoded = serde_json::to_string(&amount).unwrap();
        let decoded: Amount = serde_json::from_str(&encoded).unwrap();

        prop_assert_eq!(decoded, amount);
    }
}

#[test]
fn invalid_amounts_are_refused_by_all_public_constructors() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        assert!(Amount::new(value).is_err());
        assert!(Cost::seconds(value).is_err());
        assert!(Budget::seconds(value).is_err());
    }
}

#[test]
fn invalid_amounts_are_refused_during_deserialization() {
    assert!(serde_json::from_str::<Amount>("-1.0").is_err());
    assert!(serde_json::from_str::<Amount>("1e999").is_err());
}

#[test]
fn cost_combination_never_produces_non_finite_amounts() {
    let huge = Cost {
        seconds: Amount::new(f64::MAX).unwrap(),
        ..Cost::zero()
    };

    let combined = huge.clone().combine(&huge);

    assert_eq!(combined.seconds, Amount::new(f64::MAX).unwrap());
}
