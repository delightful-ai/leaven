use leaven_engine::PopulationEvent;
use leaven_kernel::{CandidateId, FiniteF64, PopulationId};

#[test]
fn reweighted_population_events_use_finite_weights() {
    let event = PopulationEvent::Reweighted {
        population: PopulationId::new(),
        candidate: CandidateId::new(),
        weight: FiniteF64::new(0.75).unwrap(),
        reason: "frequency update".to_owned(),
    };

    assert!(matches!(
        event,
        PopulationEvent::Reweighted { weight, .. } if weight == FiniteF64::new(0.75).unwrap()
    ));
}

#[test]
fn population_event_weights_reject_nan_and_infinities() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(FiniteF64::new(value).is_err());
    }
}
