mod support;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use bytes::Bytes;
use futures::executor::block_on;
use leaven_core::{CacheIdentity, CaseSetVersion, PartitionId, Proposal};
use leaven_engine::{
    CacheIndexSnapshot, CachePolicy, Callback, CaseSet, CheckpointContext, CheckpointError,
    CheckpointableOptimizer, Engine, EvaluationCache, EvaluationCacheKey, EvaluationCacheSnapshot,
    GraphSnapshotRef, Optimizer, OptimizerError, OptimizerStateSnapshot, OptimizerStateWrite,
    PrivateStatePolicy, RestoreContext, RunCheckpoint, RunCheckpointRequest, RunContext, RunEvent,
    RunGraph, RunGraphSnapshot, RunGraphView, RunPersistence, RunPersistenceError, StateFormat,
    StepStatus, StopReason, Stopper, StoreRunPersistence, TrustPolicy, optimize,
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
                reason: StopReason::BudgetExceeded,
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
            case_set_version: CaseSetVersion("v1".to_owned()),
            case_ids: vec![CaseId::new(0)],
            candidates: vec![CacheIdentity::Content(ContentId::from_bytes([4; 32]))],
        },
        vec![AssessmentId::new()],
    );

    persistence
        .checkpoint(
            RunCheckpointRequest::new(&graph, &budget, Some(&cache)).with_optimizer_state(
                OptimizerStateWrite::json(
                    Fingerprint::from_bytes([5; 32]),
                    Fingerprint::from_bytes([6; 32]),
                    &StatefulOptimizerState {
                        selected: Some(seed),
                        cursor: 42,
                    },
                )
                .unwrap(),
            ),
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
fn store_run_persistence_reports_absent_and_corrupt_checkpoints_explicitly() {
    let store = RecordingStore::new("recording");
    let persistence = StoreRunPersistence::new(store.clone());

    assert!(
        persistence
            .latest_checkpoint::<TestProblem>()
            .unwrap()
            .is_none()
    );

    CheckpointStore::put(
        &store,
        CheckpointBytes(Bytes::from_static(b"not a checkpoint")),
    )
    .unwrap();
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
    CheckpointStore::put(
        &missing_graph_store,
        CheckpointBytes(Bytes::from(serde_json::to_vec(&checkpoint).unwrap())),
    )
    .unwrap();
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
    CheckpointStore::put(
        &corrupt_graph_store,
        CheckpointBytes(Bytes::from(
            serde_json::to_vec(&checkpoint_referencing_graph(graph_ref)).unwrap(),
        )),
    )
    .unwrap();
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
        .checkpoint(RunCheckpointRequest::new(&graph, &budget, None))
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
    CheckpointStore::put(
        persistence.store(),
        CheckpointBytes(Bytes::from(serde_json::to_vec(&checkpoint).unwrap())),
    )
    .unwrap();

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
    CheckpointStore::put(
        persistence.store(),
        CheckpointBytes(Bytes::from(serde_json::to_vec(&checkpoint).unwrap())),
    )
    .unwrap();

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
        .checkpoint(RunCheckpointRequest::new(&graph, &budget, None))
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
        .checkpoint(RunCheckpointRequest::new(&graph, &budget, None))
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
        .checkpoint(RunCheckpointRequest::new(&graph, &budget, None))
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

struct StopImmediately;

struct CountingCallback {
    seen: Arc<AtomicUsize>,
}

struct CountingPersistence {
    checkpoints: Arc<AtomicUsize>,
    cache_present: Option<Arc<AtomicUsize>>,
    cache_absent: Option<Arc<AtomicUsize>>,
}

#[derive(Clone)]
struct RecordingStore {
    name: String,
    inner: Arc<RecordingStoreInner>,
}

#[derive(Default)]
struct RecordingStoreInner {
    blobs: Mutex<Vec<(BlobRef, Bytes)>>,
    latest_checkpoint: Mutex<Option<CheckpointBytes>>,
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
        self.inner.latest_checkpoint.lock().unwrap().clone()
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
        *self.inner.latest_checkpoint.lock().unwrap() = Some(checkpoint);
        *self.inner.latest_checkpoint_id.lock().unwrap() = Some(id);
        Ok(id)
    }

    fn get(&self, _id: leaven_kernel::CheckpointId) -> Result<CheckpointBytes, StoreError> {
        self.latest_checkpoint()
            .ok_or_else(|| StoreError::OperationFailed {
                store: self.name.clone(),
                operation: "get_checkpoint",
                reason: "no checkpoint has been recorded".to_owned(),
                retryable: Some(false),
            })
    }

    fn latest(&self) -> Result<Option<leaven_kernel::CheckpointId>, StoreError> {
        Ok(*self.inner.latest_checkpoint_id.lock().unwrap())
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

impl Stopper<TestProblem> for StopImmediately {
    fn should_stop(&self, _graph: RunGraphView<'_, TestProblem>) -> bool {
        true
    }
}

struct StatefulOptimizer {
    selected: Option<CandidateId>,
    cursor: u64,
}

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
}

impl CheckpointableOptimizer<TestProblem> for StatefulOptimizer {
    type State = StatefulOptimizerState;

    fn private_state_policy(&self) -> PrivateStatePolicy {
        PrivateStatePolicy::ExplicitSnapshot {
            schema: Fingerprint::from_bytes([5; 32]),
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
