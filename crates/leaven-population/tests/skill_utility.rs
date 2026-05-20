use leaven_artifact_skill::SkillName;
use leaven_kernel::FiniteF64;
use leaven_population::{
    SkillUseStats, SkillUtilitySmoothing, SkillUtilityState, SkillUtilityTransfer,
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
