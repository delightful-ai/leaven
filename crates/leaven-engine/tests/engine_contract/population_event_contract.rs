mod support;

use leaven_engine::{Arity, Population, PopulationEvent, PopulationView, RunContext, RunGraphView};
use leaven_kernel::{AssessmentId, CandidateId, FiniteF64, PopulationId};

use support::{TestProblem, graph_and_budget};

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

#[test]
fn population_default_observers_emit_no_events() {
    let (mut graph, mut budget) = graph_and_budget();
    let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
    let mut population = EmptyPopulation(PopulationId::new());

    assert!(
        population
            .observe_candidate(CandidateId::new(), ctx.graph())
            .is_empty()
    );
    assert!(
        population
            .observe_assessment(AssessmentId::new(), ctx.graph())
            .is_empty()
    );
}

struct EmptyPopulation(PopulationId);

impl Population<TestProblem> for EmptyPopulation {
    fn id(&self) -> PopulationId {
        self.0
    }

    fn insert_seed(
        &mut self,
        _candidate: CandidateId,
        _graph: RunGraphView<'_, TestProblem>,
    ) -> Vec<PopulationEvent> {
        Vec::new()
    }

    fn select_candidates(
        &mut self,
        _arity: Arity,
        _graph: RunGraphView<'_, TestProblem>,
    ) -> Vec<CandidateId> {
        Vec::new()
    }

    fn best(&self, _graph: RunGraphView<'_, TestProblem>) -> Option<CandidateId> {
        None
    }

    fn view(&self) -> PopulationView<'_> {
        unreachable!("view is not part of the default observer law")
    }
}
