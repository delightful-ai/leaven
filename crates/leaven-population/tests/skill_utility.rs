use leaven_artifact_skill::SkillName;
use leaven_kernel::FiniteF64;
use leaven_population::{
    SkillRetrievalCandidate, SkillUseStats, SkillUtilityRank, SkillUtilityRanker,
    SkillUtilityRankingWeights, SkillUtilitySmoothing, SkillUtilityState, SkillUtilityTransfer,
};

fn skill(name: &str) -> SkillName {
    SkillName::new(name).unwrap()
}

fn finite(value: f64) -> FiniteF64 {
    FiniteF64::new(value).unwrap()
}

#[test]
fn skill_utility_state_updates_utility_with_checkpointable_ema_stats() {
    let mut state = SkillUtilityState::default();
    let alpha = skill("alpha");
    let smoothing = SkillUtilitySmoothing::new(0.5).unwrap();

    let first = state.observe_delta(alpha.clone(), finite(1.0), smoothing);
    let second = state.observe_delta(alpha.clone(), finite(-1.0), smoothing);

    assert_eq!(first.utility_before(), FiniteF64::ZERO);
    assert_eq!(first.utility_after(), finite(0.5));
    assert_eq!(second.utility_before(), finite(0.5));
    assert_eq!(second.utility_after(), finite(-0.25));
    assert_eq!(state.utility(&alpha), finite(-0.25));
    assert_eq!(
        state.stats(&alpha),
        SkillUseStats {
            retrievals: 0,
            triggers: 0,
            utility_updates: 2,
        }
    );
}

#[test]
fn skill_utility_state_tracks_retrievals_and_triggers_without_artifact_mutation() {
    let mut state = SkillUtilityState::default();
    let alpha = skill("alpha");

    state.record_retrieval(alpha.clone());
    state.record_retrieval(alpha.clone());
    state.record_trigger(alpha.clone());

    assert_eq!(state.utility(&alpha), FiniteF64::ZERO);
    assert_eq!(
        state.stats(&alpha),
        SkillUseStats {
            retrievals: 2,
            triggers: 1,
            utility_updates: 0,
        }
    );
}

#[test]
fn skill_utility_state_transfers_or_discards_state_on_lifecycle_changes() {
    let mut state = SkillUtilityState::default();
    let alpha = skill("alpha");
    let beta = skill("beta");
    let occupied = skill("occupied");

    state.observe_delta(alpha.clone(), finite(1.0), SkillUtilitySmoothing::one());
    state.record_retrieval(alpha.clone());
    state.record_trigger(alpha.clone());
    state.observe_delta(occupied.clone(), finite(0.25), SkillUtilitySmoothing::one());

    assert_eq!(
        state.transfer_skill(&alpha, beta.clone()),
        SkillUtilityTransfer::Transferred
    );
    assert_eq!(state.utility(&alpha), FiniteF64::ZERO);
    assert_eq!(state.utility(&beta), finite(1.0));
    assert_eq!(
        state.stats(&beta),
        SkillUseStats {
            retrievals: 1,
            triggers: 1,
            utility_updates: 1,
        }
    );

    assert_eq!(
        state.transfer_skill(&beta, occupied),
        SkillUtilityTransfer::TargetExists
    );
    assert!(state.remove_skill(&beta));
    assert_eq!(state.utility(&beta), FiniteF64::ZERO);
    assert_eq!(state.stats(&beta), SkillUseStats::default());
    assert_eq!(
        state.transfer_skill(&beta, skill("missing-target")),
        SkillUtilityTransfer::SourceMissing
    );
}

#[test]
fn skill_utility_smoothing_rejects_nonfinite_and_out_of_range_weights() {
    assert_eq!(
        SkillUtilitySmoothing::new(-0.01).unwrap_err(),
        leaven_population::SkillUtilitySmoothingError::OutOfRange { value: -0.01 }
    );
    assert_eq!(
        SkillUtilitySmoothing::new(1.01).unwrap_err(),
        leaven_population::SkillUtilitySmoothingError::OutOfRange { value: 1.01 }
    );
    assert!(matches!(
        SkillUtilitySmoothing::new(f64::NAN),
        Err(leaven_population::SkillUtilitySmoothingError::NonFinite { .. })
    ));
}

#[test]
fn skill_utility_ranker_combines_relevance_and_utility_for_top_k_selection() {
    let mut state = SkillUtilityState::default();
    let alpha = skill("alpha");
    let beta = skill("beta");
    let gamma = skill("gamma");

    state.observe_delta(alpha.clone(), finite(0.2), SkillUtilitySmoothing::one());
    state.observe_delta(beta.clone(), finite(0.8), SkillUtilitySmoothing::one());

    let ranker = SkillUtilityRanker::new(
        SkillUtilityRankingWeights::new(finite(1.0), finite(1.0), finite(0.0)).unwrap(),
    );
    let ranked = ranker.top_k(
        &state,
        [
            SkillRetrievalCandidate::new(alpha.clone(), finite(0.9)),
            SkillRetrievalCandidate::new(beta.clone(), finite(0.6)),
            SkillRetrievalCandidate::new(gamma, finite(0.6)),
        ],
        2.try_into().unwrap(),
    );

    assert_eq!(
        ranked
            .iter()
            .map(SkillUtilityRank::skill)
            .collect::<Vec<_>>(),
        [&beta, &alpha]
    );
    assert_eq!(ranked[0].score(), finite(1.4));
    assert_eq!(ranked[0].utility(), finite(0.8));
    assert_eq!(ranked[0].relevance(), finite(0.6));
}

#[test]
fn skill_utility_ranker_adds_ucb_exploration_for_less_retrieved_skills() {
    let mut state = SkillUtilityState::default();
    let saturated = skill("saturated");
    let fresh = skill("fresh");
    for _ in 0..8 {
        state.record_retrieval(saturated.clone());
    }

    let ranker = SkillUtilityRanker::new(
        SkillUtilityRankingWeights::new(finite(0.0), finite(0.0), finite(1.0)).unwrap(),
    );
    let ranked = ranker.rank(
        &state,
        [
            SkillRetrievalCandidate::new(saturated.clone(), finite(0.5)),
            SkillRetrievalCandidate::new(fresh.clone(), finite(0.5)),
        ],
    );

    assert_eq!(
        ranked
            .iter()
            .map(SkillUtilityRank::skill)
            .collect::<Vec<_>>(),
        [&fresh, &saturated]
    );
    assert!(ranked[0].exploration_bonus() > ranked[1].exploration_bonus());
}

#[test]
fn skill_utility_ranker_uses_stable_skill_name_tiebreaks_and_rejects_bad_weights() {
    let state = SkillUtilityState::default();
    let ranker = SkillUtilityRanker::default();
    let ranked = ranker.rank(
        &state,
        [
            SkillRetrievalCandidate::new(skill("beta"), finite(1.0)),
            SkillRetrievalCandidate::new(skill("alpha"), finite(1.0)),
        ],
    );

    assert_eq!(
        ranked
            .iter()
            .map(|rank| rank.skill().as_str())
            .collect::<Vec<_>>(),
        ["alpha", "beta"]
    );
    assert!(matches!(
        SkillUtilityRankingWeights::new(finite(-1.0), finite(0.0), finite(0.0)),
        Err(leaven_population::SkillUtilityRankingWeightsError::NegativeRelevanceWeight { .. })
    ));
    assert!(matches!(
        SkillUtilityRankingWeights::new(finite(0.0), finite(0.0), finite(-1.0)),
        Err(leaven_population::SkillUtilityRankingWeightsError::NegativeExplorationWeight { .. })
    ));
}
