use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use futures::executor::block_on;
use leaven_core::{
    Artifact, ArtifactIdentity, AssessmentGranularity, EvaluationPurpose, EvaluationRequest,
    EvaluationSet,
};
use leaven_engine::{
    Callback, CheckpointContext, CheckpointError, CheckpointableOptimizer, Optimizer,
    OptimizerCompatibility, OptimizerError, OptimizerStateWrite, PrivateStatePolicy,
    RestoreContext, RunCheckpointRequest, RunContext, RunEvent, RunGraphView, RunPersistence,
    RunPersistenceError, StateFormat, StepStatus,
};
use leaven_eval::Case;
use leaven_evidence::CaseAssessmentEvidence;
use leaven_kernel::{Budget, CandidateId, CaseId, ContentId, EvaluatorId, Fingerprint, RunId};
use leaven_run::{
    CachePolicy, EvaluationCacheBackend, EvaluationCacheBypassReason, OptimizationStopReason,
    OptimizeBuilder, OptimizeError, OptimizeStore, ResumeCompatibilityError, RunCase,
    RunEventSummary, RunNotResumableReason, RunOutput, RunProblem, RunResumability, RunStorage,
    Score, ScoreContext, ScoreError, default_local_run_dir, optimize,
};
use leaven_store::{EvidenceStore, StoreError};
use leaven_store_inline::InlineEvidenceStore;
use rusqlite::Connection;

const TEST_RUNNER_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([7; 32]);
const TEST_SCORER_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([8; 32]);
const ALT_RUNNER_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([9; 32]);
const ALT_SCORER_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([10; 32]);

trait TestRuntimeFingerprints {
    fn test_runtime_fingerprints(self) -> Self;
}

impl<A, I, T, O> TestRuntimeFingerprints for OptimizeBuilder<A, I, T, O>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    fn test_runtime_fingerprints(self) -> Self {
        self.runner_fingerprint(TEST_RUNNER_FINGERPRINT)
            .scorer_fingerprint(TEST_SCORER_FINGERPRINT)
    }
}

#[test]
fn run_builder_requires_explicit_budget() {
    let error = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap_err();

    assert!(matches!(error, OptimizeError::MissingBudget));
}

#[test]
fn run_builder_accepts_explicit_unlimited_budget() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .evaluation_parallelism(NonZeroUsize::new(1).unwrap())
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(result.best(), Some(&TextArtifact(40)));
    assert_eq!(result.stop, OptimizationStopReason::OptimizerDone);
    assert_resumable_storage(result.summary().storage.clone(), result.run_id);
    cleanup_result_storage(&result.summary().storage);
}

#[test]
fn run_builder_uses_supplied_fresh_run_id_for_default_durable_dir() {
    let run_id = RunId::new();
    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .run_id(run_id)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(result.run_id, run_id);
    match result.summary().storage.clone() {
        RunStorage::Stored {
            run_id: stored_run,
            run_dir: Some(run_dir),
            latest_checkpoint: Some(_),
            resumability: RunResumability::Resumable,
        } => {
            assert_eq!(stored_run, run_id);
            assert_eq!(run_dir, default_local_run_dir(run_id));
        }
        other => panic!("expected default durable run-dir storage, got {other:?}"),
    }
    cleanup_result_storage(&result.summary().storage);
}

#[test]
fn run_builder_ephemeral_is_the_explicit_throwaway_path() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .ephemeral()
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(
        result.summary().storage,
        RunStorage::Ephemeral {
            run_id: result.run_id
        }
    );
    assert_eq!(
        result.summary().cache.evaluation.backend,
        EvaluationCacheBackend::InMemory
    );
    assert!(!result.summary().cache.evaluation.durable);
    assert!(
        result
            .summary()
            .cache
            .evaluation
            .bypasses
            .iter()
            .any(|summary| summary.reason == EvaluationCacheBypassReason::DisabledByPolicy)
    );
}

#[test]
fn run_builder_run_dir_writes_discoverable_durable_artifacts() {
    let run_dir = temp_run_dir("explicit-run-dir");
    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    match result.summary().storage.clone() {
        RunStorage::Stored {
            run_id,
            run_dir: Some(stored_dir),
            latest_checkpoint: Some(_),
            resumability: RunResumability::Resumable,
        } => {
            assert_eq!(run_id, result.run_id);
            assert_eq!(stored_dir, run_dir);
        }
        other => panic!("expected resumable run-dir storage, got {other:?}"),
    }
    assert!(run_dir.join("blobs").is_dir());
    assert!(run_dir.join("checkpoints").join("LATEST").is_file());
    assert!(run_dir.join("compatibility.json").is_file());
    assert!(run_dir.join("evidence").is_dir());
    let summary_json = run_dir.join("reports").join("summary.json");
    assert!(summary_json.is_file());
    assert!(run_dir.join("run.sqlite").is_file());
    assert_eq!(
        result.summary().reports.summary_json.as_deref(),
        Some(summary_json.as_path())
    );
    let summary_payload: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&summary_json).unwrap()).unwrap();
    assert_eq!(
        summary_payload["storage"]["Stored"]["run_dir"].as_str(),
        Some(run_dir.to_string_lossy().as_ref())
    );
    assert_eq!(
        summary_payload["reports"]["summary_json"].as_str(),
        Some(summary_json.to_string_lossy().as_ref())
    );
    assert_eq!(
        summary_payload["cache"]["evaluation"]["backend"],
        "SqliteRunStore"
    );
    let compatibility = result
        .summary()
        .compatibility
        .as_ref()
        .expect("durable run reports compatibility summary");
    assert_eq!(compatibility.schema, "leaven-run.compatibility.v1");
    assert_eq!(compatibility.run_kind, "leaven-run.optimize");
    assert_eq!(compatibility.lm_role_count, 0);
    assert_eq!(
        result.summary().cache.evaluation.backend,
        EvaluationCacheBackend::SqliteRunStore
    );
    assert!(result.summary().cache.evaluation.durable);
    assert_eq!(result.summary().cache.evaluation.hits, 0);
    assert_eq!(result.summary().cache.evaluation.misses, 0);
    assert_eq!(result.summary().cache.evaluation.write_errors, 0);
    assert!(result.summary().cache.evaluation.hit_cost_zero);
    assert!(
        result
            .summary()
            .cache
            .evaluation
            .bypasses
            .iter()
            .all(|summary| summary.reason != EvaluationCacheBypassReason::DisabledByPolicy)
    );
    assert!(result.summary().storage.is_resumable());
    cleanup_path(&run_dir);
}

#[test]
fn run_builder_run_dir_reports_blocked_summary_directory() {
    let run_dir = temp_run_dir("blocked-summary-dir");
    std::fs::create_dir_all(&run_dir).unwrap();
    std::fs::write(run_dir.join("reports"), b"not a directory").unwrap();

    let error = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        OptimizeError::ReportStore {
            operation: "create report directory",
            ..
        }
    ));
    cleanup_path(&run_dir);
}

#[test]
fn run_builder_run_dir_reports_blocked_summary_file() {
    let run_dir = temp_run_dir("blocked-summary-file");
    std::fs::create_dir_all(run_dir.join("reports").join("summary.json")).unwrap();

    let error = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        OptimizeError::ReportStore {
            operation: "write summary json",
            ..
        }
    ));
    cleanup_path(&run_dir);
}

#[test]
fn run_builder_run_dir_reports_blocked_compatibility_manifest() {
    let run_dir = temp_run_dir("blocked-compatibility-manifest");
    std::fs::create_dir_all(run_dir.join("compatibility.json")).unwrap();

    let error = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        OptimizeError::CompatibilityStore {
            operation: "write compatibility manifest",
            ..
        }
    ));
    cleanup_path(&run_dir);
}

#[test]
fn run_builder_run_dir_reports_corrupt_sqlite_eval_cache_on_resume() {
    let run_dir = temp_run_dir("corrupt-eval-cache-resume");
    let first = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();
    assert!(first.summary().storage.is_resumable());

    let connection = Connection::open(run_dir.join("run.sqlite")).unwrap();
    connection
        .execute("DROP TABLE evaluation_cache_entries", [])
        .unwrap();
    drop(connection);

    let error = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        OptimizeError::EvaluationCache {
            operation: "load sqlite evaluation cache",
            ..
        }
    ));
    cleanup_path(&run_dir);
}

#[test]
fn run_builder_run_dir_refuses_missing_or_malformed_compatibility_manifest_on_resume() {
    enum ManifestMutation {
        Missing,
        Malformed,
    }
    for (name, mutate, expected) in [
        (
            "missing-compatibility-manifest",
            ManifestMutation::Missing,
            "read",
        ),
        (
            "malformed-compatibility-manifest",
            ManifestMutation::Malformed,
            "decode",
        ),
    ] {
        let run_dir = temp_run_dir(name);
        block_on(
            optimize(TextArtifact(40))
                .train_inputs(vec![TextCase(2)])
                .runner(|artifact, case| async move { text_runner(&artifact, &case) })
                .score(text_score)
                .using(ResumeOnce::new(Arc::new(AtomicUsize::new(0))))
                .budget(Budget::metric_calls(1))
                .run_dir(&run_dir)
                .test_runtime_fingerprints()
                .run(),
        )
        .unwrap();

        match mutate {
            ManifestMutation::Missing => {
                std::fs::remove_file(run_dir.join("compatibility.json")).unwrap();
            }
            ManifestMutation::Malformed => {
                std::fs::write(run_dir.join("compatibility.json"), b"{not-json").unwrap();
            }
        }

        let error = block_on(
            optimize(TextArtifact(40))
                .train_inputs(vec![TextCase(2)])
                .runner(|artifact, case| async move { text_runner(&artifact, &case) })
                .score(text_score)
                .using(ResumeOnce::new(Arc::new(AtomicUsize::new(0))))
                .budget(Budget::metric_calls(1))
                .run_dir(&run_dir)
                .test_runtime_fingerprints()
                .run(),
        )
        .unwrap_err();

        assert!(
            matches!(
                error,
                OptimizeError::ResumeCompatibility(ref source)
                    if source.to_string().contains(expected)
            ),
            "expected {expected} compatibility error, got {error:?}"
        );
        cleanup_path(&run_dir);
    }
}

#[test]
fn run_builder_run_dir_refuses_evaluator_cache_and_budget_compatibility_drift() {
    enum ManifestDrift {
        Evaluator,
        Optimizer,
        LmRole,
        Cache,
        Budget,
    }
    for (name, drift, expected) in [
        (
            "evaluator-compatibility-drift",
            ManifestDrift::Evaluator,
            "evaluator fingerprint",
        ),
        (
            "cache-compatibility-drift",
            ManifestDrift::Cache,
            "cache compatibility",
        ),
        (
            "optimizer-compatibility-drift",
            ManifestDrift::Optimizer,
            "optimizer compatibility",
        ),
        (
            "lm-role-compatibility-drift",
            ManifestDrift::LmRole,
            "LM role `solver` fingerprint",
        ),
        (
            "budget-compatibility-drift",
            ManifestDrift::Budget,
            "budget compatibility",
        ),
    ] {
        let run_dir = temp_run_dir(name);
        block_on(
            optimize(TextArtifact(40))
                .train_inputs(vec![TextCase(2)])
                .runner(|artifact, case| async move { text_runner(&artifact, &case) })
                .score(text_score)
                .using(ResumeOnce::new(Arc::new(AtomicUsize::new(0))))
                .budget(Budget::metric_calls(1))
                .run_dir(&run_dir)
                .lm_role_fingerprint("solver", TEST_RUNNER_FINGERPRINT)
                .test_runtime_fingerprints()
                .run(),
        )
        .unwrap();

        let path = run_dir.join("compatibility.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        match drift {
            ManifestDrift::Evaluator => {
                manifest["evaluator"]["fingerprint"] =
                    serde_json::to_value(Fingerprint::from_bytes([1; 32])).unwrap();
            }
            ManifestDrift::Optimizer => {
                manifest["optimizer"] = serde_json::to_value(OptimizerCompatibility::new(
                    Fingerprint::from_bytes([2; 32]),
                    PrivateStatePolicy::DerivedFromGraph,
                ))
                .unwrap();
            }
            ManifestDrift::LmRole => {
                manifest["lm_roles"]["solver"]["fingerprint"] =
                    serde_json::to_value(Fingerprint::from_bytes([3; 32])).unwrap();
            }
            ManifestDrift::Cache => {
                manifest["cache"] = serde_json::json!("cache:changed");
            }
            ManifestDrift::Budget => {
                manifest["budget"] = serde_json::json!("budget:changed");
            }
        }
        std::fs::write(&path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();

        let error = block_on(
            optimize(TextArtifact(40))
                .train_inputs(vec![TextCase(2)])
                .runner(|artifact, case| async move { text_runner(&artifact, &case) })
                .score(text_score)
                .using(ResumeOnce::new(Arc::new(AtomicUsize::new(0))))
                .budget(Budget::metric_calls(1))
                .run_dir(&run_dir)
                .lm_role_fingerprint("solver", TEST_RUNNER_FINGERPRINT)
                .test_runtime_fingerprints()
                .run(),
        )
        .unwrap_err();

        assert!(
            matches!(
                error,
                OptimizeError::ResumeCompatibility(ref source)
                    if source.to_string().contains(expected)
            ),
            "expected {expected} compatibility error, got {error:?}"
        );
        cleanup_path(&run_dir);
    }
}

#[test]
fn run_builder_run_dir_resume_preserves_existing_budget_and_graph_state() {
    let run_dir = temp_run_dir("resume-existing-run");
    let first_steps = Arc::new(AtomicUsize::new(0));
    let first = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(ResumeOnce::new(Arc::clone(&first_steps)))
            .budget(Budget::metric_calls(1))
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();
    assert_eq!(first_steps.load(Ordering::SeqCst), 1);

    let restored_steps = Arc::new(AtomicUsize::new(0));
    let restored = block_on(
        optimize(TextArtifact(999))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(ResumeOnce::new(Arc::clone(&restored_steps)))
            .budget(Budget::metric_calls(1))
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(restored.run_id, first.run_id);
    assert_eq!(restored_steps.load(Ordering::SeqCst), 0);
    assert_eq!(
        restored.summary().optimization_budget.spent.metric_calls,
        first.summary().optimization_budget.spent.metric_calls
    );
    assert!(restored.events.len() > first.events.len());
    cleanup_path(&run_dir);
}

#[test]
fn run_builder_refuses_runner_fingerprint_mismatch_before_runner_call() {
    let run_dir = temp_run_dir("resume-runner-mismatch");
    block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(ResumeOnce::new(Arc::new(AtomicUsize::new(0))))
            .budget(Budget::metric_calls(1))
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    let runner_calls = Arc::new(AtomicUsize::new(0));
    let error = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner({
                let runner_calls = Arc::clone(&runner_calls);
                move |artifact, case| {
                    let runner_calls = Arc::clone(&runner_calls);
                    async move {
                        runner_calls.fetch_add(1, Ordering::SeqCst);
                        text_runner(&artifact, &case)
                    }
                }
            })
            .score(text_score)
            .using(ResumeOnce::new(Arc::new(AtomicUsize::new(0))))
            .budget(Budget::metric_calls(1))
            .run_dir(&run_dir)
            .runner_fingerprint(ALT_RUNNER_FINGERPRINT)
            .scorer_fingerprint(TEST_SCORER_FINGERPRINT)
            .run(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        OptimizeError::ResumeCompatibility(source)
            if matches!(
                source.as_ref(),
                ResumeCompatibilityError::RunnerFingerprintMismatch { .. }
            )
    ));
    assert_eq!(runner_calls.load(Ordering::SeqCst), 0);
    cleanup_path(&run_dir);
}

#[test]
fn run_builder_refuses_scorer_fingerprint_mismatch_before_scorer_call() {
    let run_dir = temp_run_dir("resume-scorer-mismatch");
    block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(ResumeOnce::new(Arc::new(AtomicUsize::new(0))))
            .budget(Budget::metric_calls(1))
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    let scorer_calls = Arc::new(AtomicUsize::new(0));
    let error = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score({
                let scorer_calls = Arc::clone(&scorer_calls);
                move |ctx| {
                    let scorer_calls = Arc::clone(&scorer_calls);
                    async move {
                        scorer_calls.fetch_add(1, Ordering::SeqCst);
                        text_score(ctx).await
                    }
                }
            })
            .using(ResumeOnce::new(Arc::new(AtomicUsize::new(0))))
            .budget(Budget::metric_calls(1))
            .run_dir(&run_dir)
            .runner_fingerprint(TEST_RUNNER_FINGERPRINT)
            .scorer_fingerprint(ALT_SCORER_FINGERPRINT)
            .run(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        OptimizeError::ResumeCompatibility(source)
            if matches!(
                source.as_ref(),
                ResumeCompatibilityError::ScorerFingerprintMismatch { .. }
            )
    ));
    assert_eq!(scorer_calls.load(Ordering::SeqCst), 0);
    cleanup_path(&run_dir);
}

#[test]
fn run_builder_refuses_case_content_or_split_identity_mismatch() {
    let run_dir = temp_run_dir("resume-case-mismatch");
    block_on(
        optimize(TextArtifact(40))
            .train(vec![Case::targeted(
                CaseId::new(77),
                TextCase(2),
                TextTarget(42),
            )])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(
                |ctx: ScoreContext<TextArtifact, TextCase, TextTarget>| async move {
                    Ok(Score::new(
                        f64::from(u8::from(
                            ctx.output.output == ctx.case.target().unwrap().0.to_string(),
                        )),
                        "ok",
                    ))
                },
            )
            .using(TargetSeedBest::default())
            .budget(Budget::metric_calls(1))
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    let error = block_on(
        optimize(TextArtifact(40))
            .train(vec![Case::targeted(
                CaseId::new(77),
                TextCase(2),
                TextTarget(43),
            )])
            .validation(vec![Case::targeted(
                CaseId::new(78),
                TextCase(3),
                TextTarget(43),
            )])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(
                |ctx: ScoreContext<TextArtifact, TextCase, TextTarget>| async move {
                    Ok(Score::new(
                        f64::from(u8::from(
                            ctx.output.output == ctx.case.target().unwrap().0.to_string(),
                        )),
                        "ok",
                    ))
                },
            )
            .using(TargetSeedBest::default())
            .budget(Budget::metric_calls(1))
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        OptimizeError::ResumeCompatibility(source)
            if matches!(
                source.as_ref(),
                ResumeCompatibilityError::DatasetFingerprintMismatch { .. }
            )
    ));
    cleanup_path(&run_dir);
}

#[test]
fn run_builder_missing_runtime_fingerprint_refuses_durable_but_not_ephemeral() {
    let run_dir = temp_run_dir("missing-runtime-fingerprint");
    let runner_calls = Arc::new(AtomicUsize::new(0));
    let error = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner({
                let runner_calls = Arc::clone(&runner_calls);
                move |artifact, case| {
                    let runner_calls = Arc::clone(&runner_calls);
                    async move {
                        runner_calls.fetch_add(1, Ordering::SeqCst);
                        text_runner(&artifact, &case)
                    }
                }
            })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::metric_calls(1))
            .run_dir(&run_dir)
            .run(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        OptimizeError::RuntimeFingerprintMissing { runtime: "runner" }
    ));
    assert_eq!(runner_calls.load(Ordering::SeqCst), 0);

    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::metric_calls(1))
            .ephemeral()
            .run(),
    )
    .unwrap();
    assert!(matches!(
        result.summary().storage,
        RunStorage::Ephemeral { .. }
    ));
    cleanup_path(&run_dir);
}

#[test]
fn run_builder_missing_scorer_fingerprint_refuses_durable_before_runner_call() {
    let run_dir = temp_run_dir("missing-scorer-fingerprint");
    let runner_calls = Arc::new(AtomicUsize::new(0));
    let error = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner({
                let runner_calls = Arc::clone(&runner_calls);
                move |artifact, case| {
                    let runner_calls = Arc::clone(&runner_calls);
                    async move {
                        runner_calls.fetch_add(1, Ordering::SeqCst);
                        text_runner(&artifact, &case)
                    }
                }
            })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::metric_calls(1))
            .run_dir(&run_dir)
            .runner_fingerprint(TEST_RUNNER_FINGERPRINT)
            .run(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        OptimizeError::RuntimeFingerprintMissing { runtime: "scorer" }
    ));
    assert_eq!(runner_calls.load(Ordering::SeqCst), 0);
    cleanup_path(&run_dir);
}

#[test]
fn public_stop_reason_preserves_all_engine_stop_variants() {
    let cases = [
        (
            leaven_engine::StopReason::OptimizerDone,
            OptimizationStopReason::OptimizerDone,
        ),
        (
            leaven_engine::StopReason::BudgetReached,
            OptimizationStopReason::BudgetReached,
        ),
        (
            leaven_engine::StopReason::BudgetExceeded,
            OptimizationStopReason::BudgetExceeded,
        ),
        (
            leaven_engine::StopReason::StopperTriggered,
            OptimizationStopReason::StopperTriggered,
        ),
        (
            leaven_engine::StopReason::External,
            OptimizationStopReason::External,
        ),
        (
            leaven_engine::StopReason::Error,
            OptimizationStopReason::Error,
        ),
    ];

    for (engine_reason, public_reason) in cases {
        assert_eq!(OptimizationStopReason::from(engine_reason), public_reason);
    }
}

#[test]
fn public_storage_and_cache_names_cover_all_status_variants() {
    let run_id = RunId::new();
    let missing_checkpoint = RunStorage::Stored {
        run_id,
        run_dir: Some(PathBuf::from("/tmp/leaven-missing-checkpoint")),
        latest_checkpoint: None,
        resumability: RunResumability::NotResumable {
            reason: RunNotResumableReason::MissingLatestCheckpoint,
        },
    };
    let explicit_store = RunStorage::Stored {
        run_id,
        run_dir: None,
        latest_checkpoint: None,
        resumability: RunResumability::NotResumable {
            reason: RunNotResumableReason::ExplicitStoreWithoutLocalRunDir,
        },
    };
    let resumable = RunStorage::Stored {
        run_id,
        run_dir: Some(PathBuf::from("/tmp/leaven-resumable")),
        latest_checkpoint: Some(leaven_kernel::CheckpointId::new()),
        resumability: RunResumability::Resumable,
    };

    assert!(!RunStorage::Ephemeral { run_id }.is_resumable());
    assert!(!missing_checkpoint.is_resumable());
    assert!(!explicit_store.is_resumable());
    assert!(resumable.is_resumable());
    assert_eq!(
        RunNotResumableReason::MissingLatestCheckpoint.as_str(),
        "missing_latest_checkpoint"
    );
    assert_eq!(
        RunNotResumableReason::ExplicitStoreWithoutLocalRunDir.as_str(),
        "explicit_store_without_local_run_dir"
    );

    let backends = [
        (EvaluationCacheBackend::SqliteRunStore, "sqlite-run-store"),
        (
            EvaluationCacheBackend::CheckpointedRunStore,
            "checkpointed-run-store",
        ),
        (EvaluationCacheBackend::InMemory, "in-memory"),
    ];
    for (backend, expected) in backends {
        assert_eq!(backend.as_str(), expected);
    }

    let bypasses = [
        (
            EvaluationCacheBypassReason::DisabledByPolicy,
            "disabled_by_policy",
        ),
        (
            EvaluationCacheBypassReason::CacheUnavailable,
            "cache_unavailable",
        ),
        (
            EvaluationCacheBypassReason::MissingCandidateIdentity,
            "missing_candidate_identity",
        ),
    ];
    for (reason, expected) in bypasses {
        assert_eq!(reason.as_str(), expected);
    }
}

#[test]
fn run_event_summary_names_cover_public_variants() {
    let cases = [
        (RunEventSummary::OptimizationStarted, "optimization_started"),
        (RunEventSummary::IterationStarted, "iteration_started"),
        (RunEventSummary::BudgetCharged, "budget_charged"),
        (
            RunEventSummary::ProposalBatchProduced,
            "proposal_batch_produced",
        ),
        (RunEventSummary::ProposalRecorded, "proposal_recorded"),
        (
            RunEventSummary::StageAttemptRecorded,
            "stage_attempt_recorded",
        ),
        (RunEventSummary::ApplySucceeded, "apply_succeeded"),
        (RunEventSummary::ApplyFailed, "apply_failed"),
        (RunEventSummary::EvaluationRequested, "evaluation_requested"),
        (RunEventSummary::EvaluationCompleted, "evaluation_completed"),
        (RunEventSummary::PopulationUpdated, "population_updated"),
        (RunEventSummary::IterationEnded, "iteration_ended"),
        (
            RunEventSummary::OptimizationStopping,
            "optimization_stopping",
        ),
        (RunEventSummary::OptimizationEnded, "optimization_ended"),
        (RunEventSummary::Error, "error"),
    ];

    for (event, name) in cases {
        assert_eq!(event.as_str(), name);
    }
}

#[test]
fn run_builder_reports_final_train_scores_when_optimizer_does_not_evaluate_train() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .evaluation_parallelism(NonZeroUsize::new(1).unwrap())
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(result.summary().baseline_train_score, Some(42.0));
    assert_eq!(result.summary().optimized_train_score, Some(42.0));
    assert_eq!(result.report().splits_reported.len(), 1);
}

#[test]
fn run_builder_surfaces_final_evaluation_failures() {
    let error = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(|_ctx| async move { Err(ScoreError::new("final judge offline")) })
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .ephemeral()
            .run(),
    )
    .unwrap_err();

    assert!(
        error.to_string().contains("final evaluation failed"),
        "{error:?} / {error}"
    );
}

#[test]
fn run_builder_checkpoints_search_state_before_final_evaluation_failures() {
    let run_dir = temp_run_dir("resume-after-final-eval-failure");
    let first_scores = Arc::new(AtomicUsize::new(0));
    let first_error = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score({
                let first_scores = Arc::clone(&first_scores);
                move |ctx| {
                    let first_scores = Arc::clone(&first_scores);
                    async move {
                        let calls = first_scores.fetch_add(1, Ordering::SeqCst);
                        if calls == 0 {
                            text_score(ctx).await
                        } else {
                            Err(ScoreError::new("final judge offline"))
                        }
                    }
                }
            })
            .using(ResumeOnce::new(Arc::new(AtomicUsize::new(0))))
            .budget(Budget::metric_calls(1))
            .evaluation_cache_policy(CachePolicy::Never)
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap_err();

    assert!(
        first_error.to_string().contains("final evaluation failed"),
        "{first_error:?} / {first_error}"
    );

    let restored_steps = Arc::new(AtomicUsize::new(0));
    let restored = block_on(
        optimize(TextArtifact(999))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(ResumeOnce::new(Arc::clone(&restored_steps)))
            .budget(Budget::metric_calls(1))
            .evaluation_cache_policy(CachePolicy::Never)
            .run_dir(&run_dir)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(restored_steps.load(Ordering::SeqCst), 0);
    assert_eq!(restored.summary().optimization_budget.spent.metric_calls, 1);
    cleanup_path(&run_dir);
}

#[test]
fn run_builder_surfaces_report_evidence_lookup_failures() {
    let error = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .store(OptimizeStore::evidence(FailingGetEvidenceStore::default()))
            .run(),
    )
    .unwrap_err();

    assert!(
        error.to_string().contains("report evidence lookup failed"),
        "{error:?} / {error}"
    );
}

#[test]
fn run_builder_accepts_cloned_evidence_only_store() {
    let evidence_store = CountingEvidenceStore::new("builder-evidence-only");
    let store =
        OptimizeStore::<RunProblem<TextArtifact, TextCase>>::evidence(evidence_store.clone());
    let cloned_store = store.clone();

    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(EvaluateSeed::default())
            .budget(Budget::metric_calls(16))
            .store(store)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();
    let cloned_result = block_on(
        optimize(TextArtifact(41))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(EvaluateSeed::default())
            .budget(Budget::metric_calls(16))
            .store(cloned_store)
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(result.best(), Some(&TextArtifact(40)));
    assert_eq!(cloned_result.best(), Some(&TextArtifact(41)));
    assert!(evidence_store.puts() > 0);
    assert!(evidence_store.gets() > 0);
    cleanup_result_storage(&result.summary().storage);
}

#[test]
fn run_builder_rejects_held_out_cases_without_train_cases() {
    let error = block_on(
        optimize(TextArtifact(40))
            .train_inputs(Vec::<TextCase>::new())
            .validation_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::metric_calls(8))
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap_err();

    assert!(matches!(error, OptimizeError::HeldOutWithoutTrain));
}

#[test]
fn run_builder_accepts_empty_train_when_no_held_out_sets_exist() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(Vec::<TextCase>::new())
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(EvaluateSeed::default())
            .budget(Budget::metric_calls(8))
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(result.best(), Some(&TextArtifact(40)));
    assert_eq!(result.summary().baseline_train_score, None);
    assert_eq!(result.summary().optimized_train_score, None);
    assert_eq!(result.summary().optimization_cost.metric_calls, 0);
    assert_eq!(result.summary().final_report_cost.metric_calls, 0);
    cleanup_result_storage(&result.summary().storage);
}

#[test]
fn run_builder_separates_optimization_cost_from_final_report_cost() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .validation_inputs(vec![TextCase(3)])
            .test_inputs(vec![TextCase(4)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(EvaluateSeed::default())
            .budget(Budget::metric_calls(16))
            .evaluation_parallelism(NonZeroUsize::new(1).unwrap())
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(result.summary().optimization_budget.spent.metric_calls, 1);
    assert_eq!(result.summary().optimization_cost.metric_calls, 1);
    assert_eq!(result.summary().baseline_train_score, Some(42.0));
    assert_eq!(result.summary().optimized_train_score, Some(42.0));
    assert_eq!(result.summary().final_report_cost.metric_calls, 6);
    assert_eq!(result.budget.spent.metric_calls, 7);
    assert_eq!(result.summary().cost.metric_calls, 7);
    assert_eq!(result.summary().baseline_validation_score, Some(43.0));
    assert_eq!(result.summary().validation_score, Some(43.0));
    assert_eq!(result.summary().baseline_test_score, Some(44.0));
    assert_eq!(result.summary().test_score, Some(44.0));
    cleanup_result_storage(&result.summary().storage);
}

#[test]
fn run_builder_reports_budget_stop_reason_from_metric_call_budget() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(ContinueAfterSeedEvaluation::default())
            .budget(Budget::metric_calls(1))
            .evaluation_parallelism(NonZeroUsize::new(1).unwrap())
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(result.best(), Some(&TextArtifact(40)));
    assert_eq!(result.stop, OptimizationStopReason::BudgetReached);
    assert_eq!(result.summary().optimization_cost.metric_calls, 1);
    assert_eq!(result.summary().final_report_cost.metric_calls, 2);
    cleanup_result_storage(&result.summary().storage);
}

#[test]
fn run_builder_runs_final_reports_after_metric_budget_stop() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .validation_inputs(vec![TextCase(3)])
            .test_inputs(vec![TextCase(4)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(ContinueAfterSeedEvaluation::default())
            .budget(Budget::metric_calls(1))
            .evaluation_parallelism(NonZeroUsize::new(1).unwrap())
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(result.best(), Some(&TextArtifact(40)));
    assert_eq!(result.stop, OptimizationStopReason::BudgetReached);
    assert_eq!(result.summary().optimization_cost.metric_calls, 1);
    assert_eq!(result.summary().final_report_cost.metric_calls, 6);
    assert_eq!(result.summary().cost.metric_calls, 7);
    assert_eq!(result.summary().baseline_validation_score, Some(43.0));
    assert_eq!(result.summary().validation_score, Some(43.0));
    assert_eq!(result.summary().baseline_test_score, Some(44.0));
    assert_eq!(result.summary().test_score, Some(44.0));
    cleanup_result_storage(&result.summary().storage);
}

#[test]
fn run_builder_reports_case_ids_output_and_feedback_for_case_level_rows() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2), TextCase(3)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(EvaluateSeed::default())
            .budget(Budget::metric_calls(16))
            .evaluation_parallelism(NonZeroUsize::new(1).unwrap())
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    let train = result
        .summary()
        .evaluation
        .splits_reported
        .iter()
        .find(|split| split.partition.0 == "TRAIN")
        .expect("train split is reported");
    let candidate = train
        .candidates
        .iter()
        .find(|candidate| candidate.candidate == result.best_id().expect("best exists"))
        .expect("best candidate train summary exists");

    assert_eq!(candidate.average_score, Some(42.5));
    assert_eq!(candidate.assessments.len(), 2);
    assert_eq!(candidate.cases.len(), 2);
    assert_eq!(candidate.cases[0].case_id, CaseId::from_index(0));
    assert_eq!(candidate.cases[0].output, "42");
    assert_eq!(candidate.cases[0].feedback, "case 2");
    assert_eq!(candidate.cases[1].case_id, CaseId::from_index(1));
    assert_eq!(candidate.cases[1].output, "43");
    assert_eq!(candidate.cases[1].feedback, "case 3");
    cleanup_result_storage(&result.summary().storage);
}

#[test]
fn run_builder_preserves_case_envelope_ids_and_targets_score_only() {
    let runner_seen = Arc::new(Mutex::new(Vec::new()));
    let scorer_seen_target = Arc::new(Mutex::new(None));
    let result = block_on(
        optimize(TextArtifact(40))
            .train(vec![Case::targeted(
                CaseId::new(77),
                TextCase(2),
                TextTarget(42),
            )])
            .runner({
                let runner_seen = Arc::clone(&runner_seen);
                move |artifact, case: RunCase<TextCase>| {
                    let runner_seen = Arc::clone(&runner_seen);
                    async move {
                        runner_seen
                            .lock()
                            .unwrap()
                            .push((case.id(), case.input().0));
                        text_runner(&artifact, &case)
                    }
                }
            })
            .score({
                let scorer_seen_target = Arc::clone(&scorer_seen_target);
                move |ctx: ScoreContext<TextArtifact, TextCase, TextTarget>| {
                    let scorer_seen_target = Arc::clone(&scorer_seen_target);
                    async move {
                        let target = ctx.case.target().expect("target is scorer-visible");
                        *scorer_seen_target.lock().unwrap() = Some(target.0);
                        Ok(Score::new(
                            f64::from(u8::from(ctx.output.output == target.0.to_string())),
                            "target checked",
                        ))
                    }
                }
            })
            .using(TargetSeedBest::default())
            .budget(Budget::metric_calls(8))
            .ephemeral()
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(
        runner_seen.lock().unwrap().as_slice(),
        &[(CaseId::new(77), 2), (CaseId::new(77), 2)]
    );
    assert_eq!(*scorer_seen_target.lock().unwrap(), Some(42));
    let train = result
        .summary()
        .evaluation
        .splits_reported
        .iter()
        .find(|split| split.partition.0 == "TRAIN")
        .expect("train split is reported");
    let candidate = &train.candidates[0];
    assert_eq!(candidate.assessments.len(), 1);
    assert_eq!(candidate.cases[0].case_id, CaseId::new(77));
    assert_eq!(candidate.cases[0].feedback, "target checked");
}

#[test]
fn run_builder_reports_no_best_candidate_without_error() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(NoBest)
            .budget(Budget::metric_calls(8))
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(result.best_id(), None);
    assert_eq!(result.best(), None);
    assert_eq!(result.summary().baseline_train_score, Some(42.0));
    assert_eq!(result.summary().optimized_train_score, None);
    assert_eq!(result.summary().final_report_cost.metric_calls, 1);
    assert!(result.events.contains(&RunEventSummary::OptimizationEnded));
    cleanup_result_storage(&result.summary().storage);
}

#[test]
fn run_builder_dispatches_callbacks_and_supplied_store_capabilities() {
    let evidence_store = CountingEvidenceStore::new("builder-test");
    let persistence = CountingPersistence::default();
    let events = Arc::new(Mutex::new(Vec::new()));

    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2), TextCase(3)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(EvaluateSeed::default())
            .budget(Budget::metric_calls(32))
            .store(OptimizeStore::durable(
                evidence_store.clone(),
                persistence.clone(),
            ))
            .on_event(RecordingCallback {
                events: Arc::clone(&events),
            })
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    assert_eq!(result.best(), Some(&TextArtifact(40)));
    assert!(evidence_store.puts() > 0);
    assert!(evidence_store.gets() > 0);
    assert!(persistence.checkpoints() > 0);
    assert_eq!(
        result.summary().storage,
        RunStorage::Stored {
            run_id: result.run_id,
            run_dir: None,
            latest_checkpoint: None,
            resumability: RunResumability::NotResumable {
                reason: RunNotResumableReason::ExplicitStoreWithoutLocalRunDir,
            },
        }
    );
    let events = events.lock().unwrap();
    assert!(events.contains(&"optimization_started"));
    assert!(events.contains(&"optimization_ended"));
}

#[test]
fn run_builder_exposes_typed_optimizer_report_payload() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train_inputs(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBestWithReport::default())
            .budget(Budget::unlimited())
            .test_runtime_fingerprints()
            .run(),
    )
    .unwrap();

    let report = result
        .optimizer_report::<TestOptimizerReport>()
        .expect("optimizer report should downcast to the concrete report type");
    assert_eq!(report.label, "seed-best-report");
    assert_eq!(report.best, result.best_id());
    assert!(result.optimizer_report::<String>().is_none());
    cleanup_result_storage(&result.summary().storage);
}

fn assert_resumable_storage(storage: RunStorage, run_id: RunId) {
    match storage {
        RunStorage::Stored {
            run_id: stored_run,
            run_dir: Some(run_dir),
            latest_checkpoint: Some(_),
            resumability: RunResumability::Resumable,
        } => {
            assert_eq!(stored_run, run_id);
            assert!(run_dir.ends_with(run_id.to_string()));
            assert!(run_dir.join("checkpoints").join("LATEST").is_file());
        }
        other => panic!("expected default durable storage, got {other:?}"),
    }
}

fn cleanup_result_storage(storage: &RunStorage) {
    if let RunStorage::Stored {
        run_dir: Some(run_dir),
        ..
    } = storage
    {
        cleanup_path(run_dir);
    }
}

fn temp_run_dir(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("leaven-run-{label}-{}", RunId::new()));
    cleanup_path(&root);
    root
}

fn cleanup_path(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

#[derive(Default)]
struct SeedBest {
    best: Option<CandidateId>,
}

impl Optimizer<RunProblem<TextArtifact, TextCase>> for SeedBest {
    async fn initialize(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<(), OptimizerError> {
        self.best = ctx.graph().candidate_tree().roots().first().copied();
        Ok(())
    }

    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Option<CandidateId> {
        self.best
            .or_else(|| graph.candidate_tree().roots().first().copied())
    }
}

#[derive(Default)]
struct SeedBestWithReport {
    inner: SeedBest,
}

impl Optimizer<RunProblem<TextArtifact, TextCase>> for SeedBestWithReport {
    async fn initialize(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<(), OptimizerError> {
        self.inner.initialize(ctx).await
    }

    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<StepStatus, OptimizerError> {
        self.inner.step(ctx).await
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Option<CandidateId> {
        self.inner.best_candidate(graph)
    }

    fn optimizer_report(&self) -> Option<leaven_engine::OptimizerReportPayload> {
        Some(Arc::new(TestOptimizerReport {
            label: "seed-best-report",
            best: self.inner.best,
        }))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct TestOptimizerReport {
    label: &'static str,
    best: Option<CandidateId>,
}

#[derive(Default)]
struct TargetSeedBest {
    best: Option<CandidateId>,
}

impl Optimizer<RunProblem<TextArtifact, TextCase, TextTarget>> for TargetSeedBest {
    async fn initialize(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase, TextTarget>>,
    ) -> Result<(), OptimizerError> {
        self.best = ctx.graph().candidate_tree().roots().first().copied();
        Ok(())
    }

    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase, TextTarget>>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, RunProblem<TextArtifact, TextCase, TextTarget>>,
    ) -> Option<CandidateId> {
        self.best
            .or_else(|| graph.candidate_tree().roots().first().copied())
    }
}

struct NoBest;

impl Optimizer<RunProblem<TextArtifact, TextCase>> for NoBest {
    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: RunGraphView<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Option<CandidateId> {
        None
    }
}

#[derive(Default)]
struct ContinueAfterSeedEvaluation {
    best: Option<CandidateId>,
    evaluated: bool,
}

impl Optimizer<RunProblem<TextArtifact, TextCase>> for ContinueAfterSeedEvaluation {
    async fn initialize(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<(), OptimizerError> {
        self.best = ctx.graph().candidate_tree().roots().first().copied();
        Ok(())
    }

    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<StepStatus, OptimizerError> {
        if self.evaluated {
            return Ok(StepStatus::Continue);
        }
        self.evaluated = true;
        let seed = self
            .best
            .or_else(|| ctx.graph().candidate_tree().roots().first().copied())
            .ok_or_else(|| OptimizerError::Message("missing seed".to_owned()))?;
        ctx.evaluate(
            EvaluatorId::PRIMARY,
            EvaluationRequest::Independent {
                candidates: vec![seed],
                set: EvaluationSet::Partition("TRAIN".into()),
                granularity: AssessmentGranularity::PerCase,
                purpose: EvaluationPurpose::Search,
            },
        )
        .await
        .map_err(|source| OptimizerError::with_source("seed evaluation failed", source))?;
        Ok(StepStatus::Continue)
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Option<CandidateId> {
        self.best
            .or_else(|| graph.candidate_tree().roots().first().copied())
    }
}

#[derive(Default)]
struct EvaluateSeed {
    best: Option<CandidateId>,
}

impl Optimizer<RunProblem<TextArtifact, TextCase>> for EvaluateSeed {
    async fn initialize(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<(), OptimizerError> {
        self.best = ctx.graph().candidate_tree().roots().first().copied();
        Ok(())
    }

    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<StepStatus, OptimizerError> {
        let seed = self
            .best
            .or_else(|| ctx.graph().candidate_tree().roots().first().copied())
            .ok_or_else(|| OptimizerError::Message("missing seed".to_owned()))?;
        ctx.evaluate(
            EvaluatorId::PRIMARY,
            EvaluationRequest::Independent {
                candidates: vec![seed],
                set: EvaluationSet::Partition("TRAIN".into()),
                granularity: AssessmentGranularity::PerCase,
                purpose: EvaluationPurpose::Search,
            },
        )
        .await
        .map_err(|source| OptimizerError::with_source("seed evaluation failed", source))?;
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Option<CandidateId> {
        self.best
            .or_else(|| graph.candidate_tree().roots().first().copied())
    }
}

struct ResumeOnce {
    best: Option<CandidateId>,
    evaluated: bool,
    step_calls: Arc<AtomicUsize>,
}

const RESUME_ONCE_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([41; 32]);
const RESUME_ONCE_STATE_SCHEMA: Fingerprint = Fingerprint::from_bytes([42; 32]);

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct ResumeOnceState {
    best: Option<CandidateId>,
    evaluated: bool,
}

impl ResumeOnce {
    fn new(step_calls: Arc<AtomicUsize>) -> Self {
        Self {
            best: None,
            evaluated: false,
            step_calls,
        }
    }
}

impl Optimizer<RunProblem<TextArtifact, TextCase>> for ResumeOnce {
    async fn initialize(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<(), OptimizerError> {
        self.best = ctx.graph().candidate_tree().roots().first().copied();
        Ok(())
    }

    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<StepStatus, OptimizerError> {
        self.step_calls.fetch_add(1, Ordering::SeqCst);
        if self.evaluated {
            return Ok(StepStatus::Continue);
        }
        self.evaluated = true;
        let seed = self
            .best
            .or_else(|| ctx.graph().candidate_tree().roots().first().copied())
            .ok_or_else(|| OptimizerError::Message("missing seed".to_owned()))?;
        ctx.evaluate(
            EvaluatorId::PRIMARY,
            EvaluationRequest::Independent {
                candidates: vec![seed],
                set: EvaluationSet::Partition("TRAIN".into()),
                granularity: AssessmentGranularity::PerCase,
                purpose: EvaluationPurpose::Search,
            },
        )
        .await
        .map_err(|source| OptimizerError::with_source("seed evaluation failed", source))?;
        Ok(StepStatus::Continue)
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Option<CandidateId> {
        self.best
            .or_else(|| graph.candidate_tree().roots().first().copied())
    }

    fn checkpoint_state_write(
        &self,
        ctx: CheckpointContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<Option<OptimizerStateWrite>, OptimizerError> {
        <Self as CheckpointableOptimizer<RunProblem<TextArtifact, TextCase>>>::checkpoint_state_write(
            self, ctx,
        )
    }

    fn restore_checkpoint_state<R>(
        &mut self,
        checkpoint: &leaven_engine::RunCheckpoint,
        reader: &R,
        ctx: RestoreContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<(), OptimizerError>
    where
        R: leaven_engine::OptimizerStateReader,
    {
        leaven_engine::restore_checkpointable_optimizer_state(self, checkpoint, reader, ctx)
    }
}

impl CheckpointableOptimizer<RunProblem<TextArtifact, TextCase>> for ResumeOnce {
    type State = ResumeOnceState;

    fn optimizer_fingerprint(&self) -> Fingerprint {
        RESUME_ONCE_FINGERPRINT
    }

    fn private_state_policy(&self) -> PrivateStatePolicy {
        PrivateStatePolicy::ExplicitSnapshot {
            schema: RESUME_ONCE_STATE_SCHEMA,
            format: StateFormat::Json,
        }
    }

    fn checkpoint_state(
        &self,
        ctx: CheckpointContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<Self::State, CheckpointError> {
        if let Some(best) = self.best
            && ctx.graph().candidate(best).is_none()
        {
            return Err(CheckpointError::MissingGraphTruth {
                reason: format!("best candidate `{best}` is not in the graph"),
            });
        }
        Ok(ResumeOnceState {
            best: self.best,
            evaluated: self.evaluated,
        })
    }

    fn restore_state(
        &mut self,
        state: Self::State,
        ctx: RestoreContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<(), CheckpointError> {
        if let Some(best) = state.best
            && ctx.graph().candidate(best).is_none()
        {
            return Err(CheckpointError::MissingGraphTruth {
                reason: format!("best candidate `{best}` is not in the graph"),
            });
        }
        self.best = state.best;
        self.evaluated = state.evaluated;
        Ok(())
    }
}

#[derive(Clone)]
struct CountingEvidenceStore {
    inner: Arc<CountingEvidenceInner>,
}

struct CountingEvidenceInner {
    store: InlineEvidenceStore<CaseAssessmentEvidence>,
    puts: AtomicUsize,
    gets: AtomicUsize,
}

impl CountingEvidenceStore {
    fn new(name: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(CountingEvidenceInner {
                store: InlineEvidenceStore::new(name),
                puts: AtomicUsize::new(0),
                gets: AtomicUsize::new(0),
            }),
        }
    }

    fn puts(&self) -> usize {
        self.inner.puts.load(Ordering::SeqCst)
    }

    fn gets(&self) -> usize {
        self.inner.gets.load(Ordering::SeqCst)
    }
}

impl EvidenceStore<CaseAssessmentEvidence> for CountingEvidenceStore {
    fn put(
        &self,
        evidence: CaseAssessmentEvidence,
    ) -> Result<leaven_kernel::EvidenceRef, StoreError> {
        self.inner.puts.fetch_add(1, Ordering::SeqCst);
        self.inner.store.put(evidence)
    }

    fn get(
        &self,
        reference: &leaven_kernel::EvidenceRef,
    ) -> Result<CaseAssessmentEvidence, StoreError> {
        self.inner.gets.fetch_add(1, Ordering::SeqCst);
        self.inner.store.get(reference)
    }
}

struct FailingGetEvidenceStore {
    inner: InlineEvidenceStore<CaseAssessmentEvidence>,
}

impl Default for FailingGetEvidenceStore {
    fn default() -> Self {
        Self {
            inner: InlineEvidenceStore::new("failing-get"),
        }
    }
}

impl EvidenceStore<CaseAssessmentEvidence> for FailingGetEvidenceStore {
    fn put(
        &self,
        evidence: CaseAssessmentEvidence,
    ) -> Result<leaven_kernel::EvidenceRef, StoreError> {
        self.inner.put(evidence)
    }

    fn get(
        &self,
        _reference: &leaven_kernel::EvidenceRef,
    ) -> Result<CaseAssessmentEvidence, StoreError> {
        Err(StoreError::OperationFailed {
            store: "failing-get".to_owned(),
            operation: "get evidence",
            reason: "synthetic lookup refusal".to_owned(),
            retryable: Some(false),
        })
    }
}

#[derive(Clone, Default)]
struct CountingPersistence {
    checkpoints: Arc<AtomicUsize>,
}

impl CountingPersistence {
    fn checkpoints(&self) -> usize {
        self.checkpoints.load(Ordering::SeqCst)
    }
}

impl RunPersistence<RunProblem<TextArtifact, TextCase>> for CountingPersistence {
    fn checkpoint(
        &self,
        _request: RunCheckpointRequest<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<(), RunPersistenceError> {
        self.checkpoints.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct RecordingCallback {
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl Callback<RunProblem<TextArtifact, TextCase>> for RecordingCallback {
    fn on_event(
        &mut self,
        event: &RunEvent,
        _graph: RunGraphView<'_, RunProblem<TextArtifact, TextCase>>,
    ) {
        let name = match event {
            RunEvent::OptimizationStarted { .. } => "optimization_started",
            RunEvent::OptimizationEnded { .. } => "optimization_ended",
            _ => "other",
        };
        self.events.lock().unwrap().push(name);
    }
}

fn text_runner(artifact: &TextArtifact, case: &RunCase<TextCase>) -> RunOutput {
    RunOutput::new((artifact.0 + case.input().0).to_string())
}

#[allow(clippy::needless_pass_by_value)]
async fn text_score(ctx: ScoreContext<TextArtifact, TextCase>) -> Result<Score, ScoreError> {
    let ScoreContext { case, output, .. } = ctx;
    let value = output.output.parse::<f64>().unwrap();
    Ok(Score::new(value, format!("case {}", case.input().0)))
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
struct TextArtifact(i32);

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
struct TextCase(i32);

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
struct TextTarget(i32);

#[derive(Debug)]
struct TextArtifactError;

impl std::fmt::Display for TextArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("text artifact error")
    }
}

impl std::error::Error for TextArtifactError {}

impl Artifact for TextArtifact {
    type Change = i32;
    type ApplyError = TextArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        let mut bytes = [0; ContentId::BYTES];
        bytes[..std::mem::size_of::<i32>()].copy_from_slice(&self.0.to_le_bytes());
        ArtifactIdentity::Content(ContentId::from_bytes(bytes))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(*change))
    }
}
