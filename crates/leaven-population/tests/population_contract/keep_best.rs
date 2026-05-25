use leaven_evidence::ScalarEvidence;
use leaven_kernel::{AssessmentId, CandidateId};
use leaven_population::KeepBest;

#[test]
fn default_population_starts_empty() {
    let population = KeepBest::default();

    assert!(population.best().is_none());
    assert!(population.best_score().is_none());
    assert!(population.best_assessment().is_none());
}

#[test]
fn first_observation_becomes_best() {
    let mut population = KeepBest::new();
    let candidate = CandidateId::new();
    let assessment = AssessmentId::new();

    let events = population.observe(candidate, assessment, ScalarEvidence::new(1.0).unwrap());

    assert_eq!(population.best(), Some(candidate));
    assert_eq!(population.best_assessment(), Some(assessment));
    assert_eq!(population.best_score(), Some(1.0));
    assert!(matches!(
        events.as_slice(),
        [leaven_engine::PopulationEvent::Inserted { .. }]
    ));
}

#[test]
fn higher_score_replaces_current_best() {
    let mut population = KeepBest::new();
    let old = CandidateId::new();
    let new = CandidateId::new();
    population.observe(old, AssessmentId::new(), ScalarEvidence::new(1.0).unwrap());

    let events = population.observe(new, AssessmentId::new(), ScalarEvidence::new(2.0).unwrap());

    assert_eq!(population.best(), Some(new));
    assert_eq!(population.best_score(), Some(2.0));
    assert!(matches!(
        events.as_slice(),
        [leaven_engine::PopulationEvent::Replaced { .. }]
    ));
}

#[test]
fn lower_or_equal_score_does_not_replace_current_best() {
    let mut population = KeepBest::new();
    let best = CandidateId::new();
    let lower = CandidateId::new();
    let equal = CandidateId::new();
    population.observe(best, AssessmentId::new(), ScalarEvidence::new(2.0).unwrap());

    let lower_events = population.observe(
        lower,
        AssessmentId::new(),
        ScalarEvidence::new(1.0).unwrap(),
    );
    let equal_events = population.observe(
        equal,
        AssessmentId::new(),
        ScalarEvidence::new(2.0).unwrap(),
    );

    assert_eq!(population.best(), Some(best));
    assert!(matches!(
        lower_events.as_slice(),
        [leaven_engine::PopulationEvent::Ignored { .. }]
    ));
    assert!(matches!(
        equal_events.as_slice(),
        [leaven_engine::PopulationEvent::Ignored { .. }]
    ));
}
