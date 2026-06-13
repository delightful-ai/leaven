mod support;

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use bytes::Bytes;
use futures::executor::block_on;
use leaven_core::{
    AssessmentGranularity, CacheIdentity, CaseSetVersion, EvaluationPurpose, PartitionId, Proposal,
};
use leaven_engine::{
    CacheIndexSnapshot, CachePolicy, Callback, CaseSet, CheckpointContext, CheckpointError,
    CheckpointableOptimizer, Engine, EvaluationCache, EvaluationCacheKey,
    EvaluationCacheRequestKind, EvaluationCacheSnapshot, GraphSnapshotRef, Optimizer,
    OptimizerError, OptimizerStateSnapshot, OptimizerStateWrite, PrivateStatePolicy,
    RestoreContext, RunCheckpoint, RunCheckpointRequest, RunContext, RunEvent, RunGraph,
    RunGraphSnapshot, RunGraphView, RunPersistence, RunPersistenceError, StateFormat, StepStatus,
    StopReason, Stopper, StoreRunPersistence, TrustPolicy, optimize,
    restore_checkpointable_optimizer_state,
};
use leaven_kernel::{
    AssessmentId, BlobRef, Budget, CandidateId, CaseId, ContentId, Cost, ErrorKind, Fingerprint,
    RunId, StageId, now,
};
use leaven_store::{BlobStore, BlobWrite, CheckpointBytes, CheckpointStore, StoreError};
use leaven_store_inline::InlineEvidenceStore;

use support::{TestEvidence, TestProblem, TextArtifact, graph_and_budget, record_one};

#[test]
fn engine_getters_expose_read_only_state() {
    let mut engine = Engine::<TestProblem>::builder().build();

    let seed = engine
        .insert_seed(TextArtifact("seed".to_owned()), 0)
        .unwrap();

    let _ = engine.graph();
    let _ = engine.budget();
    assert_eq!(engine.view().candidate(seed).unwrap().id(), seed);
}

#[test]
fn engine_continues_until_optimizer_reports_done() {
    block_on(async {
        let mut engine = Engine::<TestProblem>::builder().build();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = ContinueThenDone { steps: 0 };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(optimizer.steps, 2);
        assert!(result.best.is_none());
        assert_eq!(
            engine
                .view()
                .events()
                .filter(|event| matches!(event, RunEvent::IterationStarted { .. }))
                .count(),
            2
        );
    });
}

#[test]
fn optimize_builder_wires_budget_and_callbacks() {
    block_on(async {
        let seen = Arc::new(AtomicUsize::new(0));
        let callback = CountingCallback { seen: seen.clone() };
        let mut engine = optimize::<TestProblem>()
            .budget(Budget::metric_calls(7))
            .callback(callback)
            .build();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = ContinueThenDone { steps: 0 };

        engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert!(seen.load(Ordering::SeqCst) > 0);
        assert_eq!(engine.budget().snapshot().limit.metric_calls, Some(7));
    });
}

#[test]
fn engine_builder_stopper_stops_cleanly_before_first_step_with_current_best() {
    block_on(async {
        let mut engine = Engine::<TestProblem>::builder()
            .stopper(StopImmediately)
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = StatefulOptimizer {
            selected: Some(seed),
            cursor: 0,
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
        assert!(engine.view().events().any(|event| matches!(
            event,
            RunEvent::OptimizationStopping {
                reason: StopReason::StopperTriggered,
            }
        )));
        assert!(
            !engine
                .view()
                .events()
                .any(|event| matches!(event, RunEvent::IterationStarted { .. }))
        );
        assert!(
            !engine
                .view()
                .events()
                .any(|event| matches!(event, RunEvent::Error { .. }))
        );
    });
}

#[test]
fn metric_call_budget_stopper_stops_before_next_step_without_budget_error() {
    block_on(async {
        let mut engine = Engine::<TestProblem>::builder()
            .budget(Budget::metric_calls(1))
            .metric_call_budget_stopper(1)
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = ChargeMetricThenContinue {
            best: Some(seed),
            steps: 0,
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(optimizer.steps, 1);
        assert_eq!(result.best, Some(seed));
        assert_eq!(engine.budget().snapshot().spent.metric_calls, 1);
        assert_eq!(
            engine
                .view()
                .events()
                .filter(|event| matches!(event, RunEvent::IterationStarted { .. }))
                .count(),
            1
        );
        assert!(engine.view().events().any(|event| matches!(
            event,
            RunEvent::OptimizationStopping {
                reason: StopReason::BudgetReached,
            }
        )));
        assert!(
            !engine
                .view()
                .events()
                .any(|event| matches!(event, RunEvent::Error { .. }))
        );
    });
}

#[test]
fn engine_notifies_optimizer_when_budget_stop_prevents_next_step() {
    block_on(async {
        let stopped = Arc::new(Mutex::new(Vec::new()));
        let mut engine = Engine::<TestProblem>::builder()
            .budget(Budget::metric_calls(1))
            .metric_call_budget_stopper(1)
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = StopNotifiedAfterCharge {
            best: Some(seed),
            steps: 0,
            stopped: Arc::clone(&stopped),
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
        assert_eq!(optimizer.steps, 1);
        assert_eq!(*stopped.lock().unwrap(), vec![StopReason::BudgetReached]);
    });
}

#[test]
fn metric_budget_hard_guard_stops_as_budget_not_optimizer_error() {
    block_on(async {
        let mut engine = Engine::<TestProblem>::builder()
            .budget(Budget::metric_calls(0))
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = ChargeMetricThenContinue {
            best: Some(seed),
            steps: 0,
        };

        let err = engine
            .run(&mut optimizer, &cases, &store)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("metric charge failed"));
        assert_eq!(optimizer.steps, 0);
        assert_eq!(engine.budget().snapshot().spent.metric_calls, 0);
        assert!(engine.view().events().any(|event| matches!(
            event,
            RunEvent::OptimizationStopping {
                reason: StopReason::BudgetExceeded,
            }
        )));
        assert!(!engine.view().events().any(|event| matches!(
            event,
            RunEvent::OptimizationStopping {
                reason: StopReason::Error,
            }
        )));
        assert!(engine.view().events().any(|event| matches!(
            event,
            RunEvent::Error { error, .. } if error.kind == ErrorKind::Budget
        )));
        assert!(!engine.view().events().any(|event| matches!(
            event,
            RunEvent::Error { error, .. } if error.kind == ErrorKind::Optimizer
        )));
    });
}

#[test]
fn optimizer_can_request_clean_budget_stop_mid_step() {
    block_on(async {
        let mut engine = Engine::<TestProblem>::builder()
            .budget(Budget::metric_calls(10))
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = StopWithReason {
            best: Some(seed),
            reason: StopReason::BudgetReached,
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(result.best, Some(seed));
        assert!(engine.view().events().any(|event| matches!(
            event,
            RunEvent::OptimizationStopping {
                reason: StopReason::BudgetReached,
            }
        )));
        assert!(
            !engine
                .view()
                .events()
                .any(|event| matches!(event, RunEvent::Error { .. }))
        );
    });
}

#[test]
fn engine_trust_policy_reaches_optimizer_context() {
    block_on(async {
        let secret = PartitionId::from("secret");
        let mut engine = Engine::<TestProblem>::builder()
            .trust_policy(TrustPolicy::default().hide_from_optimizers([secret.clone()]))
            .build();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = TrustInspectingOptimizer {
            expected_hidden: secret,
            observed_hidden: false,
        };

        engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert!(optimizer.observed_hidden);
    });
}

#[test]
fn engine_checkpoints_clean_run_boundaries() {
    block_on(async {
        let checkpoints = Arc::new(AtomicUsize::new(0));
        let cache_present = Arc::new(AtomicUsize::new(0));
        let persistence = CountingPersistence {
            checkpoints: checkpoints.clone(),
            cache_present: Some(cache_present.clone()),
            cache_absent: None,
        };
        let mut engine = Engine::<TestProblem>::builder()
            .persistence(persistence)
            .build();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = ContinueThenDone { steps: 0 };

        engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(checkpoints.load(Ordering::SeqCst), 4);
        assert_eq!(cache_present.load(Ordering::SeqCst), 4);
    });
}

#[test]
fn engine_checkpoints_explicit_optimizer_state_at_clean_run_boundaries() {
    block_on(async {
        let optimizer_state_presence = Arc::new(Mutex::new(Vec::new()));
        let persistence = OptimizerStatePresencePersistence {
            optimizer_state_presence: Arc::clone(&optimizer_state_presence),
        };
        let mut engine = Engine::<TestProblem>::builder()
            .persistence(persistence)
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = StatefulOptimizer {
            selected: Some(seed),
            cursor: 7,
        };

        engine.run(&mut optimizer, &cases, &store).await.unwrap();

        let calls = optimizer_state_presence.lock().unwrap().clone();
        assert_eq!(
            calls,
            vec![(false, false), (true, true), (true, true), (true, true)]
        );
    });
}

#[test]
fn engine_error_checkpoints_do_not_advance_latest_boundary() {
    block_on(async {
        let checkpoint_flags = Arc::new(Mutex::new(Vec::new()));
        let persistence = OptimizerStatePresencePersistence {
            optimizer_state_presence: Arc::clone(&checkpoint_flags),
        };
        let mut engine = Engine::<TestProblem>::builder()
            .persistence(persistence)
            .build();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = FailingStep;

        let err = engine
            .run(&mut optimizer, &cases, &store)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("step failed"));

        let calls = checkpoint_flags.lock().unwrap().clone();
        assert_eq!(calls, vec![(false, true), (false, false)]);
    });
}

#[test]
fn engine_restores_explicit_optimizer_state_from_stored_run_checkpoint() {
    block_on(async {
        let store = RecordingStore::new("recording");
        let persistence = StoreRunPersistence::new(store);
        let mut engine = Engine::<TestProblem>::builder()
            .persistence(persistence.clone())
            .build();
        let seed = engine
            .insert_seed(TextArtifact("seed".to_owned()), 0)
            .unwrap();
        let cases = CaseSet::new(vec!["case"]);
        let evidence = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = StatefulOptimizer {
            selected: Some(seed),
            cursor: 7,
        };

        engine.run(&mut optimizer, &cases, &evidence).await.unwrap();

        let restored = persistence
            .latest_checkpoint::<TestProblem>()
            .unwrap()
            .unwrap();
        let state: StatefulOptimizerState = persistence
            .load_optimizer_state(
                &restored.checkpoint,
                STATEFUL_OPTIMIZER_FINGERPRINT,
                STATEFUL_OPTIMIZER_STATE_SCHEMA,
            )
            .unwrap()
            .expect("ordinary run checkpoint should include optimizer state");
        let mut restored_graph = restored.graph;
        let mut restored_budget = restored.budget;
        let restored_ctx =
            RunContext::<TestProblem>::new(&mut restored_graph, &mut restored_budget);
        let mut resumed = StatefulOptimizer {
            selected: None,
            cursor: 0,
        };
        resumed
            .restore_state(state, RestoreContext::new(restored_ctx.graph()))
            .unwrap();

        assert_eq!(resumed.selected, Some(seed));
        assert_eq!(resumed.cursor, 7);
    });
}

#[test]
fn store_run_persistence_writes_graph_cache_and_checkpoint_envelope() {
    let store = RecordingStore::new("recording");
    let persistence = StoreRunPersistence::new(store.clone());
    let (mut graph, mut budget) = graph_and_budget();
    let seed = {
        let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget);
        ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap()
    };
    let mut cache = EvaluationCache::default();
    cache.insert(
        EvaluationCacheKey {
            evaluator: Fingerprint::from_bytes([3; 32]),
            policy: CachePolicy::Deterministic,
            kind: EvaluationCacheRequestKind::Independent,
            granularity: AssessmentGranularity::Aggregate,
            purpose: EvaluationPurpose::Search,
            case_set_version: CaseSetVersion("v1".to_owned()),
            case_ids: vec![CaseId::new(0)],
            candidates: vec![CacheIdentity::Content(ContentId::from_bytes([4; 32]))],
        },
        vec![AssessmentId::new()],
    );

    persistence
        .checkpoint(
            RunCheckpointRequest::new(&graph, &budget, Some(&cache))
                .with_optimizer_state(
                    OptimizerStateWrite::json(
                        Fingerprint::from_bytes([5; 32]),
                        Fingerprint::from_bytes([6; 32]),
                        &StatefulOptimizerState {
                            selected: Some(seed),
                            cursor: 42,
                        },
                    )
                    .unwrap(),
                )
                .advance_latest(),
        )
        .unwrap();

    let checkpoint: RunCheckpoint =
        serde_json::from_slice(&store.latest_checkpoint().unwrap().0).unwrap();
    let graph_bytes = BlobStore::get(&store, &checkpoint.graph_snapshot.bytes).unwrap();
    let graph_snapshot: RunGraphSnapshot<TestProblem> =
        serde_json::from_slice(&graph_bytes).unwrap();
    let mut restored = RunGraph::<TestProblem>::from_snapshot(graph_snapshot).unwrap();
    let mut restored_budget = leaven_engine::BudgetLedger::new(Budget::unlimited());
    let restored_ctx = RunContext::<TestProblem>::new(&mut restored, &mut restored_budget);
    assert!(restored_ctx.graph().candidate(seed).is_some());

    let cache_ref = checkpoint.cache_index.as_ref().unwrap();
    let cache_bytes = BlobStore::get(&store, &cache_ref.bytes).unwrap();
    let cache_snapshot: EvaluationCacheSnapshot = serde_json::from_slice(&cache_bytes).unwrap();
    assert_eq!(cache_snapshot.entries.len(), 1);
    assert!(checkpoint.optimizer_state.is_some());

    let restored = persistence
        .latest_checkpoint::<TestProblem>()
        .unwrap()
        .unwrap();
    let restored_state: StatefulOptimizerState = persistence
        .load_optimizer_state(
            &restored.checkpoint,
            Fingerprint::from_bytes([5; 32]),
            Fingerprint::from_bytes([6; 32]),
        )
        .unwrap()
        .unwrap();
    assert_eq!(restored_state.cursor, 42);
    assert_eq!(restored_state.selected, Some(seed));
    let mut restored_graph = restored.graph;
    let mut restored_budget = restored.budget;
    let restored_ctx = RunContext::<TestProblem>::new(&mut restored_graph, &mut restored_budget);
    assert!(restored_ctx.graph().candidate(seed).is_some());
    assert_eq!(restored.cache.unwrap().len(), 1);
}

#[test]
fn store_run_persistence_keeps_latest_on_clean_resume_boundary() {
    let store = RecordingStore::new("recording");
    let persistence = StoreRunPersistence::new(store.clone());
    let (graph, budget) = graph_and_budget();

    persistence
        .checkpoint(
            RunCheckpointRequest::new(&graph, &budget, None)
                .with_optimizer_state(
                    OptimizerStateWrite::json(
                        Fingerprint::from_bytes([5; 32]),
                        Fingerprint::from_bytes([6; 32]),
                        &StatefulOptimizerState {
                            selected: None,
                            cursor: 42,
                        },
                    )
                    .unwrap(),
                )
                .advance_latest(),
        )
        .unwrap();
    let latest_before = CheckpointStore::latest(&store).unwrap();
    let mut cache = EvaluationCache::default();
    cache.insert(
        EvaluationCacheKey {
            evaluator: Fingerprint::from_bytes([3; 32]),
            policy: CachePolicy::Deterministic,
            kind: EvaluationCacheRequestKind::Independent,
            granularity: AssessmentGranularity::Aggregate,
            purpose: EvaluationPurpose::Search,
            case_set_version: CaseSetVersion("v1".to_owned()),
            case_ids: vec![CaseId::new(0)],
            candidates: vec![CacheIdentity::Content(ContentId::from_bytes([4; 32]))],
        },
        vec![AssessmentId::new()],
    );

    persistence
        .checkpoint(RunCheckpointRequest::new(&graph, &budget, Some(&cache)))
        .unwrap();

    assert_eq!(CheckpointStore::latest(&store).unwrap(), latest_before);
    let graph_only_checkpoint = store
        .checkpoints()
        .into_iter()
        .find(|(id, _)| Some(*id) != latest_before)
        .map(|(_, bytes)| bytes)
        .expect("graph/cache checkpoint should still be written");
    let graph_only_checkpoint: RunCheckpoint =
        serde_json::from_slice(&graph_only_checkpoint.0).unwrap();
    let cache_ref = graph_only_checkpoint.cache_index.as_ref().unwrap();
    let cache_bytes = BlobStore::get(&store, &cache_ref.bytes).unwrap();
    let cache_snapshot: EvaluationCacheSnapshot = serde_json::from_slice(&cache_bytes).unwrap();
    assert_eq!(cache_snapshot.entries.len(), 1);

    let restored = persistence
        .latest_checkpoint::<TestProblem>()
        .unwrap()
        .unwrap();
    let restored_state: StatefulOptimizerState = persistence
        .load_optimizer_state(
            &restored.checkpoint,
            Fingerprint::from_bytes([5; 32]),
            Fingerprint::from_bytes([6; 32]),
        )
        .unwrap()
        .unwrap();
    assert_eq!(restored_state.cursor, 42);
}

#[test]
fn store_run_persistence_reports_absent_and_corrupt_checkpoints_explicitly() {
    let store = RecordingStore::new("recording");
    let persistence = StoreRunPersistence::new(store.clone());

    assert!(
        persistence
            .latest_checkpoint::<TestProblem>()
            .unwrap()
            .is_none()
    );

    let corrupt = CheckpointStore::put(
        &store,
        CheckpointBytes(Bytes::from_static(b"not a checkpoint")),
    )
    .unwrap();
    CheckpointStore::mark_latest(&store, corrupt).unwrap();
    let err = latest_checkpoint_err(&persistence);
    assert!(matches!(
        err,
        RunPersistenceError::Serialization {
            state: "run checkpoint envelope",
            ..
        }
    ));
}

#[test]
fn store_run_persistence_reports_missing_and_corrupt_referenced_blobs() {
    let missing_graph_store = RecordingStore::new("recording");
    let persistence = StoreRunPersistence::new(missing_graph_store.clone());
    let checkpoint = checkpoint_referencing_graph(BlobRef {
        store: "recording".to_owned(),
        key: "missing".to_owned(),
    });
    let missing_graph = CheckpointStore::put(
        &missing_graph_store,
        CheckpointBytes(Bytes::from(serde_json::to_vec(&checkpoint).unwrap())),
    )
    .unwrap();
    CheckpointStore::mark_latest(&missing_graph_store, missing_graph).unwrap();
    let err = latest_checkpoint_err(&persistence);
    assert!(matches!(
        err,
        RunPersistenceError::Store {
            operation: "read graph snapshot blob",
            ..
        }
    ));

    let corrupt_graph_store = RecordingStore::new("recording");
    let graph_ref = BlobStore::put(
        &corrupt_graph_store,
        BlobWrite {
            bytes: Bytes::from_static(b"{not graph json"),
            content_type: Some("application/json".to_owned()),
        },
    )
    .unwrap();
    let persistence = StoreRunPersistence::new(corrupt_graph_store.clone());
    let corrupt_graph = CheckpointStore::put(
        &corrupt_graph_store,
        CheckpointBytes(Bytes::from(
            serde_json::to_vec(&checkpoint_referencing_graph(graph_ref)).unwrap(),
        )),
    )
    .unwrap();
    CheckpointStore::mark_latest(&corrupt_graph_store, corrupt_graph).unwrap();
    let err = latest_checkpoint_err(&persistence);
    assert!(matches!(
        err,
        RunPersistenceError::Serialization {
            state: "graph snapshot",
            ..
        }
    ));
}

#[test]
fn store_run_persistence_reports_missing_and_corrupt_cache_indexes() {
    let store = RecordingStore::new("recording");
    let persistence = StoreRunPersistence::new(store);
    let (graph, budget) = graph_and_budget();
    persistence
        .checkpoint(RunCheckpointRequest::new(&graph, &budget, None).advance_latest())
        .unwrap();
    let mut checkpoint: RunCheckpoint =
        serde_json::from_slice(&persistence.store().latest_checkpoint().unwrap().0).unwrap();
    checkpoint.cache_index = Some(CacheIndexSnapshot {
        schema: Fingerprint::from_bytes([12; 32]),
        format: StateFormat::Json,
        bytes: BlobRef {
            store: "recording".to_owned(),
            key: "missing-cache".to_owned(),
        },
    });
    let missing_cache = CheckpointStore::put(
        persistence.store(),
        CheckpointBytes(Bytes::from(serde_json::to_vec(&checkpoint).unwrap())),
    )
    .unwrap();
    CheckpointStore::mark_latest(persistence.store(), missing_cache).unwrap();

    let missing = latest_checkpoint_err(&persistence);
    assert!(matches!(
        missing,
        RunPersistenceError::Store {
            operation: "read evaluation cache blob",
            ..
        }
    ));

    let corrupt_cache = BlobStore::put(
        persistence.store(),
        BlobWrite {
            bytes: Bytes::from_static(b"{not cache json"),
            content_type: Some("application/json".to_owned()),
        },
    )
    .unwrap();
    checkpoint.cache_index = Some(CacheIndexSnapshot {
        schema: Fingerprint::from_bytes([12; 32]),
        format: StateFormat::Json,
        bytes: corrupt_cache,
    });
    let corrupt_cache_checkpoint = CheckpointStore::put(
        persistence.store(),
        CheckpointBytes(Bytes::from(serde_json::to_vec(&checkpoint).unwrap())),
    )
    .unwrap();
    CheckpointStore::mark_latest(persistence.store(), corrupt_cache_checkpoint).unwrap();

    let corrupt = latest_checkpoint_err(&persistence);
    assert!(matches!(
        corrupt,
        RunPersistenceError::Serialization {
            state: "evaluation cache index",
            ..
        }
    ));
}

#[test]
fn store_run_persistence_validates_optimizer_state_identity_and_format() {
    let store = RecordingStore::new("recording");
    let persistence = StoreRunPersistence::new(store.clone());
    let (graph, budget) = graph_and_budget();
    persistence
        .checkpoint(RunCheckpointRequest::new(&graph, &budget, None).advance_latest())
        .unwrap();
    let mut restored = persistence
        .latest_checkpoint::<TestProblem>()
        .unwrap()
        .unwrap()
        .checkpoint;
    assert!(
        persistence
            .load_optimizer_state::<StatefulOptimizerState>(
                &restored,
                Fingerprint::from_bytes([5; 32]),
                Fingerprint::from_bytes([6; 32]),
            )
            .unwrap()
            .is_none()
    );

    let state_ref = BlobStore::put(
        &store,
        BlobWrite {
            bytes: Bytes::from_static(br#"{"selected":null,"cursor":9}"#),
            content_type: Some("application/json".to_owned()),
        },
    )
    .unwrap();
    restored.optimizer_state = Some(OptimizerStateSnapshot {
        optimizer: Fingerprint::from_bytes([5; 32]),
        schema: Fingerprint::from_bytes([6; 32]),
        format: StateFormat::Json,
        bytes: state_ref.clone(),
    });
    let ok: StatefulOptimizerState = persistence
        .load_optimizer_state(
            &restored,
            Fingerprint::from_bytes([5; 32]),
            Fingerprint::from_bytes([6; 32]),
        )
        .unwrap()
        .unwrap();
    assert_eq!(ok.cursor, 9);

    let wrong_optimizer = persistence
        .load_optimizer_state::<StatefulOptimizerState>(
            &restored,
            Fingerprint::from_bytes([7; 32]),
            Fingerprint::from_bytes([6; 32]),
        )
        .unwrap_err();
    assert!(matches!(
        wrong_optimizer,
        RunPersistenceError::IncompatibleState {
            state: "optimizer state",
            ..
        }
    ));

    let wrong_schema = persistence
        .load_optimizer_state::<StatefulOptimizerState>(
            &restored,
            Fingerprint::from_bytes([5; 32]),
            Fingerprint::from_bytes([8; 32]),
        )
        .unwrap_err();
    assert!(matches!(
        wrong_schema,
        RunPersistenceError::IncompatibleState {
            state: "optimizer state",
            ..
        }
    ));

    restored.optimizer_state = Some(OptimizerStateSnapshot {
        optimizer: Fingerprint::from_bytes([5; 32]),
        schema: Fingerprint::from_bytes([6; 32]),
        format: StateFormat::Postcard,
        bytes: state_ref,
    });
    let wrong_format = persistence
        .load_optimizer_state::<StatefulOptimizerState>(
            &restored,
            Fingerprint::from_bytes([5; 32]),
            Fingerprint::from_bytes([6; 32]),
        )
        .unwrap_err();
    assert!(matches!(
        wrong_format,
        RunPersistenceError::IncompatibleState {
            state: "optimizer state",
            ..
        }
    ));
}

#[test]
fn store_run_persistence_reports_corrupt_optimizer_state_payload() {
    let store = RecordingStore::new("recording");
    let persistence = StoreRunPersistence::new(store.clone());
    let (graph, budget) = graph_and_budget();
    persistence
        .checkpoint(RunCheckpointRequest::new(&graph, &budget, None).advance_latest())
        .unwrap();
    let mut restored = persistence
        .latest_checkpoint::<TestProblem>()
        .unwrap()
        .unwrap()
        .checkpoint;

    let corrupt_ref = BlobStore::put(
        &store,
        BlobWrite {
            bytes: Bytes::from_static(b"{not optimizer json"),
            content_type: Some("application/json".to_owned()),
        },
    )
    .unwrap();
    restored.optimizer_state = Some(OptimizerStateSnapshot {
        optimizer: Fingerprint::from_bytes([5; 32]),
        schema: Fingerprint::from_bytes([6; 32]),
        format: StateFormat::Json,
        bytes: corrupt_ref,
    });
    let corrupt_payload = persistence
        .load_optimizer_state::<StatefulOptimizerState>(
            &restored,
            Fingerprint::from_bytes([5; 32]),
            Fingerprint::from_bytes([6; 32]),
        )
        .unwrap_err();
    assert!(matches!(
        corrupt_payload,
        RunPersistenceError::Serialization {
            state: "optimizer state",
            ..
        }
    ));
}

#[test]
fn store_run_persistence_reports_missing_optimizer_state_payload() {
    let store = RecordingStore::new("recording");
    let persistence = StoreRunPersistence::new(store);
    let (graph, budget) = graph_and_budget();
    persistence
        .checkpoint(RunCheckpointRequest::new(&graph, &budget, None).advance_latest())
        .unwrap();
    let mut restored = persistence
        .latest_checkpoint::<TestProblem>()
        .unwrap()
        .unwrap()
        .checkpoint;
    restored.optimizer_state = Some(OptimizerStateSnapshot {
        optimizer: Fingerprint::from_bytes([5; 32]),
        schema: Fingerprint::from_bytes([6; 32]),
        format: StateFormat::Json,
        bytes: BlobRef {
            store: "recording".to_owned(),
            key: "missing-optimizer".to_owned(),
        },
    });

    let missing = persistence
        .load_optimizer_state::<StatefulOptimizerState>(
            &restored,
            Fingerprint::from_bytes([5; 32]),
            Fingerprint::from_bytes([6; 32]),
        )
        .unwrap_err();
    assert!(matches!(
        missing,
        RunPersistenceError::Store {
            operation: "read optimizer state blob",
            ..
        }
    ));
}

#[test]
fn store_run_persistence_reports_store_read_refusals() {
    let latest_failure = StoreRunPersistence::new(FaultyStore::new("recording").fail_latest());
    let latest = latest_checkpoint_err(&latest_failure);
    assert!(matches!(
        latest,
        RunPersistenceError::Store {
            operation: "read latest checkpoint pointer",
            ..
        }
    ));

    let get_failure = StoreRunPersistence::new(FaultyStore::new("recording").fail_checkpoint_get());
    let Err(missing_envelope) =
        get_failure.load_checkpoint::<TestProblem>(leaven_kernel::CheckpointId::new())
    else {
        panic!("checkpoint load unexpectedly succeeded");
    };
    assert!(matches!(
        missing_envelope,
        RunPersistenceError::Store {
            operation: "read checkpoint envelope",
            ..
        }
    ));
}

#[test]
fn store_run_persistence_reports_store_write_refusals() {
    let (graph, budget) = graph_and_budget();

    let graph_blob_failure =
        StoreRunPersistence::new(FaultyStore::new("recording").fail_blob_put_on(1));
    let graph_err = graph_blob_failure
        .checkpoint(RunCheckpointRequest::new(&graph, &budget, None))
        .unwrap_err();
    assert!(matches!(
        graph_err,
        RunPersistenceError::Store {
            operation: "write checkpoint blob",
            ..
        }
    ));

    let optimizer_blob_failure =
        StoreRunPersistence::new(FaultyStore::new("recording").fail_blob_put_on(2));
    let optimizer_err = optimizer_blob_failure
        .checkpoint(
            RunCheckpointRequest::new(&graph, &budget, None).with_optimizer_state(
                OptimizerStateWrite::json(
                    Fingerprint::from_bytes([5; 32]),
                    Fingerprint::from_bytes([6; 32]),
                    &StatefulOptimizerState {
                        selected: None,
                        cursor: 42,
                    },
                )
                .unwrap(),
            ),
        )
        .unwrap_err();
    assert!(matches!(
        optimizer_err,
        RunPersistenceError::Store {
            operation: "write optimizer state blob",
            ..
        }
    ));

    let mut cache = EvaluationCache::default();
    cache.insert(
        EvaluationCacheKey {
            evaluator: Fingerprint::from_bytes([3; 32]),
            policy: CachePolicy::Deterministic,
            kind: EvaluationCacheRequestKind::Independent,
            granularity: AssessmentGranularity::Aggregate,
            purpose: EvaluationPurpose::Search,
            case_set_version: CaseSetVersion("v1".to_owned()),
            case_ids: vec![CaseId::new(0)],
            candidates: vec![CacheIdentity::Content(ContentId::from_bytes([4; 32]))],
        },
        vec![AssessmentId::new()],
    );
    let cache_blob_failure =
        StoreRunPersistence::new(FaultyStore::new("recording").fail_blob_put_on(2));
    let cache_err = cache_blob_failure
        .checkpoint(RunCheckpointRequest::new(&graph, &budget, Some(&cache)))
        .unwrap_err();
    assert!(matches!(
        cache_err,
        RunPersistenceError::Store {
            operation: "write checkpoint blob",
            ..
        }
    ));

    let envelope_failure =
        StoreRunPersistence::new(FaultyStore::new("recording").fail_checkpoint_put());
    let envelope_err = envelope_failure
        .checkpoint(RunCheckpointRequest::new(&graph, &budget, None))
        .unwrap_err();
    assert!(matches!(
        envelope_err,
        RunPersistenceError::Store {
            operation: "write checkpoint envelope",
            ..
        }
    ));

    let latest_failure = StoreRunPersistence::new(FaultyStore::new("recording").fail_mark_latest());
    let latest_err = latest_failure
        .checkpoint(RunCheckpointRequest::new(&graph, &budget, None).advance_latest())
        .unwrap_err();
    assert!(matches!(
        latest_err,
        RunPersistenceError::Store {
            operation: "advance latest checkpoint pointer",
            ..
        }
    ));
}

#[test]
fn engine_surfaces_checkpoint_failures_as_run_errors() {
    block_on(async {
        let mut engine = Engine::<TestProblem>::builder()
            .persistence(FailingPersistence)
            .build();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = ContinueThenDone { steps: 0 };

        let err = engine
            .run(&mut optimizer, &cases, &store)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("run checkpoint failed"));
        assert!(
            engine
                .view()
                .events()
                .any(|event| matches!(event, RunEvent::OptimizationEnded { best: None, .. }))
        );
    });
}

#[test]
fn engine_surfaces_final_checkpoint_failures_after_end_event() {
    block_on(async {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut engine = Engine::<TestProblem>::builder()
            .persistence(FailOnCheckpoint {
                calls: calls.clone(),
                fail_on: 4,
            })
            .build();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = ContinueThenDone { steps: 0 };

        let err = engine
            .run(&mut optimizer, &cases, &store)
            .await
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("run checkpoint failed after finish")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 4);
        assert!(
            engine
                .view()
                .events()
                .any(|event| matches!(event, RunEvent::OptimizationEnded { best: None, .. }))
        );
        assert!(engine.view().events().any(|event| matches!(
            event,
            RunEvent::Error {
                error,
                ..
            } if error.kind == ErrorKind::Optimizer
        )));
    });
}

#[test]
fn run_context_checkpoints_after_graph_mutation_boundaries() {
    let checkpoints = Arc::new(AtomicUsize::new(0));
    let cache_absent = Arc::new(AtomicUsize::new(0));
    let persistence = CountingPersistence {
        checkpoints: checkpoints.clone(),
        cache_present: None,
        cache_absent: Some(cache_absent.clone()),
    };
    let (mut graph, mut budget) = graph_and_budget();
    let mut ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
        .with_persistence(Some(&persistence));

    ctx.insert_seed(TextArtifact("seed".to_owned()), 0).unwrap();
    let batch = record_one(
        &mut ctx,
        Proposal::create(TextArtifact("child".to_owned())).build(),
    );
    ctx.apply_batch(batch).unwrap();

    assert_eq!(checkpoints.load(Ordering::SeqCst), 3);
    assert_eq!(cache_absent.load(Ordering::SeqCst), 3);
}

#[test]
fn run_context_checkpoint_with_optimizer_state_advances_latest() {
    let optimizer_state_presence = Arc::new(Mutex::new(Vec::new()));
    let persistence = OptimizerStatePresencePersistence {
        optimizer_state_presence: optimizer_state_presence.clone(),
    };
    let (mut graph, mut budget) = graph_and_budget();
    let ctx = RunContext::<TestProblem>::new(&mut graph, &mut budget)
        .with_persistence(Some(&persistence));

    ctx.checkpoint_with_optimizer_state(
        OptimizerStateWrite::json(
            Fingerprint::from_bytes([7; 32]),
            Fingerprint::from_bytes([8; 32]),
            &StatefulOptimizerState {
                selected: None,
                cursor: 7,
            },
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        *optimizer_state_presence.lock().unwrap(),
        vec![(true, true)]
    );
}

fn checkpoint_referencing_graph(graph: BlobRef) -> RunCheckpoint {
    RunCheckpoint::new(
        RunId::new(),
        now(),
        GraphSnapshotRef {
            schema: Fingerprint::from_bytes([11; 32]),
            format: StateFormat::Json,
            bytes: graph,
        },
        leaven_engine::BudgetLedger::new(Budget::unlimited()).snapshot(),
    )
}

fn latest_checkpoint_err<S>(persistence: &StoreRunPersistence<S>) -> RunPersistenceError
where
    S: BlobStore + CheckpointStore,
{
    match persistence.latest_checkpoint::<TestProblem>() {
        Ok(_) => panic!("checkpoint load unexpectedly succeeded"),
        Err(error) => error,
    }
}

#[test]
fn checkpointable_optimizer_round_trips_explicit_private_state_against_graph() {
    let mut engine = Engine::<TestProblem>::builder().build();
    let seed = engine
        .insert_seed(TextArtifact("seed".to_owned()), 0)
        .unwrap();
    let optimizer = StatefulOptimizer {
        selected: Some(seed),
        cursor: 7,
    };

    assert!(matches!(
        optimizer.private_state_policy(),
        PrivateStatePolicy::ExplicitSnapshot { .. }
    ));
    let state = optimizer
        .checkpoint_state(CheckpointContext::new(engine.view()))
        .unwrap();

    let mut restored = StatefulOptimizer {
        selected: None,
        cursor: 0,
    };
    restored
        .restore_state(state, RestoreContext::new(engine.view()))
        .unwrap();

    assert_eq!(restored.selected, Some(seed));
    assert_eq!(restored.cursor, 7);
}

#[test]
fn checkpointable_optimizer_restore_rejects_missing_graph_truth() {
    let engine = Engine::<TestProblem>::builder().build();
    let mut optimizer = StatefulOptimizer {
        selected: None,
        cursor: 0,
    };
    let err = optimizer
        .restore_state(
            StatefulOptimizerState {
                selected: Some(CandidateId::new()),
                cursor: 1,
            },
            RestoreContext::new(engine.view()),
        )
        .unwrap_err();

    assert!(matches!(err, CheckpointError::MissingGraphTruth { .. }));
}

#[test]
fn checkpointable_optimizer_write_omits_graph_derived_state() {
    let engine = Engine::<TestProblem>::builder().build();
    let optimizer = DerivedStateOptimizer;

    let state =
        <DerivedStateOptimizer as CheckpointableOptimizer<TestProblem>>::checkpoint_state_write(
            &optimizer,
            CheckpointContext::new(engine.view()),
        )
        .unwrap();

    assert!(state.is_none());
}

#[test]
fn checkpointable_optimizer_write_rejects_non_json_state_format() {
    let engine = Engine::<TestProblem>::builder().build();
    let optimizer = NonJsonStateOptimizer;

    let err =
        <NonJsonStateOptimizer as CheckpointableOptimizer<TestProblem>>::checkpoint_state_write(
            &optimizer,
            CheckpointContext::new(engine.view()),
        )
        .unwrap_err();

    assert!(err.to_string().contains("private state checkpoint failed"));
    assert!(format!("{err:?}").contains("Postcard"));
}

#[test]
fn checkpointable_optimizer_write_surfaces_checkpoint_state_failures() {
    let engine = Engine::<TestProblem>::builder().build();
    let optimizer = FailingCheckpointStateOptimizer;

    let err =
        <FailingCheckpointStateOptimizer as CheckpointableOptimizer<TestProblem>>::checkpoint_state_write(
            &optimizer,
            CheckpointContext::new(engine.view()),
        )
        .unwrap_err();

    assert!(err.to_string().contains("private state checkpoint failed"));
    assert!(format!("{err:?}").contains("state unavailable"));
}

#[test]
fn checkpointable_optimizer_write_surfaces_serialization_failures() {
    let engine = Engine::<TestProblem>::builder().build();
    let optimizer = UnserializableStateOptimizer;

    let err =
        <UnserializableStateOptimizer as CheckpointableOptimizer<TestProblem>>::checkpoint_state_write(
            &optimizer,
            CheckpointContext::new(engine.view()),
        )
        .unwrap_err();

    assert!(
        err.to_string()
            .contains("private state serialization failed")
    );
}

#[test]
fn checkpointable_optimizer_restore_default_refuses_unhandled_private_state() {
    let engine = Engine::<TestProblem>::builder().build();
    let mut optimizer = PlainStatelessOptimizer;
    let mut checkpoint = checkpoint_referencing_graph(BlobRef {
        store: "recording".to_owned(),
        key: "graph".to_owned(),
    });
    checkpoint.optimizer_state = Some(OptimizerStateSnapshot {
        optimizer: Fingerprint::from_bytes([90; 32]),
        schema: Fingerprint::from_bytes([91; 32]),
        format: StateFormat::Json,
        bytes: BlobRef {
            store: "recording".to_owned(),
            key: "optimizer-state".to_owned(),
        },
    });

    let err = optimizer
        .restore_checkpoint_state(
            &checkpoint,
            &JsonStateReader::missing(),
            RestoreContext::new(engine.view()),
        )
        .unwrap_err();

    assert!(err.to_string().contains("private state"));
}

#[test]
fn checkpointable_optimizer_restore_accepts_absent_private_state_for_stateless_optimizers() {
    let engine = Engine::<TestProblem>::builder().build();
    let checkpoint_without_state = checkpoint_referencing_graph(BlobRef {
        store: "recording".to_owned(),
        key: "graph".to_owned(),
    });

    let mut plain = PlainStatelessOptimizer;
    plain
        .restore_checkpoint_state(
            &checkpoint_without_state,
            &JsonStateReader::missing(),
            RestoreContext::new(engine.view()),
        )
        .unwrap();

    let mut graph_derived = DerivedStateOptimizer;
    restore_checkpointable_optimizer_state(
        &mut graph_derived,
        &checkpoint_without_state,
        &JsonStateReader::missing(),
        RestoreContext::new(engine.view()),
    )
    .unwrap();
}

#[test]
fn checkpointable_optimizer_restore_helper_validates_policy_and_reader_state() {
    let engine = Engine::<TestProblem>::builder().build();
    let checkpoint_without_state = checkpoint_referencing_graph(BlobRef {
        store: "recording".to_owned(),
        key: "graph".to_owned(),
    });

    let mut non_json = NonJsonStateOptimizer;
    let err = restore_checkpointable_optimizer_state(
        &mut non_json,
        &checkpoint_without_state,
        &JsonStateReader::missing(),
        RestoreContext::new(engine.view()),
    )
    .unwrap_err();
    assert!(format!("{err:?}").contains("Postcard"));

    let mut explicit = StatefulOptimizer {
        selected: None,
        cursor: 0,
    };
    let err = restore_checkpointable_optimizer_state(
        &mut explicit,
        &checkpoint_without_state,
        &JsonStateReader::missing(),
        RestoreContext::new(engine.view()),
    )
    .unwrap_err();
    assert!(format!("{err:?}").contains("does not contain optimizer private state"));

    let mut explicit = StatefulOptimizer {
        selected: None,
        cursor: 0,
    };
    let err = restore_checkpointable_optimizer_state(
        &mut explicit,
        &checkpoint_without_state,
        &JsonStateReader::failing(),
        RestoreContext::new(engine.view()),
    )
    .unwrap_err();
    assert!(format!("{err:?}").contains("reader refused"));
}

#[test]
fn checkpointable_optimizer_restore_helper_rejects_graph_derived_private_state() {
    let engine = Engine::<TestProblem>::builder().build();
    let mut checkpoint = checkpoint_referencing_graph(BlobRef {
        store: "recording".to_owned(),
        key: "graph".to_owned(),
    });
    checkpoint.optimizer_state = Some(OptimizerStateSnapshot {
        optimizer: Fingerprint::from_bytes([90; 32]),
        schema: Fingerprint::from_bytes([91; 32]),
        format: StateFormat::Json,
        bytes: BlobRef {
            store: "recording".to_owned(),
            key: "optimizer-state".to_owned(),
        },
    });
    let mut optimizer = DerivedStateOptimizer;

    let err = restore_checkpointable_optimizer_state(
        &mut optimizer,
        &checkpoint,
        &JsonStateReader::missing(),
        RestoreContext::new(engine.view()),
    )
    .unwrap_err();

    assert!(format!("{err:?}").contains("graph-derived optimizer"));
}

#[test]
fn checkpointable_optimizer_restore_helper_surfaces_restore_state_errors() {
    let engine = Engine::<TestProblem>::builder().build();
    let mut checkpoint = checkpoint_referencing_graph(BlobRef {
        store: "recording".to_owned(),
        key: "graph".to_owned(),
    });
    checkpoint.optimizer_state = Some(OptimizerStateSnapshot {
        optimizer: STATEFUL_OPTIMIZER_FINGERPRINT,
        schema: STATEFUL_OPTIMIZER_STATE_SCHEMA,
        format: StateFormat::Json,
        bytes: BlobRef {
            store: "recording".to_owned(),
            key: "optimizer-state".to_owned(),
        },
    });
    let mut optimizer = StatefulOptimizer {
        selected: None,
        cursor: 0,
    };

    let err = restore_checkpointable_optimizer_state(
        &mut optimizer,
        &checkpoint,
        &JsonStateReader::state(StatefulOptimizerState {
            selected: Some(CandidateId::new()),
            cursor: 99,
        }),
        RestoreContext::new(engine.view()),
    )
    .unwrap_err();

    assert!(format!("{err:?}").contains("selected candidate"));
}

#[test]
fn checkpointable_optimizer_restore_helper_restores_explicit_json_state() {
    let mut engine = Engine::<TestProblem>::builder().build();
    let seed = engine
        .insert_seed(TextArtifact("seed".to_owned()), 0)
        .unwrap();
    let mut checkpoint = checkpoint_referencing_graph(BlobRef {
        store: "recording".to_owned(),
        key: "graph".to_owned(),
    });
    checkpoint.optimizer_state = Some(OptimizerStateSnapshot {
        optimizer: STATEFUL_OPTIMIZER_FINGERPRINT,
        schema: STATEFUL_OPTIMIZER_STATE_SCHEMA,
        format: StateFormat::Json,
        bytes: BlobRef {
            store: "recording".to_owned(),
            key: "optimizer-state".to_owned(),
        },
    });
    let mut optimizer = StatefulOptimizer {
        selected: None,
        cursor: 0,
    };

    restore_checkpointable_optimizer_state(
        &mut optimizer,
        &checkpoint,
        &JsonStateReader::state(StatefulOptimizerState {
            selected: Some(seed),
            cursor: 99,
        }),
        RestoreContext::new(engine.view()),
    )
    .unwrap();

    assert_eq!(optimizer.selected, Some(seed));
    assert_eq!(optimizer.cursor, 99);
}

#[test]
fn engine_surfaces_iteration_checkpoint_failures_as_run_errors() {
    block_on(async {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut engine = Engine::<TestProblem>::builder()
            .persistence(FailOnCheckpoint {
                calls: calls.clone(),
                fail_on: 2,
            })
            .build();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = ContinueThenDone { steps: 0 };

        let err = engine
            .run(&mut optimizer, &cases, &store)
            .await
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("run checkpoint failed after iteration")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    });
}

#[test]
fn engine_surfaces_final_optimizer_state_failures_as_run_errors() {
    block_on(async {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut engine = Engine::<TestProblem>::builder().build();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = FailingCheckpointWriteOptimizer {
            calls: calls.clone(),
            fail_on: 3,
        };

        let err = engine
            .run(&mut optimizer, &cases, &store)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("checkpoint write failed"));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    });
}

#[test]
fn engine_stops_run_when_optimizer_never_finishes() {
    block_on(async {
        let mut engine = Engine::<TestProblem>::builder().build();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = NeverDone;

        let err = engine
            .run(&mut optimizer, &cases, &store)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("exceeded"));
        assert!(engine.view().events().any(|event| matches!(
            event,
            RunEvent::Error {
                error,
                ..
            } if error.kind == ErrorKind::Optimizer
        )));
    });
}

#[test]
fn engine_records_initialize_errors_and_ends_run() {
    block_on(async {
        let mut engine = Engine::<TestProblem>::builder().build();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = FailingInitialize;

        let err = engine
            .run(&mut optimizer, &cases, &store)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("initialize failed"));
        assert!(engine.view().events().any(|event| matches!(
            event,
            RunEvent::Error {
                error,
                ..
            } if error.kind == ErrorKind::Optimizer
        )));
        let recorded = engine
            .view()
            .events()
            .find_map(|event| match event {
                RunEvent::Error { error, .. } if error.kind == ErrorKind::Optimizer => Some(error),
                _ => None,
            })
            .unwrap();
        assert!(recorded.debug.as_deref().unwrap().contains("WithSource"));
        assert_eq!(recorded.source_chain, vec!["optimizer backend offline"]);
        assert!(
            engine
                .view()
                .events()
                .any(|event| matches!(event, RunEvent::OptimizationEnded { best: None, .. }))
        );
    });
}

#[test]
fn engine_records_step_errors_and_ends_run() {
    block_on(async {
        let mut engine = Engine::<TestProblem>::builder().build();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = FailingStep;

        let err = engine
            .run(&mut optimizer, &cases, &store)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("step failed"));
        assert!(
            engine
                .view()
                .events()
                .any(|event| matches!(event, RunEvent::IterationEnded { .. }))
        );
        assert!(
            engine
                .view()
                .events()
                .any(|event| matches!(event, RunEvent::OptimizationEnded { best: None, .. }))
        );
    });
}

struct ContinueThenDone {
    steps: usize,
}

struct ChargeMetricThenContinue {
    best: Option<CandidateId>,
    steps: usize,
}

struct StopNotifiedAfterCharge {
    best: Option<CandidateId>,
    steps: usize,
    stopped: Arc<Mutex<Vec<StopReason>>>,
}

struct StopWithReason {
    best: Option<CandidateId>,
    reason: StopReason,
}

struct StopImmediately;

struct CountingCallback {
    seen: Arc<AtomicUsize>,
}

struct CountingPersistence {
    checkpoints: Arc<AtomicUsize>,
    cache_present: Option<Arc<AtomicUsize>>,
    cache_absent: Option<Arc<AtomicUsize>>,
}

struct OptimizerStatePresencePersistence {
    optimizer_state_presence: Arc<Mutex<Vec<(bool, bool)>>>,
}

#[derive(Clone)]
struct RecordingStore {
    name: String,
    inner: Arc<RecordingStoreInner>,
}

#[derive(Default)]
struct RecordingStoreInner {
    blobs: Mutex<Vec<(BlobRef, Bytes)>>,
    checkpoints: Mutex<BTreeMap<leaven_kernel::CheckpointId, CheckpointBytes>>,
    latest_checkpoint_id: Mutex<Option<leaven_kernel::CheckpointId>>,
}

impl RecordingStore {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            inner: Arc::default(),
        }
    }

    fn latest_checkpoint(&self) -> Option<CheckpointBytes> {
        let id = (*self.inner.latest_checkpoint_id.lock().unwrap())?;
        self.inner.checkpoints.lock().unwrap().get(&id).cloned()
    }

    fn checkpoints(&self) -> Vec<(leaven_kernel::CheckpointId, CheckpointBytes)> {
        self.inner
            .checkpoints
            .lock()
            .unwrap()
            .iter()
            .map(|(id, bytes)| (*id, bytes.clone()))
            .collect()
    }
}

impl BlobStore for RecordingStore {
    fn put(&self, write: BlobWrite) -> Result<BlobRef, StoreError> {
        let mut blobs = self.inner.blobs.lock().unwrap();
        let reference = BlobRef {
            store: self.name.clone(),
            key: blobs.len().to_string(),
        };
        blobs.push((reference.clone(), write.bytes));
        Ok(reference)
    }

    fn get(&self, reference: &BlobRef) -> Result<Bytes, StoreError> {
        if reference.store != self.name {
            return Err(StoreError::BlobNotFound(reference.clone()));
        }
        self.inner
            .blobs
            .lock()
            .unwrap()
            .iter()
            .find(|(stored, _)| stored.key == reference.key)
            .map(|(_, bytes)| bytes.clone())
            .ok_or_else(|| StoreError::BlobNotFound(reference.clone()))
    }
}

impl CheckpointStore for RecordingStore {
    fn put(&self, checkpoint: CheckpointBytes) -> Result<leaven_kernel::CheckpointId, StoreError> {
        let id = leaven_kernel::CheckpointId::new();
        self.inner
            .checkpoints
            .lock()
            .unwrap()
            .insert(id, checkpoint);
        Ok(id)
    }

    fn get(&self, id: leaven_kernel::CheckpointId) -> Result<CheckpointBytes, StoreError> {
        self.inner
            .checkpoints
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| StoreError::OperationFailed {
                store: self.name.clone(),
                operation: "get_checkpoint",
                reason: format!("checkpoint `{id}` was not found"),
                retryable: Some(false),
            })
    }

    fn latest(&self) -> Result<Option<leaven_kernel::CheckpointId>, StoreError> {
        Ok(*self.inner.latest_checkpoint_id.lock().unwrap())
    }

    fn mark_latest(&self, id: leaven_kernel::CheckpointId) -> Result<(), StoreError> {
        if !self.inner.checkpoints.lock().unwrap().contains_key(&id) {
            return Err(StoreError::OperationFailed {
                store: self.name.clone(),
                operation: "mark_latest_checkpoint",
                reason: format!("checkpoint `{id}` was not found"),
                retryable: Some(false),
            });
        }
        *self.inner.latest_checkpoint_id.lock().unwrap() = Some(id);
        Ok(())
    }
}

impl RunPersistence<TestProblem> for CountingPersistence {
    fn checkpoint(
        &self,
        request: RunCheckpointRequest<'_, TestProblem>,
    ) -> Result<(), RunPersistenceError> {
        let _ = request.run_id();
        let _ = request.budget.snapshot();
        if request.cache.is_some() {
            if let Some(cache_present) = &self.cache_present {
                cache_present.fetch_add(1, Ordering::SeqCst);
            }
        } else if let Some(cache_absent) = &self.cache_absent {
            cache_absent.fetch_add(1, Ordering::SeqCst);
        }
        self.checkpoints.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl RunPersistence<TestProblem> for OptimizerStatePresencePersistence {
    fn checkpoint(
        &self,
        request: RunCheckpointRequest<'_, TestProblem>,
    ) -> Result<(), RunPersistenceError> {
        self.optimizer_state_presence
            .lock()
            .unwrap()
            .push((request.optimizer_state.is_some(), request.advance_latest));
        Ok(())
    }
}

struct FailingPersistence;

impl RunPersistence<TestProblem> for FailingPersistence {
    fn checkpoint(
        &self,
        _request: RunCheckpointRequest<'_, TestProblem>,
    ) -> Result<(), RunPersistenceError> {
        Err(RunPersistenceError::CheckpointFailed {
            reason: "disk full".to_owned(),
            retryable: Some(true),
        })
    }
}

#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
struct FaultyStore {
    inner: RecordingStore,
    blob_puts: Arc<AtomicUsize>,
    fail_blob_put_on: Option<usize>,
    fail_checkpoint_put: bool,
    fail_checkpoint_get: bool,
    fail_latest: bool,
    fail_mark_latest: bool,
}

impl FaultyStore {
    fn new(name: impl Into<String>) -> Self {
        Self {
            inner: RecordingStore::new(name),
            blob_puts: Arc::new(AtomicUsize::new(0)),
            fail_blob_put_on: None,
            fail_checkpoint_put: false,
            fail_checkpoint_get: false,
            fail_latest: false,
            fail_mark_latest: false,
        }
    }

    fn fail_blob_put_on(mut self, call: usize) -> Self {
        self.fail_blob_put_on = Some(call);
        self
    }

    fn fail_checkpoint_put(mut self) -> Self {
        self.fail_checkpoint_put = true;
        self
    }

    fn fail_checkpoint_get(mut self) -> Self {
        self.fail_checkpoint_get = true;
        self
    }

    fn fail_latest(mut self) -> Self {
        self.fail_latest = true;
        self
    }

    fn fail_mark_latest(mut self) -> Self {
        self.fail_mark_latest = true;
        self
    }

    fn store_error(&self, operation: &'static str) -> StoreError {
        StoreError::OperationFailed {
            store: self.inner.name.clone(),
            operation,
            reason: "injected store refusal".to_owned(),
            retryable: Some(false),
        }
    }
}

impl BlobStore for FaultyStore {
    fn put(&self, write: BlobWrite) -> Result<BlobRef, StoreError> {
        let call = self.blob_puts.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_blob_put_on == Some(call) {
            return Err(self.store_error("put_blob"));
        }
        BlobStore::put(&self.inner, write)
    }

    fn get(&self, reference: &BlobRef) -> Result<Bytes, StoreError> {
        BlobStore::get(&self.inner, reference)
    }
}

impl CheckpointStore for FaultyStore {
    fn put(&self, checkpoint: CheckpointBytes) -> Result<leaven_kernel::CheckpointId, StoreError> {
        if self.fail_checkpoint_put {
            return Err(self.store_error("put_checkpoint"));
        }
        CheckpointStore::put(&self.inner, checkpoint)
    }

    fn get(&self, id: leaven_kernel::CheckpointId) -> Result<CheckpointBytes, StoreError> {
        if self.fail_checkpoint_get {
            return Err(self.store_error("get_checkpoint"));
        }
        CheckpointStore::get(&self.inner, id)
    }

    fn latest(&self) -> Result<Option<leaven_kernel::CheckpointId>, StoreError> {
        if self.fail_latest {
            return Err(self.store_error("latest_checkpoint"));
        }
        CheckpointStore::latest(&self.inner)
    }

    fn mark_latest(&self, id: leaven_kernel::CheckpointId) -> Result<(), StoreError> {
        if self.fail_mark_latest {
            return Err(self.store_error("mark_latest_checkpoint"));
        }
        CheckpointStore::mark_latest(&self.inner, id)
    }
}

struct FailOnCheckpoint {
    calls: Arc<AtomicUsize>,
    fail_on: usize,
}

impl RunPersistence<TestProblem> for FailOnCheckpoint {
    fn checkpoint(
        &self,
        _request: RunCheckpointRequest<'_, TestProblem>,
    ) -> Result<(), RunPersistenceError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == self.fail_on {
            return Err(RunPersistenceError::CheckpointFailed {
                reason: format!("checkpoint {call} failed"),
                retryable: Some(true),
            });
        }
        Ok(())
    }
}

impl Callback<TestProblem> for CountingCallback {
    fn on_event(&mut self, _event: &RunEvent, graph: RunGraphView<'_, TestProblem>) {
        let _ = graph.event_count();
        self.seen.fetch_add(1, Ordering::SeqCst);
    }
}

struct NeverDone;

impl Optimizer<TestProblem> for NeverDone {
    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Continue)
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Option<leaven_kernel::CandidateId> {
        None
    }
}

struct TrustInspectingOptimizer {
    expected_hidden: PartitionId,
    observed_hidden: bool,
}

impl Optimizer<TestProblem> for TrustInspectingOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        self.observed_hidden = ctx
            .graph()
            .read_scope()
            .hidden_partitions
            .contains(&self.expected_hidden);
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Option<leaven_kernel::CandidateId> {
        None
    }
}

impl Optimizer<TestProblem> for ContinueThenDone {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        assert!(ctx.iteration().is_some());
        self.steps += 1;
        if self.steps == 1 {
            Ok(StepStatus::Continue)
        } else {
            Ok(StepStatus::Done)
        }
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Option<leaven_kernel::CandidateId> {
        None
    }
}

impl Optimizer<TestProblem> for ChargeMetricThenContinue {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        assert!(ctx.iteration().is_some());
        ctx.charge(StageId::custom("metric"), Cost::metric_calls(1))
            .map_err(|error| OptimizerError::with_source("metric charge failed", error))?;
        self.steps += 1;
        Ok(StepStatus::Continue)
    }

    fn best_candidate(&self, _graph: RunGraphView<'_, TestProblem>) -> Option<CandidateId> {
        self.best
    }
}

impl Optimizer<TestProblem> for StopNotifiedAfterCharge {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        assert!(ctx.iteration().is_some());
        ctx.charge(StageId::custom("metric"), Cost::metric_calls(1))
            .map_err(|error| OptimizerError::with_source("metric charge failed", error))?;
        self.steps += 1;
        Ok(StepStatus::Continue)
    }

    fn best_candidate(&self, _graph: RunGraphView<'_, TestProblem>) -> Option<CandidateId> {
        self.best
    }

    fn on_engine_stop(&mut self, reason: StopReason) -> Result<(), OptimizerError> {
        self.stopped.lock().unwrap().push(reason);
        Ok(())
    }
}

impl Optimizer<TestProblem> for StopWithReason {
    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Stopped(self.reason))
    }

    fn best_candidate(&self, _graph: RunGraphView<'_, TestProblem>) -> Option<CandidateId> {
        self.best
    }
}

impl Stopper<TestProblem> for StopImmediately {
    fn should_stop(&self, _graph: RunGraphView<'_, TestProblem>) -> bool {
        true
    }
}

struct StatefulOptimizer {
    selected: Option<CandidateId>,
    cursor: u64,
}

struct PlainStatelessOptimizer;

impl Optimizer<TestProblem> for PlainStatelessOptimizer {
    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Done)
    }

    fn best_candidate(&self, _graph: RunGraphView<'_, TestProblem>) -> Option<CandidateId> {
        None
    }
}

struct JsonStateReader {
    state: Result<Option<serde_json::Value>, &'static str>,
}

impl JsonStateReader {
    fn missing() -> Self {
        Self { state: Ok(None) }
    }

    fn state<T: serde::Serialize>(state: T) -> Self {
        Self {
            state: Ok(Some(serde_json::to_value(state).unwrap())),
        }
    }

    fn failing() -> Self {
        Self {
            state: Err("reader refused"),
        }
    }
}

impl leaven_engine::OptimizerStateReader for JsonStateReader {
    fn load_optimizer_state<T>(
        &self,
        _checkpoint: &RunCheckpoint,
        _optimizer: Fingerprint,
        _schema: Fingerprint,
    ) -> Result<Option<T>, RunPersistenceError>
    where
        T: serde::de::DeserializeOwned,
    {
        match &self.state {
            Ok(Some(state)) => serde_json::from_value(state.clone())
                .map(Some)
                .map_err(|source| RunPersistenceError::Serialization {
                    state: "optimizer state",
                    reason: source.to_string(),
                }),
            Ok(None) => Ok(None),
            Err(reason) => Err(RunPersistenceError::Unavailable {
                reason: (*reason).to_owned(),
            }),
        }
    }
}

const STATEFUL_OPTIMIZER_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([5; 32]);
const STATEFUL_OPTIMIZER_STATE_SCHEMA: Fingerprint = Fingerprint::from_bytes([6; 32]);

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct StatefulOptimizerState {
    selected: Option<CandidateId>,
    cursor: u64,
}

impl Optimizer<TestProblem> for StatefulOptimizer {
    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Done)
    }

    fn best_candidate(&self, _graph: RunGraphView<'_, TestProblem>) -> Option<CandidateId> {
        self.selected
    }

    fn checkpoint_state_write(
        &self,
        ctx: CheckpointContext<'_, TestProblem>,
    ) -> Result<Option<OptimizerStateWrite>, OptimizerError> {
        <Self as CheckpointableOptimizer<TestProblem>>::checkpoint_state_write(self, ctx)
    }
}

impl CheckpointableOptimizer<TestProblem> for StatefulOptimizer {
    type State = StatefulOptimizerState;

    fn optimizer_fingerprint(&self) -> Fingerprint {
        STATEFUL_OPTIMIZER_FINGERPRINT
    }

    fn private_state_policy(&self) -> PrivateStatePolicy {
        PrivateStatePolicy::ExplicitSnapshot {
            schema: STATEFUL_OPTIMIZER_STATE_SCHEMA,
            format: StateFormat::Json,
        }
    }

    fn checkpoint_state(
        &self,
        ctx: CheckpointContext<'_, TestProblem>,
    ) -> Result<Self::State, CheckpointError> {
        if let Some(selected) = self.selected
            && ctx.graph().candidate(selected).is_none()
        {
            return Err(CheckpointError::MissingGraphTruth {
                reason: format!("selected candidate `{selected}` is not in the graph"),
            });
        }
        Ok(StatefulOptimizerState {
            selected: self.selected,
            cursor: self.cursor,
        })
    }

    fn restore_state(
        &mut self,
        state: Self::State,
        ctx: RestoreContext<'_, TestProblem>,
    ) -> Result<(), CheckpointError> {
        if let Some(selected) = state.selected
            && ctx.graph().candidate(selected).is_none()
        {
            return Err(CheckpointError::MissingGraphTruth {
                reason: format!("selected candidate `{selected}` is not in the graph"),
            });
        }
        self.selected = state.selected;
        self.cursor = state.cursor;
        Ok(())
    }
}

struct DerivedStateOptimizer;

impl Optimizer<TestProblem> for DerivedStateOptimizer {
    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Done)
    }

    fn best_candidate(&self, _graph: RunGraphView<'_, TestProblem>) -> Option<CandidateId> {
        None
    }
}

impl CheckpointableOptimizer<TestProblem> for DerivedStateOptimizer {
    type State = ();

    fn optimizer_fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([9; 32])
    }

    fn private_state_policy(&self) -> PrivateStatePolicy {
        PrivateStatePolicy::DerivedFromGraph
    }

    fn checkpoint_state(
        &self,
        _ctx: CheckpointContext<'_, TestProblem>,
    ) -> Result<Self::State, CheckpointError> {
        Ok(())
    }

    fn restore_state(
        &mut self,
        _state: Self::State,
        _ctx: RestoreContext<'_, TestProblem>,
    ) -> Result<(), CheckpointError> {
        Ok(())
    }
}

struct NonJsonStateOptimizer;

impl Optimizer<TestProblem> for NonJsonStateOptimizer {
    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Done)
    }

    fn best_candidate(&self, _graph: RunGraphView<'_, TestProblem>) -> Option<CandidateId> {
        None
    }
}

impl CheckpointableOptimizer<TestProblem> for NonJsonStateOptimizer {
    type State = ();

    fn optimizer_fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([10; 32])
    }

    fn private_state_policy(&self) -> PrivateStatePolicy {
        PrivateStatePolicy::ExplicitSnapshot {
            schema: Fingerprint::from_bytes([11; 32]),
            format: StateFormat::Postcard,
        }
    }

    fn checkpoint_state(
        &self,
        _ctx: CheckpointContext<'_, TestProblem>,
    ) -> Result<Self::State, CheckpointError> {
        Ok(())
    }

    fn restore_state(
        &mut self,
        _state: Self::State,
        _ctx: RestoreContext<'_, TestProblem>,
    ) -> Result<(), CheckpointError> {
        Ok(())
    }
}

struct FailingCheckpointStateOptimizer;

impl Optimizer<TestProblem> for FailingCheckpointStateOptimizer {
    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Done)
    }

    fn best_candidate(&self, _graph: RunGraphView<'_, TestProblem>) -> Option<CandidateId> {
        None
    }
}

impl CheckpointableOptimizer<TestProblem> for FailingCheckpointStateOptimizer {
    type State = ();

    fn optimizer_fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([12; 32])
    }

    fn private_state_policy(&self) -> PrivateStatePolicy {
        PrivateStatePolicy::ExplicitSnapshot {
            schema: Fingerprint::from_bytes([13; 32]),
            format: StateFormat::Json,
        }
    }

    fn checkpoint_state(
        &self,
        _ctx: CheckpointContext<'_, TestProblem>,
    ) -> Result<Self::State, CheckpointError> {
        Err(CheckpointError::StateUnavailable {
            reason: "state unavailable".to_owned(),
        })
    }

    fn restore_state(
        &mut self,
        _state: Self::State,
        _ctx: RestoreContext<'_, TestProblem>,
    ) -> Result<(), CheckpointError> {
        Ok(())
    }
}

struct UnserializableStateOptimizer;

impl Optimizer<TestProblem> for UnserializableStateOptimizer {
    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Done)
    }

    fn best_candidate(&self, _graph: RunGraphView<'_, TestProblem>) -> Option<CandidateId> {
        None
    }
}

impl CheckpointableOptimizer<TestProblem> for UnserializableStateOptimizer {
    type State = BTreeMap<(u8, u8), u8>;

    fn optimizer_fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([14; 32])
    }

    fn private_state_policy(&self) -> PrivateStatePolicy {
        PrivateStatePolicy::ExplicitSnapshot {
            schema: Fingerprint::from_bytes([15; 32]),
            format: StateFormat::Json,
        }
    }

    fn checkpoint_state(
        &self,
        _ctx: CheckpointContext<'_, TestProblem>,
    ) -> Result<Self::State, CheckpointError> {
        Ok(BTreeMap::from([((1, 2), 3)]))
    }

    fn restore_state(
        &mut self,
        _state: Self::State,
        _ctx: RestoreContext<'_, TestProblem>,
    ) -> Result<(), CheckpointError> {
        Ok(())
    }
}

struct FailingCheckpointWriteOptimizer {
    calls: Arc<AtomicUsize>,
    fail_on: usize,
}

impl Optimizer<TestProblem> for FailingCheckpointWriteOptimizer {
    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Done)
    }

    fn best_candidate(&self, _graph: RunGraphView<'_, TestProblem>) -> Option<CandidateId> {
        None
    }

    fn checkpoint_state_write(
        &self,
        _ctx: CheckpointContext<'_, TestProblem>,
    ) -> Result<Option<OptimizerStateWrite>, OptimizerError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == self.fail_on {
            return Err(OptimizerError::Message(
                "checkpoint write failed".to_owned(),
            ));
        }
        Ok(None)
    }
}

struct FailingInitialize;

impl Optimizer<TestProblem> for FailingInitialize {
    async fn initialize(
        &mut self,
        _ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<(), OptimizerError> {
        Err(OptimizerError::with_source(
            "initialize failed",
            StaticTestError("optimizer backend offline"),
        ))
    }

    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Option<leaven_kernel::CandidateId> {
        None
    }
}

struct FailingStep;

impl Optimizer<TestProblem> for FailingStep {
    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, TestProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        Err(OptimizerError::with_source(
            "step failed",
            StaticTestError("optimizer backend offline"),
        ))
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, TestProblem>,
    ) -> Option<leaven_kernel::CandidateId> {
        None
    }
}

#[derive(Debug)]
struct StaticTestError(&'static str);

impl std::fmt::Display for StaticTestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for StaticTestError {}
