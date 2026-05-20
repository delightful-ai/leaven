use std::collections::BTreeMap;

use leaven_artifact_skill::SkillName;
use leaven_artifact_skill::{
    SkillBank, SkillFile, SkillFolder, SkillPath, SkillRouteKey, SkillRoutePool,
    SkillRouteRegistry, SkillRouteSpec,
};
use leaven_evidence::{PairedRolloutEvidence, RolloutGroupOutcome};
use leaven_kernel::FiniteF64;
use leaven_population::{
    SkillPairedRolloutUtilityInput, SkillPairedRolloutUtilityInputError, SkillPruningCandidate,
    SkillRetrievalCandidate, SkillSimilarityCandidate, SkillSimilarityCandidateError,
    SkillStepTrajectoryOutcome, SkillStepTrajectoryOutcomeError, SkillTwoStageRetrievalConfig,
    SkillTwoStageRetrievalConfigError, SkillTwoStageRetrievalError, SkillTwoStageRetriever,
    SkillUseStats, SkillUtilityCredit, SkillUtilityPruner, SkillUtilityPruningConfig,
    SkillUtilityPruningError, SkillUtilityRank, SkillUtilityRanker, SkillUtilityRankingWeights,
    SkillUtilitySmoothing, SkillUtilityState, SkillUtilityTransfer,
};

fn skill(name: &str) -> SkillName {
    SkillName::new(name).unwrap()
}

fn finite(value: f64) -> FiniteF64 {
    FiniteF64::new(value).unwrap()
}

fn skill_folder(name: &str) -> SkillFolder {
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text(format!(
            "---\nname: {name}\ndescription: Use when testing routed skill retrieval.\n---\nUse this skill.\n"
        )),
    );
    SkillFolder::from_entries(skill(name), entries).unwrap()
}

fn d2skill_route_registry() -> SkillRouteRegistry {
    let task_returns = skill("task-returns");
    let step_stripes = skill("step-stripes");
    let step_warranty = skill("step-warranty");
    let step_unrelated = skill("step-unrelated");
    let bank = SkillBank::from_folders([
        skill_folder(task_returns.as_str()),
        skill_folder(step_stripes.as_str()),
        skill_folder(step_warranty.as_str()),
        skill_folder(step_unrelated.as_str()),
    ])
    .unwrap();

    SkillRouteRegistry::from_specs(
        &bank,
        [
            SkillRouteSpec::new(
                task_returns,
                SkillRoutePool::new("task").unwrap(),
                SkillRouteKey::new("minishop_returns").unwrap(),
            ),
            SkillRouteSpec::new(
                step_stripes,
                SkillRoutePool::new("step").unwrap(),
                SkillRouteKey::new("minishop_returns teal stripe").unwrap(),
            ),
            SkillRouteSpec::new(
                step_warranty,
                SkillRoutePool::new("step").unwrap(),
                SkillRouteKey::new("minishop_returns warranty").unwrap(),
            ),
            SkillRouteSpec::new(
                step_unrelated,
                SkillRoutePool::new("step").unwrap(),
                SkillRouteKey::new("shipping invoice").unwrap(),
            ),
        ],
    )
    .unwrap()
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

#[test]
fn two_stage_retriever_filters_route_pool_by_similarity_then_applies_utility_top_k() {
    let registry = d2skill_route_registry();
    let step_pool = SkillRoutePool::new("step").unwrap();
    let mut state = SkillUtilityState::default();
    let step_stripes = skill("step-stripes");
    let step_warranty = skill("step-warranty");
    let step_unrelated = skill("step-unrelated");
    let task_returns = skill("task-returns");

    state.observe_delta(
        step_warranty.clone(),
        finite(0.7),
        SkillUtilitySmoothing::one(),
    );
    state.observe_delta(
        step_stripes.clone(),
        finite(0.0),
        SkillUtilitySmoothing::one(),
    );
    state.record_retrieval(step_warranty.clone());
    state.record_retrieval(step_warranty.clone());

    let retriever = SkillTwoStageRetriever::new(
        SkillTwoStageRetrievalConfig::new(
            step_pool.clone(),
            finite(0.5),
            2.try_into().unwrap(),
            1.try_into().unwrap(),
            SkillUtilityRankingWeights::new(finite(1.0), finite(1.0), finite(0.0)).unwrap(),
        )
        .unwrap(),
    );

    let plan = retriever
        .retrieve(
            &registry,
            &state,
            [
                SkillSimilarityCandidate::new(step_stripes.clone(), finite(0.95), finite(0.95))
                    .unwrap(),
                SkillSimilarityCandidate::new(step_warranty.clone(), finite(0.8), finite(0.8))
                    .unwrap(),
                SkillSimilarityCandidate::new(step_unrelated, finite(0.4), finite(0.4)).unwrap(),
                SkillSimilarityCandidate::new(task_returns, finite(1.0), finite(1.0)).unwrap(),
            ],
        )
        .unwrap();

    assert_eq!(plan.pool(), &step_pool);
    assert_eq!(
        plan.first_stage()
            .iter()
            .map(|rank| (rank.skill().as_str(), rank.similarity(), rank.relevance()))
            .collect::<Vec<_>>(),
        [
            ("step-stripes", finite(0.95), finite(0.95)),
            ("step-warranty", finite(0.8), finite(0.8)),
        ]
    );
    assert_eq!(
        plan.selected()
            .iter()
            .map(|rank| (rank.skill().as_str(), rank.score()))
            .collect::<Vec<_>>(),
        [("step-warranty", finite(1.5))]
    );

    plan.record_selected_retrievals(&mut state);
    assert_eq!(state.stats(&step_warranty).retrievals, 3);
    assert_eq!(state.stats(&step_stripes).retrievals, 0);
}

#[test]
fn two_stage_retriever_refuses_incomplete_or_invalid_similarity_inputs() {
    let registry = d2skill_route_registry();
    let config = SkillTwoStageRetrievalConfig::new(
        SkillRoutePool::new("step").unwrap(),
        finite(0.0),
        2.try_into().unwrap(),
        1.try_into().unwrap(),
        SkillUtilityRankingWeights::default(),
    )
    .unwrap();
    let retriever = SkillTwoStageRetriever::new(config);

    assert_eq!(
        SkillTwoStageRetrievalConfig::new(
            SkillRoutePool::new("step").unwrap(),
            finite(0.0),
            1.try_into().unwrap(),
            2.try_into().unwrap(),
            SkillUtilityRankingWeights::default(),
        )
        .unwrap_err(),
        SkillTwoStageRetrievalConfigError::TopKExceedsTopM { top_m: 1, top_k: 2 },
    );
    assert_eq!(
        SkillSimilarityCandidate::new(skill("bad"), finite(0.4), finite(1.2)).unwrap_err(),
        SkillSimilarityCandidateError::RelevanceOutOfRange { value: finite(1.2) },
    );
    assert_eq!(
        retriever
            .retrieve(
                &registry,
                &SkillUtilityState::default(),
                [
                    SkillSimilarityCandidate::new(skill("step-stripes"), finite(0.8), finite(0.8),)
                        .unwrap()
                ],
            )
            .unwrap_err(),
        SkillTwoStageRetrievalError::MissingSimilarity {
            skill: skill("step-unrelated"),
        },
    );
}

#[test]
fn paired_rollout_utility_input_applies_task_gap_and_step_credits() {
    let task_alpha = skill("task-alpha");
    let task_beta = skill("task-beta");
    let step_success = skill("step-success");
    let step_failure = skill("step-failure");
    let rollout = PairedRolloutEvidence::new(
        "alfworld-put-cool-mug",
        RolloutGroupOutcome::new(2.try_into().unwrap(), finite(0.25)),
        RolloutGroupOutcome::new(2.try_into().unwrap(), finite(0.75)),
    )
    .unwrap();
    let input = SkillPairedRolloutUtilityInput::new(
        rollout,
        vec![task_alpha.clone(), task_beta.clone()],
        vec![
            SkillUtilityCredit::new(step_success.clone(), finite(0.75)),
            SkillUtilityCredit::new(step_failure.clone(), finite(-0.25)),
        ],
    )
    .unwrap();

    assert_eq!(input.task_delta(), finite(0.5));
    assert_eq!(
        input
            .task_skill_credits()
            .iter()
            .map(|credit| (credit.skill().as_str(), credit.credit()))
            .collect::<Vec<_>>(),
        [("task-alpha", finite(0.5)), ("task-beta", finite(0.5))]
    );
    assert_eq!(
        input
            .all_utility_credits()
            .iter()
            .map(|credit| (credit.skill().as_str(), credit.credit()))
            .collect::<Vec<_>>(),
        [
            ("task-alpha", finite(0.5)),
            ("task-beta", finite(0.5)),
            ("step-success", finite(0.75)),
            ("step-failure", finite(-0.25)),
        ]
    );

    let mut state = SkillUtilityState::default();
    let updates = input.apply_to_state(
        &mut state,
        SkillUtilitySmoothing::one(),
        SkillUtilitySmoothing::one(),
    );

    assert_eq!(updates.task_updates().len(), 2);
    assert_eq!(updates.step_updates().len(), 2);
    assert_eq!(state.utility(&task_alpha), finite(0.5));
    assert_eq!(state.utility(&task_beta), finite(0.5));
    assert_eq!(state.utility(&step_success), finite(0.75));
    assert_eq!(state.utility(&step_failure), finite(-0.25));
    assert_eq!(
        state.stats(&task_alpha),
        SkillUseStats {
            retrievals: 0,
            triggers: 0,
            utility_updates: 1,
        }
    );
}

#[test]
fn paired_rollout_utility_input_refuses_duplicate_task_skill_credit() {
    let duplicate = skill("task-alpha");
    let rollout = PairedRolloutEvidence::new(
        "alfworld-put-cool-mug",
        RolloutGroupOutcome::new(2.try_into().unwrap(), finite(0.25)),
        RolloutGroupOutcome::new(2.try_into().unwrap(), finite(0.75)),
    )
    .unwrap();

    assert_eq!(
        SkillPairedRolloutUtilityInput::new(
            rollout,
            vec![duplicate.clone(), duplicate.clone()],
            Vec::new(),
        )
        .unwrap_err(),
        SkillPairedRolloutUtilityInputError::DuplicateTaskSkill { skill: duplicate },
    );
}

#[test]
fn paired_rollout_utility_input_extracts_step_credits_from_skill_trajectories() {
    let task_alpha = skill("task-alpha");
    let step_open = skill("step-open");
    let step_pick = skill("step-pick");
    let step_fail = skill("step-fail");
    let rollout = PairedRolloutEvidence::new(
        "alfworld-put-cool-mug",
        RolloutGroupOutcome::new(2.try_into().unwrap(), finite(0.25)),
        RolloutGroupOutcome::new(2.try_into().unwrap(), finite(0.75)),
    )
    .unwrap();
    let input = SkillPairedRolloutUtilityInput::from_step_trajectories(
        rollout,
        vec![task_alpha.clone()],
        vec![
            SkillStepTrajectoryOutcome::new(
                "skill-traj-0",
                finite(1.0),
                vec![step_open.clone(), step_pick.clone()],
            )
            .unwrap(),
            SkillStepTrajectoryOutcome::new("skill-traj-1", finite(0.0), vec![step_fail.clone()])
                .unwrap(),
        ],
    )
    .unwrap();

    assert_eq!(
        input
            .step_skill_credits()
            .iter()
            .map(|credit| (credit.skill().as_str(), credit.credit()))
            .collect::<Vec<_>>(),
        [
            ("step-open", finite(0.75)),
            ("step-pick", finite(0.75)),
            ("step-fail", finite(-0.25)),
        ]
    );

    let mut state = SkillUtilityState::default();
    input.apply_to_state(
        &mut state,
        SkillUtilitySmoothing::one(),
        SkillUtilitySmoothing::one(),
    );

    assert_eq!(state.utility(&task_alpha), finite(0.5));
    assert_eq!(state.utility(&step_open), finite(0.75));
    assert_eq!(state.utility(&step_pick), finite(0.75));
    assert_eq!(state.utility(&step_fail), finite(-0.25));
}

#[test]
fn step_trajectory_outcome_refuses_blank_identity() {
    assert_eq!(
        SkillStepTrajectoryOutcome::new("  ", finite(1.0), Vec::new()).unwrap_err(),
        SkillStepTrajectoryOutcomeError::EmptyTrajectoryId,
    );
}

#[test]
fn skill_utility_pruner_evicts_lowest_unprotected_eviction_scores() {
    let mut state = SkillUtilityState::default();
    let stale_low = skill("stale-low");
    let stale_high = skill("stale-high");
    let fresh_low = skill("fresh-low");

    state.observe_delta(
        stale_low.clone(),
        finite(-0.4),
        SkillUtilitySmoothing::one(),
    );
    state.observe_delta(
        stale_high.clone(),
        finite(0.7),
        SkillUtilitySmoothing::one(),
    );
    state.observe_delta(
        fresh_low.clone(),
        finite(-1.0),
        SkillUtilitySmoothing::one(),
    );

    let pruner =
        SkillUtilityPruner::new(SkillUtilityPruningConfig::new(2, 100, 5, finite(0.0)).unwrap());
    let plan = pruner
        .plan(
            &state,
            [
                SkillPruningCandidate::new(stale_low, 10),
                SkillPruningCandidate::new(stale_high, 10),
                SkillPruningCandidate::new(fresh_low, 99),
            ],
        )
        .unwrap();

    assert_eq!(
        plan.evicted()
            .iter()
            .map(|rank| rank.skill().as_str())
            .collect::<Vec<_>>(),
        ["stale-low"]
    );
    assert_eq!(
        plan.kept()
            .iter()
            .map(|rank| (rank.skill().as_str(), rank.is_protected()))
            .collect::<Vec<_>>(),
        [("fresh-low", true), ("stale-high", false)]
    );
    assert!(plan.capacity_satisfied());
}

#[test]
fn skill_utility_pruner_uses_ucb_bonus_for_eviction_scores() {
    let mut state = SkillUtilityState::default();
    let saturated = skill("saturated");
    let fresh = skill("fresh");
    for _ in 0..8 {
        state.record_retrieval(saturated.clone());
    }

    let pruner =
        SkillUtilityPruner::new(SkillUtilityPruningConfig::new(1, 100, 0, finite(1.0)).unwrap());
    let plan = pruner
        .plan(
            &state,
            [
                SkillPruningCandidate::new(saturated.clone(), 10),
                SkillPruningCandidate::new(fresh.clone(), 10),
            ],
        )
        .unwrap();

    assert_eq!(plan.evicted()[0].skill(), &saturated);
    assert_eq!(plan.kept()[0].skill(), &fresh);
    assert!(plan.kept()[0].exploration_bonus() > plan.evicted()[0].exploration_bonus());
}

#[test]
fn skill_utility_pruner_refuses_duplicate_candidates_and_bad_weight() {
    let duplicate = skill("duplicate");
    let pruner =
        SkillUtilityPruner::new(SkillUtilityPruningConfig::new(1, 100, 0, finite(0.0)).unwrap());

    assert_eq!(
        pruner
            .plan(
                &SkillUtilityState::default(),
                [
                    SkillPruningCandidate::new(duplicate.clone(), 10),
                    SkillPruningCandidate::new(duplicate.clone(), 10),
                ],
            )
            .unwrap_err(),
        SkillUtilityPruningError::DuplicateCandidate { skill: duplicate },
    );
    assert!(matches!(
        SkillUtilityPruningConfig::new(1, 100, 0, finite(-0.01)),
        Err(SkillUtilityPruningError::NegativeExplorationWeight { .. }),
    ));
}
