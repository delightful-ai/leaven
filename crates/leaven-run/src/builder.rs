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
use leaven_core::{
    Artifact, AssessmentGranularity, EvaluationPurpose, EvaluationRequest, EvaluationSet,
    OptimizationProblem, PartitionId,
};
use leaven_engine::{Callback, Optimizer, OptimizerError, TrustPolicy};
use leaven_eval::{
    CandidateEvaluationSummary, Case, Dataset, DatasetSplits, EvaluationReport, NoTarget,
    ReportScore, SplitPolicy, SplitReport, SplitRole,
};
use leaven_evidence::{CaseAssessmentEvidence, CasewiseEvidence, OutputRecord};
use leaven_kernel::{
    Budget, BudgetSnapshot, CandidateId, CaseId, Cost, EvaluatorId, Fingerprint, RunId,
};
use leaven_store::{CheckpointStore, EvidenceStore};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    IntoOptimizeStore, OptimizeError, OptimizeStore, RunCase, RunOutput, Score, ScoreContext,
    ScoreError,
    compatibility::{
        DatasetCompatibility, RunCompatibilityManifest, RuntimeFingerprint, RuntimeKind,
        ScoringEvaluatorIdentity, case_content_fingerprint, case_set_version,
        compare_stored_manifest, store_fresh_manifest,
    },
    evaluator::{ScoringEvaluator, default_parallelism},
    result::{BestCandidate, Optimized, RunEventSummary, RunStorage, StandardRunSummary, average},
    store::LocalOptimizeStore,
};

type Runner<A, I> = Arc<dyn Fn(A, RunCase<I>) -> BoxFuture<'static, RunOutput> + Send + Sync>;
type Scorer<A, I, T> = Arc<
    dyn Fn(ScoreContext<A, I, T>) -> BoxFuture<'static, Result<Score, ScoreError>> + Send + Sync,
>;

struct FinalEvaluations {
    baseline_train: Option<CandidateEvaluationSummary>,
    train: Option<CandidateEvaluationSummary>,
    baseline_validation: Option<CandidateEvaluationSummary>,
    validation: Option<CandidateEvaluationSummary>,
    baseline_test: Option<CandidateEvaluationSummary>,
    test: Option<CandidateEvaluationSummary>,
    cost: Cost,
}

struct FinalEvaluationInputs {
    seed: CandidateId,
    best: Option<CandidateId>,
    has_train: bool,
    has_validation: bool,
    has_test: bool,
}

struct FinalPartitionEvaluation {
    partition: PartitionId,
    purpose: EvaluationPurpose,
}

struct FinalPartitionResults {
    baseline: CandidateEvaluationSummary,
    optimized: Option<CandidateEvaluationSummary>,
    cost: Cost,
}

struct ReportInputs<'a, I, T> {
    dataset: &'a Dataset<Case<I, T>>,
    splits: &'a DatasetSplits,
    best: Option<CandidateId>,
    final_evaluations: &'a FinalEvaluations,
    optimization_budget: BudgetSnapshot,
    storage: RunStorage,
}

type SummaryBuild<A> = (
    Option<BestCandidate<A>>,
    StandardRunSummary,
    Vec<RunEventSummary>,
);

enum StoreSource {
    DefaultDurable,
    RunDir(PathBuf),
    Ephemeral,
}

enum StoreConfig<P>
where
    P: OptimizationProblem,
{
    Source(StoreSource),
    Explicit(OptimizeStore<P>),
}

impl<P> StoreConfig<P>
where
    P: OptimizationProblem,
{
    fn into_source(self) -> StoreSource {
        match self {
            Self::Source(source) => source,
            Self::Explicit(_) => StoreSource::DefaultDurable,
        }
    }
}

struct PreparedStore<P>
where
    P: OptimizationProblem,
{
    store: OptimizeStore<P>,
    run_dir: Option<PathBuf>,
    local_persistence: Option<leaven_engine::StoreRunPersistence<leaven_store_file::FileStore>>,
    start: StoreStart<P>,
    resumable: bool,
}

enum StoreStart<P>
where
    P: OptimizationProblem,
{
    Fresh {
        run_id: RunId,
    },
    Resume {
        checkpoint: leaven_engine::RunCheckpoint,
        restored: leaven_engine::RestoredRunState<P>,
    },
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
    type Evidence = CasewiseEvidence<CaseAssessmentEvidence>;
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
        runner: Arc::new(|_artifact, _case| async { RunOutput::default() }.boxed()),
        scorer: None,
        runner_fingerprint: None,
        scorer_fingerprint: None,
        optimizer: (),
        budget: None,
        evaluation_parallelism: default_parallelism(),
        callbacks: Vec::new(),
        store: StoreConfig::Source(StoreSource::DefaultDurable),
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
    optimizer: O,
    budget: Option<Budget>,
    evaluation_parallelism: NonZeroUsize,
    callbacks: Vec<Box<dyn Callback<RunProblem<A, I, T>>>>,
    store: StoreConfig<RunProblem<A, I, T>>,
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
            runner: Arc::new(|_artifact, _case| async { RunOutput::default() }.boxed()),
            scorer: None,
            runner_fingerprint: self.runner_fingerprint,
            scorer_fingerprint: self.scorer_fingerprint,
            optimizer: (),
            budget: self.budget,
            evaluation_parallelism: self.evaluation_parallelism,
            callbacks: Vec::new(),
            store: StoreConfig::Source(self.store.into_source()),
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
    I: Clone + Serialize + Send + Sync + 'static,
    T: Clone + Serialize + Send + Sync + 'static,
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
        Fut: Future<Output = RunOutput> + Send + 'static,
    {
        self.runner = Arc::new(move |artifact, case| runner(artifact, case).boxed());
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
            optimizer,
            budget: self.budget,
            evaluation_parallelism: self.evaluation_parallelism,
            callbacks: self.callbacks,
            store: self.store,
        }
    }

    /// Sets the budget.
    #[must_use]
    pub fn budget(mut self, budget: Budget) -> Self {
        self.budget = Some(budget);
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
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
    O: Optimizer<RunProblem<A, I, T>>,
{
    /// Runs the optimization.
    pub async fn run(mut self) -> Result<Optimized<A>, OptimizeError> {
        let scorer = self.scorer.take().ok_or(OptimizeError::MissingScore)?;
        let budget = self.budget.take().ok_or(OptimizeError::MissingBudget)?;
        let metric_call_limit = budget.metric_calls;
        if self.train.is_empty() && (!self.validation.is_empty() || !self.test.is_empty()) {
            return Err(OptimizeError::HeldOutWithoutTrain);
        }
        let case_content =
            case_content_fingerprint(&self.train, &self.validation, &self.test)
                .map_err(|source| OptimizeError::CaseFingerprint { source })?;
        let all_cases = all_cases(&self.train, &self.validation, &self.test);
        let dataset = Dataset::from_cases(all_cases.clone())?;
        let splits = dataset_splits(&self.train, &self.validation, &self.test, case_content);
        let case_set = case_set(
            all_cases,
            self.train.len(),
            self.validation.len(),
            self.test.len(),
        );
        let store_config =
            std::mem::replace(&mut self.store, StoreConfig::Source(StoreSource::Ephemeral));
        let mut prepared_store = prepare_store::<RunProblem<A, I, T>>(store_config, RunId::new())?;
        let runner_fingerprint = durable_runtime_fingerprint(
            prepared_store.run_dir.as_deref(),
            self.runner_fingerprint,
            RuntimeKind::Runner,
        )?;
        let scorer_fingerprint = durable_runtime_fingerprint(
            prepared_store.run_dir.as_deref(),
            self.scorer_fingerprint,
            RuntimeKind::Scorer,
        )?;
        let evaluator_identity = ScoringEvaluatorIdentity {
            label: "leaven-run/score".to_owned(),
            runner: runner_fingerprint,
            scorer: scorer_fingerprint,
            dataset: case_content,
            splits: splits.fingerprint(),
        };
        let evaluator_fingerprint = RuntimeFingerprint::new(evaluator_identity.fingerprint());
        let compatibility = RunCompatibilityManifest::new(
            DatasetCompatibility::new(case_content, &splits),
            runner_fingerprint,
            scorer_fingerprint,
            evaluator_fingerprint,
        );
        let evaluator = ScoringEvaluator::new(
            Arc::new(case_set_cases(&self.train, &self.validation, &self.test)),
            self.runner.clone(),
            scorer,
            evaluator_identity,
        )
        .with_parallelism(self.evaluation_parallelism);
        let mut engine_builder =
            leaven_engine::Engine::<RunProblem<A, I, T>>::builder()
                .budget(budget)
                .trust_policy(TrustPolicy::default().hide_from_proposers([
                    PartitionId::from("VALIDATION"),
                    PartitionId::from("TEST"),
                ]))
                .evaluator(evaluator);
        if let Some(limit) = metric_call_limit {
            engine_builder = engine_builder.metric_call_budget_stopper(limit);
        }
        let start = std::mem::replace(
            &mut prepared_store.start,
            StoreStart::Fresh {
                run_id: RunId::new(),
            },
        );
        let (is_resume, checkpoint) = match start {
            StoreStart::Fresh { run_id } => {
                store_fresh_manifest(prepared_store.run_dir.as_deref(), &compatibility).map_err(
                    |source| OptimizeError::CompatibilityStore {
                        operation: "write compatibility manifest",
                        source,
                    },
                )?;
                engine_builder = engine_builder.run_id(run_id);
                (false, None)
            }
            StoreStart::Resume {
                checkpoint,
                restored,
            } => {
                if let Some(run_dir) = prepared_store.run_dir.as_deref() {
                    compare_stored_manifest(run_dir, &compatibility)?;
                }
                engine_builder = engine_builder.restored_run(restored);
                (true, Some(checkpoint))
            }
        };
        if let Some(persistence) = prepared_store.store.persistence() {
            engine_builder = engine_builder.persistence(persistence);
        }
        let callbacks = std::mem::take(&mut self.callbacks);
        let engine = build_engine(engine_builder, callbacks);
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
                &case_set,
                &dataset,
                &splits,
                prepared_store,
                true,
            )
            .await;
        }
        run_with_engine(
            self,
            engine,
            &case_set,
            &dataset,
            &splits,
            prepared_store,
            is_resume,
        )
        .await
    }
}

async fn run_with_engine<A, I, T, O>(
    mut builder: OptimizeBuilder<A, I, T, O>,
    mut engine: leaven_engine::Engine<RunProblem<A, I, T>>,
    case_set: &leaven_engine::CaseSet<Case<I, T>>,
    dataset: &Dataset<Case<I, T>>,
    splits: &DatasetSplits,
    prepared_store: PreparedStore<RunProblem<A, I, T>>,
    resumed: bool,
) -> Result<Optimized<A>, OptimizeError>
where
    A: Artifact + Serialize + DeserializeOwned,
    <A as Artifact>::Change: Serialize + DeserializeOwned,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
    O: Optimizer<RunProblem<A, I, T>>,
{
    let seed = if resumed {
        engine
            .view()
            .candidate_tree()
            .roots()
            .first()
            .copied()
            .ok_or(OptimizeError::MissingRestoredSeed)?
    } else {
        engine
            .insert_seed(builder.seed.clone(), 0)
            .map_err(|source| OptimizeError::SeedInsertion { source })?
    };
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
    let best = run.best;
    let has_train = !builder.train.is_empty();
    let has_validation = !builder.validation.is_empty();
    let has_test = !builder.test.is_empty();
    if has_train || has_validation || has_test {
        engine.set_budget_limit(Budget::unlimited());
    }

    let final_evaluations = run_final_evaluations(
        &mut engine,
        case_set,
        prepared_store.store.evidence_store(),
        FinalEvaluationInputs {
            seed,
            best,
            has_train,
            has_validation,
            has_test,
        },
    )
    .await?;
    if has_persistence(&prepared_store) {
        engine.checkpoint_optimizer_state(&builder.optimizer)?;
    }
    let latest_checkpoint = latest_checkpoint(&prepared_store)?;
    let storage = run_storage(run.run_id, &prepared_store, latest_checkpoint);
    let seed_artifact = engine
        .view()
        .artifact(seed)
        .ok_or(OptimizeError::MissingRestoredSeed)?
        .clone();
    let (best, summary, events) = build_summary(
        &engine,
        prepared_store.store.evidence_store(),
        ReportInputs {
            dataset,
            splits,
            best,
            final_evaluations: &final_evaluations,
            optimization_budget,
            storage,
        },
    )?;
    let budget = summary.budget.clone();
    Ok(Optimized {
        run_id: run.run_id,
        seed_artifact,
        stop: stop_reason.into(),
        budget,
        best,
        summary,
        events,
    })
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

fn prepare_store<P>(
    config: StoreConfig<P>,
    run_id: RunId,
) -> Result<PreparedStore<P>, OptimizeError>
where
    P: OptimizationProblem,
    P::Artifact: Serialize + DeserializeOwned,
    <P::Artifact as Artifact>::Change: Serialize + DeserializeOwned,
    P::Evidence: Clone + Serialize + DeserializeOwned,
    P::ProposalAnnotations: Serialize + DeserializeOwned,
{
    match config {
        StoreConfig::Source(StoreSource::DefaultDurable) => {
            prepare_local_store(default_run_dir(run_id), run_id)
        }
        StoreConfig::Source(StoreSource::RunDir(run_dir)) => prepare_local_store(run_dir, run_id),
        StoreConfig::Source(StoreSource::Ephemeral) => Ok(PreparedStore {
            store: OptimizeStore::inline("leaven-run"),
            run_dir: None,
            local_persistence: None,
            start: StoreStart::Fresh { run_id },
            resumable: false,
        }),
        StoreConfig::Explicit(store) => Ok(PreparedStore {
            store,
            run_dir: None,
            local_persistence: None,
            start: StoreStart::Fresh { run_id },
            resumable: false,
        }),
    }
}

fn prepare_local_store<P>(
    run_dir: PathBuf,
    run_id: RunId,
) -> Result<PreparedStore<P>, OptimizeError>
where
    P: OptimizationProblem,
    P::Artifact: Serialize + DeserializeOwned,
    <P::Artifact as Artifact>::Change: Serialize + DeserializeOwned,
    P::Evidence: Clone + Serialize + DeserializeOwned,
    P::ProposalAnnotations: Serialize + DeserializeOwned,
{
    let local = LocalOptimizeStore::<P>::open(&run_dir).map_err(|source| OptimizeError::Store {
        operation: "open local run store",
        source,
    })?;
    let restored = local
        .persistence
        .latest_checkpoint::<P>()
        .map_err(|source| OptimizeError::Resume {
            operation: "read latest checkpoint",
            source,
        })?;
    let start = match restored {
        Some(restored) => StoreStart::Resume {
            checkpoint: restored.checkpoint.clone(),
            restored,
        },
        None => StoreStart::Fresh { run_id },
    };
    Ok(PreparedStore {
        store: local.store,
        run_dir: Some(local.run_dir),
        local_persistence: Some(local.persistence),
        start,
        resumable: true,
    })
}

fn default_run_dir(run_id: RunId) -> PathBuf {
    PathBuf::from(".leaven")
        .join("runs")
        .join(run_id.to_string())
}

fn has_persistence<P>(store: &PreparedStore<P>) -> bool
where
    P: OptimizationProblem,
{
    store.store.persistence().is_some()
}

fn latest_checkpoint<P>(
    store: &PreparedStore<P>,
) -> Result<Option<leaven_kernel::CheckpointId>, OptimizeError>
where
    P: OptimizationProblem,
{
    let Some(persistence) = &store.local_persistence else {
        return Ok(None);
    };
    persistence
        .store()
        .latest()
        .map_err(|source| OptimizeError::Store {
            operation: "read latest checkpoint pointer",
            source,
        })
}

fn durable_runtime_fingerprint(
    run_dir: Option<&std::path::Path>,
    fingerprint: Option<RuntimeFingerprint>,
    runtime: RuntimeKind,
) -> Result<RuntimeFingerprint, OptimizeError> {
    match (run_dir, fingerprint) {
        (Some(_), Some(fingerprint)) => Ok(fingerprint),
        (Some(_), None) => Err(runtime.missing_error()),
        (None, Some(fingerprint)) => Ok(fingerprint),
        (None, None) => Ok(RuntimeFingerprint::new(ephemeral_runtime_fingerprint(runtime))),
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
    let train_ids = (0..train).map(CaseId::from_index).collect::<Vec<_>>();
    let validation_ids = (train..train + validation)
        .map(CaseId::from_index)
        .collect::<Vec<_>>();
    let test_ids = (train + validation..train + validation + test)
        .map(CaseId::from_index)
        .collect::<Vec<_>>();
    leaven_engine::CaseSet::new(cases)
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

async fn run_final_evaluations<A, I, T>(
    engine: &mut leaven_engine::Engine<RunProblem<A, I, T>>,
    case_set: &leaven_engine::CaseSet<Case<I, T>>,
    store: &dyn EvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>>,
    inputs: FinalEvaluationInputs,
) -> Result<FinalEvaluations, leaven_engine::OptimizerError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let mut cost = Cost::zero();
    let train = if inputs.has_train {
        let results = final_eval_partition(
            engine,
            case_set,
            store,
            &inputs,
            FinalPartitionEvaluation {
                partition: PartitionId::from("TRAIN"),
                purpose: EvaluationPurpose::Custom("final-train-report".into()),
            },
        )
        .await?;
        cost = cost.combine(&results.cost);
        Some((results.baseline, results.optimized))
    } else {
        None
    };
    let validation = if inputs.has_validation {
        let results = final_eval_partition(
            engine,
            case_set,
            store,
            &inputs,
            FinalPartitionEvaluation {
                partition: PartitionId::from("VALIDATION"),
                purpose: EvaluationPurpose::Validation,
            },
        )
        .await?;
        cost = cost.combine(&results.cost);
        Some((results.baseline, results.optimized))
    } else {
        None
    };
    let test = if inputs.has_test {
        let results = final_eval_partition(
            engine,
            case_set,
            store,
            &inputs,
            FinalPartitionEvaluation {
                partition: PartitionId::from("TEST"),
                purpose: EvaluationPurpose::FinalTest,
            },
        )
        .await?;
        cost = cost.combine(&results.cost);
        Some((results.baseline, results.optimized))
    } else {
        None
    };
    Ok(FinalEvaluations {
        baseline_train: train.as_ref().map(|(baseline, _)| baseline.clone()),
        train: train.and_then(|(_, optimized)| optimized),
        baseline_validation: validation.as_ref().map(|(baseline, _)| baseline.clone()),
        validation: validation.and_then(|(_, optimized)| optimized),
        baseline_test: test.as_ref().map(|(baseline, _)| baseline.clone()),
        test: test.and_then(|(_, optimized)| optimized),
        cost,
    })
}

async fn final_eval_partition<A, I, T>(
    engine: &mut leaven_engine::Engine<RunProblem<A, I, T>>,
    case_set: &leaven_engine::CaseSet<Case<I, T>>,
    store: &dyn EvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>>,
    inputs: &FinalEvaluationInputs,
    evaluation: FinalPartitionEvaluation,
) -> Result<FinalPartitionResults, leaven_engine::OptimizerError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let (baseline, baseline_cost) = final_eval(
        engine,
        case_set,
        store,
        inputs.seed,
        evaluation.partition.clone(),
        evaluation.purpose.clone(),
    )
    .await?;
    let (optimized, optimized_cost) = if let Some(best) = inputs.best {
        let (optimized, optimized_cost) = final_eval(
            engine,
            case_set,
            store,
            best,
            evaluation.partition,
            evaluation.purpose,
        )
        .await?;
        (Some(optimized), optimized_cost)
    } else {
        (None, Cost::zero())
    };
    Ok(FinalPartitionResults {
        baseline,
        optimized,
        cost: baseline_cost.combine(&optimized_cost),
    })
}

fn build_summary<A, I, T>(
    engine: &leaven_engine::Engine<RunProblem<A, I, T>>,
    store: &dyn EvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>>,
    inputs: ReportInputs<'_, I, T>,
) -> Result<SummaryBuild<A>, leaven_engine::OptimizerError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let view = engine.view();
    let best = inputs.best.map(|id| BestCandidate {
        id,
        artifact: view.artifact(id).expect("best exists").clone(),
    });
    let budget = engine.budget().snapshot();
    let cost = budget.spent.clone();
    let summary = StandardRunSummary {
        storage: inputs.storage,
        optimization_budget: inputs.optimization_budget.clone(),
        budget,
        optimization_cost: inputs.optimization_budget.spent,
        final_report_cost: inputs.final_evaluations.cost.clone(),
        cost: cost.clone(),
        baseline_train_score: inputs
            .final_evaluations
            .baseline_train
            .as_ref()
            .and_then(|summary| summary.average_score),
        optimized_train_score: inputs
            .final_evaluations
            .train
            .as_ref()
            .and_then(|summary| summary.average_score),
        baseline_validation_score: inputs
            .final_evaluations
            .baseline_validation
            .as_ref()
            .and_then(|summary| summary.average_score),
        validation_score: inputs
            .final_evaluations
            .validation
            .as_ref()
            .and_then(|summary| summary.average_score),
        baseline_test_score: inputs
            .final_evaluations
            .baseline_test
            .as_ref()
            .and_then(|summary| summary.average_score),
        test_score: inputs
            .final_evaluations
            .test
            .as_ref()
            .and_then(|summary| summary.average_score),
        evaluation: EvaluationReport {
            dataset: inputs.dataset.fingerprint(),
            splits: inputs.splits.fingerprint(),
            cost,
            splits_reported: split_reports_for(&view, store, inputs.splits)?,
        },
    };
    let events = view.events().map(event_summary).collect();
    Ok((best, summary, events))
}

async fn final_eval<A, I, T>(
    engine: &mut leaven_engine::Engine<RunProblem<A, I, T>>,
    case_set: &leaven_engine::CaseSet<Case<I, T>>,
    store: &dyn EvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>>,
    candidate: leaven_kernel::CandidateId,
    partition: PartitionId,
    purpose: EvaluationPurpose,
) -> Result<(CandidateEvaluationSummary, Cost), leaven_engine::OptimizerError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let report = engine
        .evaluate(
            EvaluatorId::PRIMARY,
            EvaluationRequest::Independent {
                candidates: vec![candidate],
                set: EvaluationSet::Partition(partition),
                granularity: AssessmentGranularity::PerCase,
                purpose,
            },
            case_set,
            store,
        )
        .await
        .map_err(|source| {
            leaven_engine::OptimizerError::with_source("final evaluation failed", source)
        })?;
    let view = engine.view();
    Ok((
        assessment_summary(&view, store, report.assessment_ids[0])?,
        report.cost,
    ))
}

fn split_reports_for<A, I, T>(
    view: &leaven_engine::RunGraphView<'_, RunProblem<A, I, T>>,
    store: &dyn EvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>>,
    splits: &DatasetSplits,
) -> Result<Vec<SplitReport>, leaven_engine::OptimizerError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let mut reports = BTreeMap::<PartitionId, SplitReport>::new();
    for assessment in view.all_assessments() {
        let Some((partition, role)) = assessment_split(view, assessment.id()) else {
            continue;
        };
        if splits.role(&partition).is_none() {
            continue;
        }
        let summary = assessment_summary(view, store, assessment.id())?;
        reports
            .entry(partition.clone())
            .or_insert_with(|| SplitReport {
                role,
                partition,
                candidates: Vec::new(),
            })
            .candidates
            .push(summary);
    }
    Ok(reports.into_values().collect())
}

fn assessment_split<A, I, T>(
    view: &leaven_engine::RunGraphView<'_, RunProblem<A, I, T>>,
    assessment: leaven_kernel::AssessmentId,
) -> Option<(PartitionId, SplitRole)>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let request_id = view.assessment(assessment)?.request_id();
    let evaluation_request = view.evaluation_request(request_id)?;
    let request = evaluation_request.request();
    let partition = match request {
        EvaluationRequest::Independent {
            set: EvaluationSet::Partition(partition),
            ..
        } => partition.clone(),
        _ => return None,
    };
    let role = match partition.0.as_str() {
        "TRAIN" => SplitRole::Train,
        "VALIDATION" => SplitRole::Validation,
        "TEST" => SplitRole::Test,
        other => SplitRole::Custom(other.to_owned().into()),
    };
    Some((partition, role))
}

fn assessment_summary<A, I, T>(
    view: &leaven_engine::RunGraphView<'_, RunProblem<A, I, T>>,
    store: &dyn EvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>>,
    assessment: leaven_kernel::AssessmentId,
) -> Result<CandidateEvaluationSummary, leaven_engine::OptimizerError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let assessment_view = view.assessment(assessment).ok_or_else(|| {
        leaven_engine::OptimizerError::Message("assessment missing from graph".to_owned())
    })?;
    let candidate = assessment_view.independent_candidate().ok_or_else(|| {
        leaven_engine::OptimizerError::Message("report expected independent assessment".to_owned())
    })?;
    let evidence = store
        .get(assessment_view.evidence_ref())
        .map_err(|source| {
            leaven_engine::OptimizerError::with_source("report evidence lookup failed", source)
        })?;
    let cases = report_scores(&evidence);
    Ok(CandidateEvaluationSummary {
        candidate,
        request: assessment_view.request_id(),
        assessment,
        average_score: average(&cases),
        cases,
    })
}

fn report_scores(evidence: &CasewiseEvidence<CaseAssessmentEvidence>) -> Vec<ReportScore> {
    evidence
        .outcomes()
        .iter()
        .map(|outcome| ReportScore {
            case_id: outcome.case(),
            score: outcome.evidence().score().score(),
            feedback: outcome.evidence().feedback().to_owned(),
            output: output_record_text(outcome.evidence().output()),
        })
        .collect()
}

fn output_record_text(output: &OutputRecord) -> String {
    match output {
        OutputRecord::Inline { text, .. } => text.clone(),
        OutputRecord::BlobRef(reference) => format!("blob:{}:{}", reference.store, reference.key),
    }
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

fn run_storage<P>(
    run_id: leaven_kernel::RunId,
    store: &PreparedStore<P>,
    latest_checkpoint: Option<leaven_kernel::CheckpointId>,
) -> RunStorage
where
    P: OptimizationProblem,
{
    if store.store.persistence().is_some() {
        RunStorage::Stored {
            run_id,
            run_dir: store.run_dir.clone(),
            latest_checkpoint,
            resumable: store.resumable && latest_checkpoint.is_some(),
        }
    } else {
        RunStorage::Ephemeral { run_id }
    }
}

fn event_summary(event: &leaven_engine::RunEvent) -> RunEventSummary {
    match event {
        leaven_engine::RunEvent::OptimizationStarted { .. } => RunEventSummary::OptimizationStarted,
        leaven_engine::RunEvent::IterationStarted { .. } => RunEventSummary::IterationStarted,
        leaven_engine::RunEvent::BudgetCharged { .. } => RunEventSummary::BudgetCharged,
        leaven_engine::RunEvent::ProposalBatchProduced { .. } => {
            RunEventSummary::ProposalBatchProduced
        }
        leaven_engine::RunEvent::ProposalRecorded { .. } => RunEventSummary::ProposalRecorded,
        leaven_engine::RunEvent::StageAttemptRecorded { .. } => {
            RunEventSummary::StageAttemptRecorded
        }
        leaven_engine::RunEvent::ApplySucceeded { .. } => RunEventSummary::ApplySucceeded,
        leaven_engine::RunEvent::ApplyFailed { .. } => RunEventSummary::ApplyFailed,
        leaven_engine::RunEvent::EvaluationRequested { .. } => RunEventSummary::EvaluationRequested,
        leaven_engine::RunEvent::EvaluationCompleted { .. } => RunEventSummary::EvaluationCompleted,
        leaven_engine::RunEvent::PopulationUpdated { .. } => RunEventSummary::PopulationUpdated,
        leaven_engine::RunEvent::IterationEnded { .. } => RunEventSummary::IterationEnded,
        leaven_engine::RunEvent::OptimizationStopping { .. } => {
            RunEventSummary::OptimizationStopping
        }
        leaven_engine::RunEvent::OptimizationEnded { .. } => RunEventSummary::OptimizationEnded,
        leaven_engine::RunEvent::Error { .. } => RunEventSummary::Error,
    }
}
