use leaven_evidence::{PairwiseJudgment, PairwiseJudgmentEvidence};
use leaven_kernel::{AssessmentId, CandidateId, FiniteF64};
use leaven_population::{BradleyTerryFit, TournamentPopulation};
use proptest::prelude::*;

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

proptest! {
    #[test]
    fn generated_pairwise_updates_keep_abilities_finite(
        rate in 0.001_f64..1.0,
        judgments in proptest::collection::vec(any_pairwise_judgment(), 1..64),
    ) {
        let left = CandidateId::new();
        let right = CandidateId::new();
        let mut fit = BradleyTerryFit::new(FiniteF64::new(rate).unwrap());

        for judgment in judgments {
            fit.observe_pairwise(left, right, judgment);
            prop_assert!(fit.ability(left).as_f64().is_finite());
            prop_assert!(fit.ability(right).as_f64().is_finite());
        }
    }

    #[test]
    fn repeated_left_wins_never_rank_right_above_left(
        rate in 0.001_f64..1.0,
        observation_count in 1_usize..64,
    ) {
        let left = CandidateId::new();
        let right = CandidateId::new();
        let mut fit = BradleyTerryFit::new(FiniteF64::new(rate).unwrap());

        for _ in 0..observation_count {
            fit.observe_pairwise(left, right, PairwiseJudgment::Left);
            prop_assert!(fit.ability(left) >= fit.ability(right));
            prop_assert_eq!(fit.best(), Some(left));
        }
    }
}

fn any_pairwise_judgment() -> impl Strategy<Value = PairwiseJudgment> {
    prop_oneof![
        Just(PairwiseJudgment::Left),
        Just(PairwiseJudgment::Right),
        Just(PairwiseJudgment::Tie),
    ]
}
