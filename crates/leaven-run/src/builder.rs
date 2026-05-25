//! Optimize builder lowering into the engine.

use std::{
    collections::BTreeMap, future::Future, marker::PhantomData, num::NonZeroUsize, path::PathBuf,
    sync::Arc,
};

use futures::{FutureExt, future::BoxFuture};
use leaven_core::{Artifact, OptimizationProblem, PartitionId};
use leaven_engine::{CachePolicy, Callback, Optimizer};
use leaven_eval::{Case, Dataset, DatasetSplits, NoTarget};
use leaven_evidence::CaseAssessmentEvidence;
use leaven_kernel::{Budget, BudgetSnapshot, CandidateId, CheckpointId, Fingerprint, RunId};
use serde::{Serialize, de::DeserializeOwned};

use self::{
    cases::{build_case_plan, case_set_cases, cases_from_inputs},
    engine::{
        EngineRunInputs, EngineStart, EngineStartInputs, default_evaluation_cache_policy,
        durable_runtime_fingerprints, prepare_run_store, run_with_engine,
        scoring_evaluator_identity, search_ledger_budget, start_engine,
    },
    final_eval::{final_evaluation_inputs, run_final_evaluations},
    order::BuilderOrder,
    resume::restore_optimizer_checkpoint,
};
use crate::{
    IntoOptimizeStore, IntoRunResult, OptimizeError, RunCase, RunError, RunOutput, Score,
    ScoreContext, ScoreError,
    compatibility::{
        DatasetCompatibility, RunCompatibilityManifest, RuntimeFingerprint, RuntimeKind,
        ScoringEvaluatorIdentity, case_content_fingerprint, compare_stored_manifest,
        store_fresh_manifest,
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

type Runner<A, I, Out> = Arc<
    dyn Fn(A, RunCase<I>) -> BoxFuture<'static, Result<RunOutput<Out>, RunError>> + Send + Sync,
>;
type Scorer<A, I, T, Out> = Arc<
    dyn Fn(ScoreContext<A, I, T, Out>) -> BoxFuture<'static, Result<Score, ScoreError>>
        + Send
        + Sync,
>;

mod cases;
mod engine;
mod final_eval;
mod order;
mod resume;

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
pub fn optimize<A>(seed: A) -> OptimizeBuilder<A, (), NoTarget, (), ()>
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
        order: BuilderOrder::default(),
    }
}

/// Public optimize/train/validation/test builder.
pub struct OptimizeBuilder<A, I, T, O, Out = ()>
where
    A: Artifact,
    I: Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    seed: A,
    train: Vec<Case<I, T>>,
    validation: Vec<Case<I, T>>,
    test: Vec<Case<I, T>>,
    runner: Runner<A, I, Out>,
    scorer: Option<Scorer<A, I, T, Out>>,
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
    order: BuilderOrder,
}

impl<A> OptimizeBuilder<A, (), NoTarget, (), ()>
where
    A: Artifact,
{
    /// Supplies training case envelopes and fixes the run case type.
    #[must_use]
    pub fn train<I, T>(self, train: Vec<Case<I, T>>) -> OptimizeBuilder<A, I, T, (), ()>
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
            order: self.order,
        }
    }

    /// Supplies input-only toy training cases with dense generated IDs.
    #[must_use]
    pub fn train_inputs<I>(self, train: Vec<I>) -> OptimizeBuilder<A, I, NoTarget, (), ()>
    where
        I: Clone + Send + Sync + 'static,
    {
        self.train(cases_from_inputs(0, train))
    }
}

impl<A, I, T, O, Out> OptimizeBuilder<A, I, T, O, Out>
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
    /// Must be called before [`Self::score`]; changing output type after scoring is a hard error.
    #[must_use]
    pub fn runner<F, Fut, NextOut>(self, runner: F) -> OptimizeBuilder<A, I, T, O, NextOut>
    where
        F: Fn(A, RunCase<I>) -> Fut + Send + Sync + 'static,
        Fut: Future + Send + 'static,
        Fut::Output: IntoRunResult<NextOut>,
        NextOut: Clone + Send + Sync + 'static,
    {
        let order = self.order.runner_after_score(self.scorer.is_some());
        OptimizeBuilder {
            seed: self.seed,
            train: self.train,
            validation: self.validation,
            test: self.test,
            runner: Arc::new(move |artifact, case| {
                let output = runner(artifact, case);
                async move { output.await.into_run_result() }.boxed()
            }),
            scorer: None,
            runner_fingerprint: self.runner_fingerprint,
            scorer_fingerprint: self.scorer_fingerprint,
            lm_role_fingerprints: self.lm_role_fingerprints,
            optimizer: self.optimizer,
            budget: self.budget,
            evaluation_cache_policy: self.evaluation_cache_policy,
            evaluation_parallelism: self.evaluation_parallelism,
            callbacks: self.callbacks,
            store: self.store,
            run_id: self.run_id,
            order,
        }
    }

    /// Supplies the async scoring function.
    #[must_use]
    pub fn score<F, Fut>(mut self, scorer: F) -> Self
    where
        F: Fn(ScoreContext<A, I, T, Out>) -> Fut + Send + Sync + 'static,
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
    pub fn using<Next>(self, optimizer: Next) -> OptimizeBuilder<A, I, T, Next, Out> {
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
            order: self.order,
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

impl<A, I, O, Out> OptimizeBuilder<A, I, NoTarget, O, Out>
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

impl<A, I, T, O, Out> OptimizeBuilder<A, I, T, O, Out>
where
    A: Artifact + Serialize + DeserializeOwned,
    <A as Artifact>::Change: Serialize + DeserializeOwned,
    I: Clone + Serialize + Send + Sync + 'static,
    T: Clone + Serialize + Send + Sync + 'static,
    O: Optimizer<RunProblem<A, I, T>>,
    Out: Clone + Send + Sync + 'static,
{
    /// Runs the optimization.
    pub async fn run(mut self) -> Result<Optimized<A>, OptimizeError> {
        self.order.check()?;
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
            restore_optimizer_checkpoint(
                &mut self.optimizer,
                &checkpoint,
                &prepared_store,
                engine.view(),
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
