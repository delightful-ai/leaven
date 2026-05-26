use std::{collections::BTreeMap, path::Path};

use leaven_engine::{PrivateStatePolicy, StateFormat};

use super::storage::write_atomic;
use super::*;

#[test]
fn optimizer_summaries_disclose_checkpoint_policy_and_format() {
    let derived = OptimizerCompatibility::new(
        Fingerprint::from_bytes([1; 32]),
        PrivateStatePolicy::DerivedFromGraph,
    );
    assert!(optimizer_summary(Some(&derived)).contains(";state=derived-from-graph"));

    let postcard = OptimizerCompatibility::new(
        Fingerprint::from_bytes([2; 32]),
        PrivateStatePolicy::ExplicitSnapshot {
            schema: Fingerprint::from_bytes([3; 32]),
            format: StateFormat::Postcard,
        },
    );
    let summary = optimizer_summary(Some(&postcard));
    assert!(summary.contains(";state=explicit-snapshot"));
    assert!(summary.contains(";format=postcard"));

    let custom = OptimizerCompatibility::new(
        Fingerprint::from_bytes([4; 32]),
        PrivateStatePolicy::ExplicitSnapshot {
            schema: Fingerprint::from_bytes([5; 32]),
            format: StateFormat::Custom("binary-gepa".to_owned()),
        },
    );
    assert!(optimizer_summary(Some(&custom)).contains(";format=binary-gepa"));
}

#[test]
fn compare_manifest_reports_first_differing_lm_role() {
    let mut stored = manifest_with_roles(BTreeMap::from([
        (
            "reflect".to_owned(),
            RuntimeFingerprint::new(Fingerprint::from_bytes([1; 32])),
        ),
        (
            "judge".to_owned(),
            RuntimeFingerprint::new(Fingerprint::from_bytes([2; 32])),
        ),
    ]));
    let live = manifest_with_roles(BTreeMap::from([(
        "reflect".to_owned(),
        RuntimeFingerprint::new(Fingerprint::from_bytes([1; 32])),
    )]));

    let error = compare_manifests(&stored, &live)
        .expect_err("missing live role must reject resume compatibility");
    assert!(matches!(
        error,
        ResumeCompatibilityError::LmRoleFingerprintMismatch {
            role,
            stored: Some(_),
            live: None,
        } if role == "judge"
    ));

    stored.lm_roles.insert(
        "judge".to_owned(),
        RuntimeFingerprint::new(Fingerprint::from_bytes([7; 32])),
    );
    let mut live = live;
    live.lm_roles.insert(
        "judge".to_owned(),
        RuntimeFingerprint::new(Fingerprint::from_bytes([8; 32])),
    );
    let error = compare_manifests(&stored, &live)
        .expect_err("changed live role must reject resume compatibility");
    assert!(matches!(
        error,
        ResumeCompatibilityError::LmRoleFingerprintMismatch {
            role,
            stored: Some(_),
            live: Some(_),
        } if role == "judge"
    ));
}

#[test]
fn manifest_cache_and_budget_are_derived_from_typed_inputs() {
    let seeded_policy = CachePolicy::DeterministicWithSeed(7);
    let deterministic_policy = CachePolicy::Deterministic;
    let three_call_budget = Budget::metric_calls(3);
    let unlimited_budget = Budget::unlimited();
    let deterministic_seed = manifest_with_policy_and_budget(&seeded_policy, &three_call_budget);
    let deterministic = manifest_with_policy_and_budget(&deterministic_policy, &three_call_budget);
    let unlimited = manifest_with_policy_and_budget(&seeded_policy, &unlimited_budget);

    assert!(
        deterministic_seed
            .cache
            .starts_with("cache:evaluation-policy-json:")
    );
    assert!(deterministic_seed.budget.starts_with("budget:limit-json:"));
    assert_ne!(deterministic_seed.cache, deterministic.cache);
    assert_ne!(deterministic_seed.budget, unlimited.budget);
}

#[test]
fn optimize_problem_shape_is_not_a_placeholder_identity() {
    let shape = optimize_problem_shape();
    let mut legacy = FingerprintBuilder::new();
    legacy.update(b"leaven-run.problem-placeholder.v1");

    assert_ne!(shape, legacy.finish());
}

#[test]
fn atomic_manifest_write_rejects_paths_without_file_names() {
    let error = write_atomic(Path::new(""), b"manifest")
        .expect_err("compatibility manifest writes require a file path");

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(error.to_string(), "path has no file name");
}

#[test]
fn compare_manifest_reports_missing_manifest_read_error() {
    let run_dir = std::env::temp_dir().join(format!(
        "leaven-run-compatibility-missing-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&run_dir).unwrap();
    let manifest = manifest_with_roles(BTreeMap::new());

    let error = compare_stored_manifest(&run_dir, &manifest)
        .expect_err("missing compatibility manifest must reject resume");

    assert!(matches!(error, ResumeCompatibilityError::Read { .. }));
    std::fs::remove_dir_all(run_dir).unwrap();
}

fn manifest_with_roles(lm_roles: BTreeMap<String, RuntimeFingerprint>) -> RunCompatibilityManifest {
    let cache_policy = CachePolicy::Never;
    let budget = Budget::unlimited();
    RunCompatibilityManifest::new(RunCompatibilityInputs {
        dataset: DatasetCompatibility {
            content: Fingerprint::from_bytes([9; 32]),
            splits: Fingerprint::from_bytes([10; 32]),
            case_set_version: "cases-v1".to_owned(),
        },
        runner: RuntimeFingerprint::new(Fingerprint::from_bytes([11; 32])),
        scorer: RuntimeFingerprint::new(Fingerprint::from_bytes([12; 32])),
        evaluator: RuntimeFingerprint::new(Fingerprint::from_bytes([13; 32])),
        optimizer: None,
        lm_roles,
        cache_policy: &cache_policy,
        budget: &budget,
    })
}

fn manifest_with_policy_and_budget(
    cache_policy: &CachePolicy,
    budget: &Budget,
) -> RunCompatibilityManifest {
    RunCompatibilityManifest::new(RunCompatibilityInputs {
        dataset: DatasetCompatibility {
            content: Fingerprint::from_bytes([9; 32]),
            splits: Fingerprint::from_bytes([10; 32]),
            case_set_version: "cases-v1".to_owned(),
        },
        runner: RuntimeFingerprint::new(Fingerprint::from_bytes([11; 32])),
        scorer: RuntimeFingerprint::new(Fingerprint::from_bytes([12; 32])),
        evaluator: RuntimeFingerprint::new(Fingerprint::from_bytes([13; 32])),
        optimizer: None,
        lm_roles: BTreeMap::new(),
        cache_policy,
        budget,
    })
}
