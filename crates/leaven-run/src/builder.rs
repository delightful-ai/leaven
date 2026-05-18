//! Optimize builder lowering into the engine.

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    marker::PhantomData,
    num::NonZeroUsize,
    path::PathBuf,
    sync::Arc,
};

use futures::{FutureExt, future::BoxFuture};
use leaven_core::{Artifact, OptimizationProblem, PartitionId};
use leaven_engine::{CachePolicy, Callback, Optimizer, OptimizerError, TrustPolicy};
use leaven_eval::{Case, Dataset, DatasetSplits, NoTarget, SplitPolicy, SplitRole};
use leaven_evidence::CaseAssessmentEvidence;
use leaven_kernel::{
    Budget, BudgetSnapshot, CandidateId, CaseId, CheckpointId, Fingerprint, RunId,
};
use serde::{Serialize, de::DeserializeOwned};

use self::final_eval::{final_evaluation_inputs, run_final_evaluations};
use crate::{
    IntoOptimizeStore, IntoRunResult, OptimizeError, RunCase, RunError, RunOutput, Score,
    ScoreContext, ScoreError,
    compatibility::{
        DatasetCompatibility, RunCompatibilityManifest, RuntimeFingerprint, RuntimeKind,
        ScoringEvaluatorIdentity, case_content_fingerprint, case_set_version,
        compare_stored_manifest, store_fresh_manifest,
    },
    evaluator::{ScoringEvaluator, default_parallelism},
    result::Optimized,
    run_report::{
        ReportInputs, build_summary, report_paths_for, run_storage, write_summary_report,
    },
    run_store::{
        PreparedStore, StoreConfig, StoreSource, StoreStart, has_persistence, latest_checkpoint,
        mark_latest_checkpoint, prepare_store,
    },
};

type Runner<A, I> =
    Arc<dyn Fn(A, RunCase<I>) -> BoxFuture<'static, Result<RunOutput, RunError>> + Send + Sync>;
type Scorer<A, I, T> = Arc<
    dyn Fn(ScoreContext<A, I, T>) -> BoxFuture<'static, Result<Score, ScoreError>> + Send + Sync,
>;

mod final_eval;

struct CasePlan<I, T> {
    dataset: Dataset<Case<I, T>>,
    splits: DatasetSplits,
    case_set: leaven_engine::CaseSet<Case<I, T>>,
}

/// Problem type used by the public run builder.
pub struct RunProblem<A, I, T = NoTarget> {
    _marker: PhantomData<(A, I, T)>,
}

impl<A, I, T> OptimizationProblem for RunProblem<A, I, T>
where
    A: Artifact,
    I: Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    type Artifact = A;
    type Case = Case<I, T>;
    type Evidence = CaseAssessmentEvidence;
    type ProposalAnnotations = ();
}

/// Starts optimizing a seed artifact through the high-level product API.
#[must_use]
pub fn optimize<A>(seed: A) -> OptimizeBuilder<A, (), NoTarget, ()>
where
    A: Artifact,
{
    OptimizeBuilder {
        seed,
        train: Vec::new(),
        validation: Vec::new(),
        test: Vec::new(),
        runner: Arc::new(|_artifact, _case| async { Ok(RunOutput::default()) }.boxed()),
        scorer: None,
        runner_fingerprint: None,
        scorer_fingerprint: None,
        lm_role_fingerprints: BTreeMap::new(),
        optimizer: (),
        budget: None,
        evaluation_cache_policy: None,
        evaluation_parallelism: default_parallelism(),
        callbacks: Vec::new(),
        store: StoreConfig::Source(StoreSource::DefaultDurable),
        run_id: RunId::new(),
    }
}

/// Public optimize/train/validation/test builder.
pub struct OptimizeBuilder<A, I, T, O>
where
    A: Artifact,
    I: Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    seed: A,
    train: Vec<Case<I, T>>,
    validation: Vec<Case<I, T>>,
    test: Vec<Case<I, T>>,
    runner: Runner<A, I>,
    scorer: Option<Scorer<A, I, T>>,
    runner_fingerprint: Option<RuntimeFingerprint>,
    scorer_fingerprint: Option<RuntimeFingerprint>,
    lm_role_fingerprints: BTreeMap<String, RuntimeFingerprint>,
    optimizer: O,
    budget: Option<Budget>,
    evaluation_cache_policy: Option<CachePolicy>,
    evaluation_parallelism: NonZeroUsize,
    callbacks: Vec<Box<dyn Callback<RunProblem<A, I, T>>>>,
    store: StoreConfig<RunProblem<A, I, T>>,
    run_id: RunId,
}

impl<A> OptimizeBuilder<A, (), NoTarget, ()>
where
    A: Artifact,
{
    /// Supplies training case envelopes and fixes the run case type.
    #[must_use]
    pub fn train<I, T>(self, train: Vec<Case<I, T>>) -> OptimizeBuilder<A, I, T, ()>
    where
        I: Clone + Send + Sync + 'static,
        T: Clone + Send + Sync + 'static,
    {
        OptimizeBuilder {
            seed: self.seed,
            train,
            validation: Vec::new(),
            test: Vec::new(),
            runner: Arc::new(|_artifact, _case| async { Ok(RunOutput::default()) }.boxed()),
            scorer: None,
            runner_fingerprint: self.runner_fingerprint,
            scorer_fingerprint: self.scorer_fingerprint,
            lm_role_fingerprints: self.lm_role_fingerprints,
            optimizer: (),
            budget: self.budget,
            evaluation_cache_policy: self.evaluation_cache_policy,
            evaluation_parallelism: self.evaluation_parallelism,
            callbacks: Vec::new(),
            store: StoreConfig::Source(self.store.into_source()),
            run_id: self.run_id,
        }
    }

    /// Supplies input-only toy training cases with dense generated IDs.
    #[must_use]
    pub fn train_inputs<I>(self, train: Vec<I>) -> OptimizeBuilder<A, I, NoTarget, ()>
    where
        I: Clone + Send + Sync + 'static,
    {
        self.train(cases_from_inputs(0, train))
    }
}

impl<A, I, T, O> OptimizeBuilder<A, I, T, O>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    /// Supplies validation/dev case envelopes.
    #[must_use]
    pub fn validation(mut self, validation: Vec<Case<I, T>>) -> Self {
        self.validation = validation;
        self
    }

    /// Supplies held-out final test case envelopes.
    #[must_use]
    pub fn test(mut self, test: Vec<Case<I, T>>) -> Self {
        self.test = test;
        self
    }

    /// Supplies the runner/executor.
    #[must_use]
    pub fn runner<F, Fut>(mut self, runner: F) -> Self
    where
        F: Fn(A, RunCase<I>) -> Fut + Send + Sync + 'static,
        Fut: Future + Send + 'static,
        Fut::Output: IntoRunResult,
    {
        self.runner = Arc::new(move |artifact, case| {
            let output = runner(artifact, case);
            async move { output.await.into_run_result() }.boxed()
        });
        self
    }

    /// Supplies the async scoring function.
    #[must_use]
    pub fn score<F, Fut>(mut self, scorer: F) -> Self
    where
        F: Fn(ScoreContext<A, I, T>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Score, ScoreError>> + Send + 'static,
    {
        self.scorer = Some(Arc::new(move |ctx| scorer(ctx).boxed()));
        self
    }

    /// Declares the durable behavior fingerprint for the runner closure or adapter.
    #[must_use]
    pub fn runner_fingerprint(mut self, fingerprint: Fingerprint) -> Self {
        self.runner_fingerprint = Some(RuntimeFingerprint::new(fingerprint));
        self
    }

    /// Declares the durable behavior fingerprint for the scorer closure or adapter.
    #[must_use]
    pub fn scorer_fingerprint(mut self, fingerprint: Fingerprint) -> Self {
        self.scorer_fingerprint = Some(RuntimeFingerprint::new(fingerprint));
        self
    }

    /// Declares a durable LM/runtime fingerprint for a role used by this run.
    #[must_use]
    pub fn lm_role_fingerprint(
        mut self,
        role: impl Into<String>,
        fingerprint: Fingerprint,
    ) -> Self {
        self.lm_role_fingerprints
            .insert(role.into(), RuntimeFingerprint::new(fingerprint));
        self
    }

    /// Supplies the optimizer.
    #[must_use]
    pub fn using<Next>(self, optimizer: Next) -> OptimizeBuilder<A, I, T, Next> {
        OptimizeBuilder {
            seed: self.seed,
            train: self.train,
            validation: self.validation,
            test: self.test,
            runner: self.runner,
            scorer: self.scorer,
            runner_fingerprint: self.runner_fingerprint,
            scorer_fingerprint: self.scorer_fingerprint,
            lm_role_fingerprints: self.lm_role_fingerprints,
            optimizer,
            budget: self.budget,
            evaluation_cache_policy: self.evaluation_cache_policy,
            evaluation_parallelism: self.evaluation_parallelism,
            callbacks: self.callbacks,
            store: self.store,
            run_id: self.run_id,
        }
    }

    /// Sets the budget.
    #[must_use]
    pub fn budget(mut self, budget: Budget) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Declares when the engine may reuse scored candidate/case evaluations.
    ///
    /// The default is automatic: ordinary durable runs use deterministic
    /// candidate/case evaluation caching, while explicit `Never` remains the
    /// throwaway/debug path.
    #[must_use]
    pub fn evaluation_cache_policy(mut self, policy: CachePolicy) -> Self {
        self.evaluation_cache_policy = Some(policy);
        self
    }

    /// Sets the maximum number of runner/scorer jobs evaluated at once.
    #[must_use]
    pub const fn evaluation_parallelism(mut self, parallelism: NonZeroUsize) -> Self {
        self.evaluation_parallelism = parallelism;
        self
    }

    /// Registers a callback for public run events.
    #[must_use]
    pub fn on_event<Cb>(mut self, callback: Cb) -> Self
    where
        Cb: Callback<RunProblem<A, I, T>> + 'static,
    {
        self.callbacks.push(Box::new(callback));
        self
    }

    /// Uses a durable local run directory as the store and resume handle.
    #[must_use]
    pub fn run_dir(mut self, run_dir: impl Into<PathBuf>) -> Self {
        self.store = StoreConfig::Source(StoreSource::RunDir(run_dir.into()));
        self
    }

    /// Sets the fresh run id used by default durable storage.
    ///
    /// Resumed runs keep the stored checkpoint run id from the selected run
    /// directory; this value only names a fresh run.
    #[must_use]
    pub const fn run_id(mut self, run_id: RunId) -> Self {
        self.run_id = run_id;
        self
    }

    /// Runs without checkpoint persistence. This is the explicit throwaway
    /// path for tests and local experiments.
    #[must_use]
    pub fn ephemeral(mut self) -> Self {
        self.store = StoreConfig::Source(StoreSource::Ephemeral);
        self
    }

    /// Supplies evidence and optional checkpoint persistence for the run.
    #[must_use]
    pub fn store<S>(mut self, store: S) -> Self
    where
        S: IntoOptimizeStore<RunProblem<A, I, T>>,
    {
        self.store = StoreConfig::Explicit(store.into_optimize_store());
        self
    }
}

impl<A, I, O> OptimizeBuilder<A, I, NoTarget, O>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
{
    /// Supplies input-only toy validation/dev cases with dense generated IDs.
    #[must_use]
    pub fn validation_inputs(mut self, validation: Vec<I>) -> Self {
        self.validation = cases_from_inputs(self.train.len(), validation);
        self
    }

    /// Supplies input-only toy held-out final test cases with dense generated IDs.
    #[must_use]
    pub fn test_inputs(mut self, test: Vec<I>) -> Self {
        self.test = cases_from_inputs(self.train.len() + self.validation.len(), test);
        self
    }
}

impl<A, I, T, O> OptimizeBuilder<A, I, T, O>
where
    A: Artifact + Serialize + DeserializeOwned,
    <A as Artifact>::Change: Serialize + DeserializeOwned,
    I: Clone + Serialize + Send + Sync + 'static,
    T: Clone + Serialize + Send + Sync + 'static,
    O: Optimizer<RunProblem<A, I, T>>,
{
    /// Runs the optimization.
    pub async fn run(mut self) -> Result<Optimized<A>, OptimizeError> {
        let scorer = self.scorer.take().ok_or(OptimizeError::MissingScore)?;
        let budget = self.budget.take().ok_or(OptimizeError::MissingBudget)?;
        let metric_call_limit = budget.metric_calls;
        let engine_budget = search_ledger_budget(budget);
        if self.train.is_empty() && (!self.validation.is_empty() || !self.test.is_empty()) {
            return Err(OptimizeError::HeldOutWithoutTrain);
        }
        let case_content = case_content_fingerprint(&self.train, &self.validation, &self.test)
            .map_err(|source| OptimizeError::CaseFingerprint { source })?;
        let case_plan = build_case_plan(&self.train, &self.validation, &self.test, case_content)?;
        let mut prepared_store = prepare_run_store(&mut self.store, self.run_id)?;
        let (runner_fingerprint, scorer_fingerprint) = durable_runtime_fingerprints(
            prepared_store.run_dir.as_deref(),
            self.runner_fingerprint,
            self.scorer_fingerprint,
        )?;
        let evaluation_cache_policy = self
            .evaluation_cache_policy
            .clone()
            .unwrap_or_else(|| default_evaluation_cache_policy(&prepared_store));
        let evaluator_identity = scoring_evaluator_identity(
            runner_fingerprint,
            scorer_fingerprint,
            case_content,
            case_plan.splits.fingerprint(),
            evaluation_cache_policy.clone(),
        );
        let evaluator_fingerprint = RuntimeFingerprint::new(evaluator_identity.fingerprint());
        let compatibility = RunCompatibilityManifest::new(
            DatasetCompatibility::new(case_content, &case_plan.splits),
            runner_fingerprint,
            scorer_fingerprint,
            evaluator_fingerprint,
            self.optimizer.optimizer_compatibility(),
            self.lm_role_fingerprints.clone(),
        );
        let compatibility_summary = prepared_store
            .run_dir
            .as_ref()
            .map(|_| compatibility.summary());
        let evaluator = ScoringEvaluator::new(
            Arc::new(case_set_cases(&self.train, &self.validation, &self.test)),
            self.runner.clone(),
            scorer,
            &evaluator_identity,
        )
        .with_parallelism(self.evaluation_parallelism)
        .with_cache_policy(evaluation_cache_policy);
        let callbacks = std::mem::take(&mut self.callbacks);
        let EngineStart {
            engine,
            resumed,
            checkpoint,
        } = start_engine(EngineStartInputs {
            budget: engine_budget,
            metric_call_limit,
            evaluator,
            prepared_store: &mut prepared_store,
            compatibility: &compatibility,
            callbacks,
        })?;
        if let Some(checkpoint) = checkpoint {
            let reader = prepared_store.local_persistence.as_ref().ok_or_else(|| {
                OptimizeError::Optimizer(OptimizerError::Message(
                    "stored run resume requires a readable local persistence store".to_owned(),
                ))
            })?;
            self.optimizer.restore_checkpoint_state(
                &checkpoint,
                reader,
                leaven_engine::RestoreContext::new(engine.view()),
            )?;
            return run_with_engine(
                self,
                engine,
                EngineRunInputs {
                    case_set: &case_plan.case_set,
                    dataset: &case_plan.dataset,
                    splits: &case_plan.splits,
                    prepared_store,
                    resumed: true,
                    compatibility_summary,
                },
            )
            .await;
        }
        run_with_engine(
            self,
            engine,
            EngineRunInputs {
                case_set: &case_plan.case_set,
                dataset: &case_plan.dataset,
                splits: &case_plan.splits,
                prepared_store,
                resumed,
                compatibility_summary,
            },
        )
        .await
    }
}

fn prepare_run_store<P>(
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

fn durable_runtime_fingerprints(
    run_dir: Option<&std::path::Path>,
    runner: Option<RuntimeFingerprint>,
    scorer: Option<RuntimeFingerprint>,
) -> Result<(RuntimeFingerprint, RuntimeFingerprint), OptimizeError> {
    Ok((
        durable_runtime_fingerprint(run_dir, runner, RuntimeKind::Runner)?,
        durable_runtime_fingerprint(run_dir, scorer, RuntimeKind::Scorer)?,
    ))
}

fn default_evaluation_cache_policy<P>(prepared_store: &PreparedStore<P>) -> CachePolicy
where
    P: OptimizationProblem,
{
    if prepared_store.evaluation_cache.is_some() {
        CachePolicy::Deterministic
    } else {
        CachePolicy::Never
    }
}

fn search_ledger_budget(mut budget: Budget) -> Budget {
    // `Budget::metric_calls` on the public optimize path is the GEPA-compatible
    // search stopper. The engine ledger still enforces non-metric hard caps,
    // while metric calls stop before the next optimizer step so started
    // evaluator batches can finish.
    budget.metric_calls = None;
    budget
}

fn scoring_evaluator_identity(
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

struct EngineStartInputs<'a, A, I, T>
where
    A: Artifact,
    I: Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    budget: Budget,
    metric_call_limit: Option<u64>,
    evaluator: ScoringEvaluator<A, I, T>,
    prepared_store: &'a mut PreparedStore<RunProblem<A, I, T>>,
    compatibility: &'a RunCompatibilityManifest,
    callbacks: Vec<Box<dyn Callback<RunProblem<A, I, T>>>>,
}

struct EngineStart<A, I, T>
where
    A: Artifact,
    I: Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    engine: leaven_engine::Engine<RunProblem<A, I, T>>,
    resumed: bool,
    checkpoint: Option<Box<leaven_engine::RunCheckpoint>>,
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

fn start_engine<A, I, T>(
    inputs: EngineStartInputs<'_, A, I, T>,
) -> Result<EngineStart<A, I, T>, OptimizeError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
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

struct EngineRunInputs<'a, A, I, T>
where
    A: Artifact,
    I: Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    case_set: &'a leaven_engine::CaseSet<Case<I, T>>,
    dataset: &'a Dataset<Case<I, T>>,
    splits: &'a DatasetSplits,
    prepared_store: PreparedStore<RunProblem<A, I, T>>,
    resumed: bool,
    compatibility_summary: Option<crate::result::RunCompatibilitySummary>,
}

struct SearchRun {
    seed: CandidateId,
    run: leaven_engine::RunResult,
    optimization_budget: BudgetSnapshot,
    stop_reason: leaven_engine::StopReason,
    checkpoint: Option<CheckpointId>,
    optimizer_report: Option<leaven_engine::OptimizerReportPayload>,
}

async fn run_with_engine<A, I, T, O>(
    mut builder: OptimizeBuilder<A, I, T, O>,
    mut engine: leaven_engine::Engine<RunProblem<A, I, T>>,
    inputs: EngineRunInputs<'_, A, I, T>,
) -> Result<Optimized<A>, OptimizeError>
where
    A: Artifact + Serialize + DeserializeOwned,
    <A as Artifact>::Change: Serialize + DeserializeOwned,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
    O: Optimizer<RunProblem<A, I, T>>,
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
    if let Some(evaluation_cache) = prepared_store.evaluation_cache.as_ref() {
        evaluation_cache
            .replace_from_snapshot(&engine.evaluation_cache_snapshot())
            .map_err(|source| OptimizeError::EvaluationCache {
                operation: "flush sqlite evaluation cache after final reports",
                source,
            })?;
    }
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
    )?;
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

async fn run_optimizer_search<A, I, T, O>(
    builder: &mut OptimizeBuilder<A, I, T, O>,
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

fn cases_from_inputs<I>(start: usize, inputs: Vec<I>) -> Vec<Case<I, NoTarget>> {
    inputs
        .into_iter()
        .enumerate()
        .map(|(offset, input)| Case::input(CaseId::from_index(start + offset), input))
        .collect()
}

fn build_case_plan<I, T>(
    train: &[Case<I, T>],
    validation: &[Case<I, T>],
    test: &[Case<I, T>],
    case_content: Fingerprint,
) -> Result<CasePlan<I, T>, OptimizeError>
where
    I: Clone,
    T: Clone,
{
    let all_cases = all_cases(train, validation, test);
    let dataset = Dataset::from_cases(all_cases.clone())?;
    let splits = dataset_splits(train, validation, test, case_content);
    let case_set = case_set(all_cases, train.len(), validation.len(), test.len());
    Ok(CasePlan {
        dataset,
        splits,
        case_set,
    })
}

fn all_cases<I: Clone, T: Clone>(
    train: &[Case<I, T>],
    validation: &[Case<I, T>],
    test: &[Case<I, T>],
) -> Vec<Case<I, T>> {
    train
        .iter()
        .chain(validation)
        .chain(test)
        .cloned()
        .collect()
}

fn case_set_cases<I: Clone, T: Clone>(
    train: &[Case<I, T>],
    validation: &[Case<I, T>],
    test: &[Case<I, T>],
) -> Vec<Case<I, T>> {
    all_cases(train, validation, test)
}

fn case_set<I: Clone, T: Clone>(
    cases: Vec<Case<I, T>>,
    train: usize,
    validation: usize,
    test: usize,
) -> leaven_engine::CaseSet<Case<I, T>> {
    let train_ids = cases
        .iter()
        .take(train)
        .map(|case| case.id)
        .collect::<Vec<_>>();
    let validation_ids = cases
        .iter()
        .skip(train)
        .take(validation)
        .map(|case| case.id)
        .collect::<Vec<_>>();
    let test_ids = cases
        .iter()
        .skip(train + validation)
        .take(test)
        .map(|case| case.id)
        .collect::<Vec<_>>();
    let entries = cases.into_iter().map(|case| (case.id, case));
    leaven_engine::CaseSet::from_entries(entries)
        .with_partition(PartitionId::from("TRAIN"), train_ids)
        .with_partition(PartitionId::from("VALIDATION"), validation_ids)
        .with_partition(PartitionId::from("TEST"), test_ids)
}

fn dataset_splits<I, T>(
    train: &[Case<I, T>],
    validation: &[Case<I, T>],
    test: &[Case<I, T>],
    case_content: Fingerprint,
) -> DatasetSplits {
    let known = train
        .iter()
        .chain(validation)
        .chain(test)
        .map(|case| case.id)
        .collect::<BTreeSet<_>>();
    let roles = BTreeMap::from([
        (PartitionId::from("TRAIN"), SplitRole::Train),
        (PartitionId::from("VALIDATION"), SplitRole::Validation),
        (PartitionId::from("TEST"), SplitRole::Test),
    ]);
    let cases = BTreeMap::from([
        (
            PartitionId::from("TRAIN"),
            train.iter().map(|case| case.id).collect(),
        ),
        (
            PartitionId::from("VALIDATION"),
            validation.iter().map(|case| case.id).collect(),
        ),
        (
            PartitionId::from("TEST"),
            test.iter().map(|case| case.id).collect(),
        ),
    ]);
    DatasetSplits::new(
        case_set_version(case_content),
        roles,
        cases,
        &known,
        SplitPolicy::DisjointRequired,
    )
    .expect("builder constructs disjoint split ids")
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
