use leaven_evidence::{PairwiseJudgment, PairwiseJudgmentEvidence};
use leaven_kernel::{AssessmentId, CandidateId, FiniteF64};
use leaven_population::{BradleyTerryFit, TournamentPopulation};

#[test]
fn bradley_terry_starts_unseen_candidates_at_zero() {
    let fit = BradleyTerryFit::new(FiniteF64::new(0.2).unwrap());

    assert_eq!(fit.ability(CandidateId::new()), FiniteF64::ZERO);
    assert_eq!(fit.best(), None);
}

#[test]
fn bradley_terry_left_judgment_moves_abilities_apart() {
    let left = CandidateId::new();
    let right = CandidateId::new();
    let mut fit = BradleyTerryFit::new(FiniteF64::new(0.2).unwrap());

    fit.observe_pairwise(left, right, PairwiseJudgment::Left);

    assert!(fit.ability(left).as_f64() > 0.0);
    assert!(fit.ability(right).as_f64() < 0.0);
    assert_eq!(fit.best(), Some(left));
}

#[test]
fn bradley_terry_right_judgment_moves_abilities_apart() {
    let left = CandidateId::new();
    let right = CandidateId::new();
    let mut fit = BradleyTerryFit::new(FiniteF64::new(0.2).unwrap());

    fit.observe_pairwise(left, right, PairwiseJudgment::Right);

    assert!(fit.ability(left).as_f64() < 0.0);
    assert!(fit.ability(right).as_f64() > 0.0);
    assert_eq!(fit.best(), Some(right));
}

#[test]
fn tie_leaves_equal_abilities_equal() {
    let left = CandidateId::new();
    let right = CandidateId::new();
    let mut fit = BradleyTerryFit::new(FiniteF64::new(0.2).unwrap());

    fit.observe_pairwise(left, right, PairwiseJudgment::Tie);

    assert_eq!(fit.ability(left), FiniteF64::ZERO);
    assert_eq!(fit.ability(right), FiniteF64::ZERO);
    assert_eq!(fit.best(), Some(left.min(right)));
}

#[test]
fn repeated_observations_cover_both_logistic_branches() {
    let left = CandidateId::new();
    let right = CandidateId::new();
    let mut fit = BradleyTerryFit::default();

    fit.observe_pairwise(left, right, PairwiseJudgment::Right);
    let after_right = fit.ability(left);
    fit.observe_pairwise(left, right, PairwiseJudgment::Left);

    assert!(after_right.as_f64() < 0.0);
    assert!(fit.ability(left).as_f64() > after_right.as_f64());
}

#[test]
fn tournament_population_observes_pairwise_evidence() {
    let left = CandidateId::new();
    let right = CandidateId::new();
    let mut population =
        TournamentPopulation::new(BradleyTerryFit::new(FiniteF64::new(0.2).unwrap()));

    let events = population.observe_pairwise(
        left,
        right,
        AssessmentId::new(),
        &PairwiseJudgmentEvidence::new(PairwiseJudgment::Right),
    );

    assert_eq!(population.observations(), 1);
    assert_ne!(population.id(), leaven_kernel::PopulationId::new());
    assert_eq!(population.best(), Some(right));
    assert_eq!(events.len(), 2);
    assert!(population.ability(right).as_f64() > population.ability(left).as_f64());
}

#[test]
fn default_tournament_population_starts_empty() {
    let population = TournamentPopulation::default();

    assert_eq!(population.best(), None);
    assert_eq!(population.observations(), 0);
}
