use std::sync::Arc;

use serde::{Serialize, de::DeserializeOwned};

use super::{
    Artifact, DatasetCompatibility, EngineRunInputs, EngineStart, EngineStartInputs,
    OptimizeBuilder, OptimizeError, Optimized, Optimizer, RunCompatibilityInputs,
    RunCompatibilityManifest, RunProblem, RuntimeFingerprint, ScoringEvaluator, build_case_plan,
    case_content_fingerprint, case_set_cases, default_evaluation_cache_policy,
    durable_runtime_fingerprints, prepare_run_store, restore_optimizer_checkpoint, run_with_engine,
    scoring_evaluator_identity, search_ledger_budget, start_engine,
};

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
        let engine_budget = search_ledger_budget(budget.clone());
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
        let compatibility = RunCompatibilityManifest::new(RunCompatibilityInputs {
            dataset: DatasetCompatibility::new(case_content, &case_plan.splits),
            runner: runner_fingerprint,
            scorer: scorer_fingerprint,
            evaluator: evaluator_fingerprint,
            optimizer: self.optimizer.optimizer_compatibility(),
            lm_roles: self.lm_role_fingerprints.clone(),
            cache_policy: &evaluation_cache_policy,
            budget: &budget,
        });
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
