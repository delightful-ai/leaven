use serde::{Serialize, de::DeserializeOwned};

use super::*;
use leaven_engine::TrustPolicy;

pub(super) fn prepare_run_store<P>(
    store: &mut StoreConfig<P>,
    run_id: RunId,
) -> Result<PreparedStore<P>, OptimizeError>
where
    P: OptimizationProblem,
    P::Artifact: Serialize + DeserializeOwned,
    <P::Artifact as Artifact>::Change: Serialize + DeserializeOwned,
    P::Evidence: Clone + Serialize + DeserializeOwned,
    P::ProposalAnnotations: Serialize + DeserializeOwned,
{
    let store_config = std::mem::replace(store, StoreConfig::Source(StoreSource::Ephemeral));
    prepare_store::<P>(store_config, run_id)
}

pub(super) fn durable_runtime_fingerprints(
    run_dir: Option<&std::path::Path>,
    runner: Option<RuntimeFingerprint>,
    scorer: Option<RuntimeFingerprint>,
) -> Result<(RuntimeFingerprint, RuntimeFingerprint), OptimizeError> {
    Ok((
        durable_runtime_fingerprint(run_dir, runner, RuntimeKind::Runner)?,
        durable_runtime_fingerprint(run_dir, scorer, RuntimeKind::Scorer)?,
    ))
}

pub(super) fn default_evaluation_cache_policy<P>(prepared_store: &PreparedStore<P>) -> CachePolicy
where
    P: OptimizationProblem,
{
    if prepared_store.evaluation_cache.is_some() {
        CachePolicy::Deterministic
    } else {
        CachePolicy::Never
    }
}

pub(super) fn search_ledger_budget(mut budget: Budget) -> Budget {
    // `Budget::metric_calls` on the public optimize path is the GEPA-compatible
    // search stopper. The engine ledger still enforces non-metric hard caps,
    // while metric calls stop before the next optimizer step so started
    // evaluator batches can finish.
    budget.metric_calls = None;
    budget
}

pub(super) fn scoring_evaluator_identity(
    runner: RuntimeFingerprint,
    scorer: RuntimeFingerprint,
    dataset: Fingerprint,
    splits: Fingerprint,
    cache_policy: CachePolicy,
) -> ScoringEvaluatorIdentity {
    ScoringEvaluatorIdentity {
        label: "leaven-run/score".to_owned(),
        runner,
        scorer,
        dataset,
        splits,
        cache_policy,
    }
}

pub(super) struct EngineStartInputs<'a, A, I, T, Out>
where
    A: Artifact,
    I: Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    pub(super) budget: Budget,
    pub(super) metric_call_limit: Option<u64>,
    pub(super) evaluator: ScoringEvaluator<A, I, T, Out>,
    pub(super) prepared_store: &'a mut PreparedStore<RunProblem<A, I, T>>,
    pub(super) compatibility: &'a RunCompatibilityManifest,
    pub(super) callbacks: Vec<Box<dyn Callback<RunProblem<A, I, T>>>>,
}

pub(super) struct EngineStart<A, I, T>
where
    A: Artifact,
    I: Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    pub(super) engine: leaven_engine::Engine<RunProblem<A, I, T>>,
    pub(super) resumed: bool,
    pub(super) checkpoint: Option<Box<leaven_engine::RunCheckpoint>>,
}

struct ConfiguredEngineStart<A, I, T>
where
    A: Artifact,
    I: Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    builder: leaven_engine::EngineBuilder<RunProblem<A, I, T>>,
    resumed: bool,
    checkpoint: Option<Box<leaven_engine::RunCheckpoint>>,
}

pub(super) fn start_engine<A, I, T, Out>(
    inputs: EngineStartInputs<'_, A, I, T, Out>,
) -> Result<EngineStart<A, I, T>, OptimizeError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
    Out: Clone + Send + Sync + 'static,
{
    let EngineStartInputs {
        budget,
        metric_call_limit,
        evaluator,
        prepared_store,
        compatibility,
        callbacks,
    } = inputs;
    let mut engine_builder = leaven_engine::Engine::<RunProblem<A, I, T>>::builder()
        .budget(budget)
        .trust_policy(
            TrustPolicy::default()
                .hide_from_proposers([PartitionId::from("VALIDATION"), PartitionId::from("TEST")]),
        )
        .evaluator(evaluator);
    if let Some(evaluation_cache) = prepared_store.evaluation_cache.as_ref() {
        let cache =
            evaluation_cache
                .load_cache()
                .map_err(|source| OptimizeError::EvaluationCache {
                    operation: "load sqlite evaluation cache",
                    source,
                })?;
        engine_builder = engine_builder.evaluation_cache(cache);
    }
    if let Some(limit) = metric_call_limit {
        engine_builder = engine_builder.metric_call_budget_stopper(limit);
    }
    let ConfiguredEngineStart {
        builder: mut engine_builder,
        resumed,
        checkpoint,
    } = configure_engine_start(engine_builder, prepared_store, compatibility)?;
    if let Some(persistence) = prepared_store.store.persistence() {
        engine_builder = engine_builder.persistence(persistence);
    }
    Ok(EngineStart {
        engine: build_engine(engine_builder, callbacks),
        resumed,
        checkpoint,
    })
}

fn configure_engine_start<A, I, T>(
    engine_builder: leaven_engine::EngineBuilder<RunProblem<A, I, T>>,
    prepared_store: &mut PreparedStore<RunProblem<A, I, T>>,
    compatibility: &RunCompatibilityManifest,
) -> Result<ConfiguredEngineStart<A, I, T>, OptimizeError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let start = std::mem::replace(
        &mut prepared_store.start,
        StoreStart::Fresh {
            run_id: RunId::new(),
        },
    );
    match start {
        StoreStart::Fresh { run_id } => {
            store_fresh_manifest(prepared_store.run_dir.as_deref(), compatibility).map_err(
                |source| OptimizeError::CompatibilityStore {
                    operation: "write compatibility manifest",
                    source,
                },
            )?;
            Ok(ConfiguredEngineStart {
                builder: engine_builder.run_id(run_id),
                resumed: false,
                checkpoint: None,
            })
        }
        StoreStart::Resume {
            checkpoint,
            restored,
        } => {
            if let Some(run_dir) = prepared_store.run_dir.as_deref() {
                compare_stored_manifest(run_dir, compatibility)?;
            }
            Ok(ConfiguredEngineStart {
                builder: engine_builder.restored_run(*restored),
                resumed: true,
                checkpoint: Some(checkpoint),
            })
        }
    }
}

pub(super) struct EngineRunInputs<'a, A, I, T>
where
    A: Artifact,
    I: Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    pub(super) case_set: &'a leaven_engine::CaseSet<Case<I, T>>,
    pub(super) dataset: &'a Dataset<Case<I, T>>,
    pub(super) splits: &'a DatasetSplits,
    pub(super) prepared_store: PreparedStore<RunProblem<A, I, T>>,
    pub(super) resumed: bool,
    pub(super) compatibility_summary: Option<crate::result::RunCompatibilitySummary>,
}

struct SearchRun {
    seed: CandidateId,
    run: leaven_engine::RunResult,
    optimization_budget: BudgetSnapshot,
    stop_reason: leaven_engine::StopReason,
    checkpoint: Option<CheckpointId>,
    optimizer_report: Option<leaven_engine::OptimizerReportPayload>,
}

pub(super) async fn run_with_engine<A, I, T, O, Out>(
    mut builder: OptimizeBuilder<A, I, T, O, Out>,
    mut engine: leaven_engine::Engine<RunProblem<A, I, T>>,
    inputs: EngineRunInputs<'_, A, I, T>,
) -> Result<Optimized<A>, OptimizeError>
where
    A: Artifact + Serialize + DeserializeOwned,
    <A as Artifact>::Change: Serialize + DeserializeOwned,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
    O: Optimizer<RunProblem<A, I, T>>,
    Out: Clone + Send + Sync + 'static,
{
    let EngineRunInputs {
        case_set,
        dataset,
        splits,
        prepared_store,
        resumed,
        compatibility_summary,
    } = inputs;
    let search = run_optimizer_search(
        &mut builder,
        &mut engine,
        case_set,
        &prepared_store,
        resumed,
    )
    .await?;
    let best = search.run.best;
    if let Some(evaluation_cache) = prepared_store.evaluation_cache.as_ref() {
        evaluation_cache
            .replace_from_snapshot(&engine.evaluation_cache_snapshot())
            .map_err(|source| OptimizeError::EvaluationCache {
                operation: "flush sqlite evaluation cache",
                source,
            })?;
    }
    let final_inputs = final_evaluation_inputs(search.seed, best, &builder);
    if final_inputs.has_any_split() {
        engine.set_budget_limit(Budget::unlimited());
    }

    let final_evaluations = match run_final_evaluations(
        &mut engine,
        case_set,
        prepared_store.store.evidence_store(),
        final_inputs,
    )
    .await
    {
        Ok(final_evaluations) => final_evaluations,
        Err(source) => {
            mark_latest_checkpoint(&prepared_store, search.checkpoint)?;
            return Err(source.into());
        }
    };
    mark_latest_checkpoint(&prepared_store, search.checkpoint)?;
    let latest_checkpoint = latest_checkpoint(&prepared_store)?;
    let storage = run_storage(
        search.run.run_id,
        &prepared_store,
        latest_checkpoint,
        compatibility_summary.is_some(),
    );
    let reports = report_paths_for(&storage);
    let seed_artifact = engine
        .view()
        .artifact(search.seed)
        .ok_or(OptimizeError::MissingRestoredSeed)?
        .clone();
    let (best, summary, events) = build_summary(
        &engine,
        ReportInputs {
            dataset,
            splits,
            best,
            final_evaluations: &final_evaluations,
            optimization_budget: search.optimization_budget,
            storage,
            reports,
            compatibility: compatibility_summary,
            stop_reason: search.stop_reason,
        },
    );
    write_summary_report(&summary)?;
    let budget = summary.budget.clone();
    Ok(Optimized {
        run_id: search.run.run_id,
        seed_artifact,
        stop: search.stop_reason.into(),
        budget,
        best,
        summary,
        events,
        optimizer_report: search.optimizer_report,
    })
}

async fn run_optimizer_search<A, I, T, O, Out>(
    builder: &mut OptimizeBuilder<A, I, T, O, Out>,
    engine: &mut leaven_engine::Engine<RunProblem<A, I, T>>,
    case_set: &leaven_engine::CaseSet<Case<I, T>>,
    prepared_store: &PreparedStore<RunProblem<A, I, T>>,
    resumed: bool,
) -> Result<SearchRun, OptimizeError>
where
    A: Artifact + Clone,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
    O: Optimizer<RunProblem<A, I, T>>,
{
    let seed = seed_for_run(engine, &builder.seed, resumed)?;
    let run = if resumed {
        engine
            .resume(
                &mut builder.optimizer,
                case_set,
                prepared_store.store.evidence_store(),
            )
            .await?
    } else {
        engine
            .run(
                &mut builder.optimizer,
                case_set,
                prepared_store.store.evidence_store(),
            )
            .await?
    };
    let optimization_budget = engine.budget().snapshot();
    let stop_reason = stop_reason_from_events(&engine.view())?;
    let checkpoint = if has_persistence(prepared_store) {
        engine.checkpoint_optimizer_state(&builder.optimizer)?;
        latest_checkpoint(prepared_store)?
    } else {
        None
    };
    let optimizer_report = builder.optimizer.optimizer_report();
    Ok(SearchRun {
        seed,
        run,
        optimization_budget,
        stop_reason,
        checkpoint,
        optimizer_report,
    })
}

fn seed_for_run<A, I, T>(
    engine: &mut leaven_engine::Engine<RunProblem<A, I, T>>,
    seed: &A,
    resumed: bool,
) -> Result<CandidateId, OptimizeError>
where
    A: Artifact + Clone,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    if resumed {
        return engine
            .view()
            .candidate_tree()
            .roots()
            .first()
            .copied()
            .ok_or(OptimizeError::MissingRestoredSeed);
    }
    engine
        .insert_seed(seed.clone(), 0)
        .map_err(|source| OptimizeError::SeedInsertion { source })
}

fn build_engine<A, I, T>(
    mut engine_builder: leaven_engine::EngineBuilder<RunProblem<A, I, T>>,
    callbacks: Vec<Box<dyn Callback<RunProblem<A, I, T>>>>,
) -> leaven_engine::Engine<RunProblem<A, I, T>>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    for callback in callbacks {
        engine_builder = engine_builder.callback(callback);
    }
    engine_builder.build()
}

fn durable_runtime_fingerprint(
    run_dir: Option<&std::path::Path>,
    fingerprint: Option<RuntimeFingerprint>,
    runtime: RuntimeKind,
) -> Result<RuntimeFingerprint, OptimizeError> {
    match (run_dir, fingerprint) {
        (Some(_), None) => Err(runtime.missing_error()),
        (Some(_) | None, Some(fingerprint)) => Ok(fingerprint),
        (None, None) => Ok(RuntimeFingerprint::new(ephemeral_runtime_fingerprint(
            runtime,
        ))),
    }
}

fn ephemeral_runtime_fingerprint(runtime: RuntimeKind) -> Fingerprint {
    let mut fingerprint = leaven_kernel::FingerprintBuilder::new();
    fingerprint.update(b"leaven-run.ephemeral-runtime.v1");
    fingerprint.update(runtime.as_str().as_bytes());
    fingerprint.finish()
}

fn stop_reason_from_events<A, I, T>(
    view: &leaven_engine::RunGraphView<'_, RunProblem<A, I, T>>,
) -> Result<leaven_engine::StopReason, leaven_engine::OptimizerError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let mut stop_reason = None;
    for event in view.events() {
        if let leaven_engine::RunEvent::OptimizationStopping { reason } = event {
            stop_reason = Some(reason);
        }
    }
    stop_reason.copied().ok_or_else(|| {
        leaven_engine::OptimizerError::Message(
            "optimizer finished without a stop reason".to_owned(),
        )
    })
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use leaven_core::{ArtifactIdentity, CacheIdentity};
    use leaven_kernel::ContentId;

    use super::*;

    #[test]
    fn stop_reason_from_events_reports_missing_engine_stop_event() {
        let engine =
            leaven_engine::Engine::<RunProblem<TestArtifact, (), NoTarget>>::builder().build();

        let error = stop_reason_from_events(&engine.view()).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("optimizer finished without a stop reason")
        );
    }

    #[derive(Clone)]
    struct TestArtifact;

    impl Artifact for TestArtifact {
        type Change = ();
        type ApplyError = Infallible;

        fn identity(&self) -> ArtifactIdentity {
            ArtifactIdentity::Content(ContentId::from_bytes([1; 32]))
        }

        fn cache_identity(&self) -> Option<CacheIdentity> {
            Some(CacheIdentity::Content(ContentId::from_bytes([1; 32])))
        }

        fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
            Ok(Self)
        }
    }
}
