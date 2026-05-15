//! Engine shell.

use std::{collections::BTreeMap, sync::Arc};

use leaven_core::OptimizationProblem;
use leaven_kernel::{
    Budget, BudgetExceeded, CandidateId, ErrorKind, ErrorRecord, EvaluatorId, IterationId, RunId,
    StageId,
};
use leaven_store::EvidenceStore;

use crate::{
    BudgetLedger, Callback, CaseSet, CheckpointContext, DynCallback, DynEvaluator, DynStopper,
    ErrorPolicy, EvaluationCache, Evaluator, Optimizer, OptimizerError, OptimizerStateWrite,
    ReadScope, RunCheckpointRequest, RunContext, RunContextError, RunEvent, RunGraph, RunGraphView,
    RunPersistence, StepStatus, StopReason, Stopper, TrustPolicy,
};

pub struct Engine<P: OptimizationProblem> {
    graph: RunGraph<P>,
    budget: BudgetLedger,
    cache: EvaluationCache,
    evaluators: BTreeMap<EvaluatorId, Arc<dyn DynEvaluator<P>>>,
    callbacks: Vec<Box<dyn DynCallback<P>>>,
    stoppers: Vec<EngineStopper<P>>,
    persistence: Option<Arc<dyn RunPersistence<P>>>,
    trust: TrustPolicy,
}

impl<P: OptimizationProblem> Engine<P> {
    #[must_use]
    pub fn builder() -> EngineBuilder<P> {
        EngineBuilder::default()
    }

    #[must_use]
    pub fn graph(&self) -> &RunGraph<P> {
        &self.graph
    }

    #[must_use]
    pub fn view(&self) -> RunGraphView<'_, P> {
        self.graph.view(ReadScope::default())
    }

    #[must_use]
    pub fn budget(&self) -> &BudgetLedger {
        &self.budget
    }

    /// Replaces the budget limit while preserving already-spent ledger state.
    pub fn set_budget_limit(&mut self, budget: Budget) {
        self.budget.set_limit(budget);
    }

    pub fn insert_seed(
        &mut self,
        artifact: P::Artifact,
        seed_index: usize,
    ) -> Result<CandidateId, RunContextError> {
        let candidate =
            RunContext::new(&mut self.graph, &mut self.budget).insert_seed(artifact, seed_index)?;
        self.checkpoint(None)
            .map_err(RunContextError::Persistence)?;
        Ok(candidate)
    }

    pub async fn run<O>(
        &mut self,
        optimizer: &mut O,
        case_set: &CaseSet<P::Case>,
        evidence_store: &dyn EvidenceStore<P::Evidence>,
    ) -> Result<RunResult, OptimizerError>
    where
        O: Optimizer<P>,
    {
        let run_id = self.graph.run_id;
        self.emit(RunEvent::OptimizationStarted { run_id });

        {
            let mut ctx = RunContext::new(&mut self.graph, &mut self.budget)
                .with_case_set(case_set)
                .with_cache(&mut self.cache)
                .with_evidence_store(evidence_store)
                .with_evaluators(&self.evaluators)
                .with_trust_policy(self.trust.clone())
                .with_callbacks(self.callbacks.as_mut_slice())
                .with_persistence(self.persistence.as_deref());
            if let Err(error) = optimizer.initialize(&mut ctx).await {
                self.record_optimizer_error(&error);
                return Err(error);
            }
        }
        if let Err(error) =
            self.checkpoint_optimizer(optimizer, "run checkpoint failed after initialize")
        {
            self.record_optimizer_error(&error);
            return Err(error);
        }

        for _ in 0..MAX_ITERATIONS {
            if let Some(reason) = self.stop_reason() {
                return self.finish_clean_stop(optimizer, reason);
            }

            let iteration = IterationId::new();
            self.emit(RunEvent::IterationStarted { iteration });
            let status = {
                let mut ctx = RunContext::new(&mut self.graph, &mut self.budget)
                    .with_case_set(case_set)
                    .with_cache(&mut self.cache)
                    .with_evidence_store(evidence_store)
                    .with_evaluators(&self.evaluators)
                    .with_trust_policy(self.trust.clone())
                    .with_callbacks(self.callbacks.as_mut_slice())
                    .with_persistence(self.persistence.as_deref())
                    .with_iteration(iteration);
                optimizer.step(&mut ctx).await
            };
            self.emit(RunEvent::IterationEnded { iteration });
            if let Err(error) =
                self.checkpoint_optimizer(optimizer, "run checkpoint failed after iteration")
            {
                self.record_optimizer_error(&error);
                return Err(error);
            }
            match status {
                Ok(StepStatus::Continue) => {}
                Ok(StepStatus::Done) => {
                    return self.finish_clean_stop(optimizer, StopReason::OptimizerDone);
                }
                Err(error) => {
                    self.record_optimizer_error(&error);
                    return Err(error);
                }
            }
        }

        let error =
            OptimizerError::Message(format!("optimizer exceeded {MAX_ITERATIONS} iterations"));
        self.record_optimizer_error(&error);
        Err(error)
    }

    /// Evaluate a request after or outside an optimizer step using the
    /// engine-owned evaluator registry and stores.
    pub async fn evaluate(
        &mut self,
        evaluator_id: leaven_kernel::EvaluatorId,
        request: leaven_core::EvaluationRequest,
        case_set: &CaseSet<P::Case>,
        evidence_store: &dyn leaven_store::EvidenceStore<P::Evidence>,
    ) -> Result<crate::EvaluationReport, RunContextError> {
        let mut ctx = RunContext::new(&mut self.graph, &mut self.budget)
            .with_case_set(case_set)
            .with_cache(&mut self.cache)
            .with_evidence_store(evidence_store)
            .with_evaluators(&self.evaluators)
            .with_trust_policy(self.trust.clone())
            .with_callbacks(self.callbacks.as_mut_slice())
            .with_persistence(self.persistence.as_deref());
        ctx.evaluate(evaluator_id, request).await
    }

    fn finish_clean_stop<O>(
        &mut self,
        optimizer: &O,
        reason: StopReason,
    ) -> Result<RunResult, OptimizerError>
    where
        O: Optimizer<P>,
    {
        let optimizer_state = match self.optimizer_state_write(optimizer) {
            Ok(state) => state,
            Err(error) => {
                self.record_optimizer_error(&error);
                return Err(error);
            }
        };
        self.emit(RunEvent::OptimizationStopping { reason });
        self.finish(optimizer.best_candidate(self.view()), optimizer_state)
            .map_err(|error| {
                let error =
                    OptimizerError::with_source("run checkpoint failed after finish", error);
                self.emit(RunEvent::Error {
                    stage: Some(StageId::custom("optimizer")),
                    error: ErrorRecord::from_error(ErrorKind::Optimizer, &error),
                    policy: ErrorPolicy::StoppedRun,
                });
                self.emit(RunEvent::OptimizationStopping {
                    reason: StopReason::Error,
                });
                error
            })
    }

    fn finish(
        &mut self,
        best: Option<CandidateId>,
        optimizer_state: Option<OptimizerStateWrite>,
    ) -> Result<RunResult, crate::RunPersistenceError> {
        let run_id = self.graph.run_id;
        let budget = self.budget.snapshot();
        self.emit(RunEvent::OptimizationEnded {
            run_id,
            best,
            budget,
        });
        self.checkpoint(optimizer_state)?;
        Ok(RunResult { run_id, best })
    }

    fn stop_reason(&self) -> Option<StopReason> {
        let budget = self.budget.snapshot();
        self.stoppers.iter().find_map(|stopper| match stopper {
            EngineStopper::External(stopper) => stopper
                .should_stop_dyn(self.graph.view(self.trust.callback_read_scope()))
                .then_some(StopReason::StopperTriggered),
            EngineStopper::MetricCalls(limit) => {
                (budget.spent.metric_calls >= *limit).then_some(StopReason::BudgetReached)
            }
        })
    }

    fn checkpoint(
        &self,
        optimizer_state: Option<OptimizerStateWrite>,
    ) -> Result<(), crate::RunPersistenceError> {
        if let Some(persistence) = &self.persistence {
            let mut request =
                RunCheckpointRequest::new(&self.graph, &self.budget, Some(&self.cache));
            if let Some(state) = optimizer_state {
                request = request.with_optimizer_state(state);
            }
            persistence.checkpoint(request)?;
        }
        Ok(())
    }

    fn checkpoint_optimizer<O>(
        &self,
        optimizer: &O,
        failure_message: &'static str,
    ) -> Result<(), OptimizerError>
    where
        O: Optimizer<P>,
    {
        let optimizer_state = self.optimizer_state_write(optimizer)?;
        self.checkpoint(optimizer_state)
            .map_err(|error| OptimizerError::with_source(failure_message, error))
    }

    fn optimizer_state_write<O>(
        &self,
        optimizer: &O,
    ) -> Result<Option<OptimizerStateWrite>, OptimizerError>
    where
        O: Optimizer<P>,
    {
        optimizer.checkpoint_state_write(CheckpointContext::new(self.view()))
    }

    fn record_optimizer_error(&mut self, error: &OptimizerError) {
        if let Some(budget_error) = budget_exceeded_source(error) {
            self.emit(RunEvent::Error {
                stage: Some(budget_error.stage.clone()),
                error: ErrorRecord::from_error(ErrorKind::Budget, budget_error),
                policy: ErrorPolicy::StoppedRun,
            });
            self.emit(RunEvent::OptimizationStopping {
                reason: StopReason::BudgetExceeded,
            });
            let _ = self.finish(None, None);
            return;
        }

        self.emit(RunEvent::Error {
            stage: Some(StageId::custom("optimizer")),
            error: ErrorRecord::from_error(ErrorKind::Optimizer, error),
            policy: ErrorPolicy::StoppedRun,
        });
        self.emit(RunEvent::OptimizationStopping {
            reason: StopReason::Error,
        });
        let _ = self.finish(None, None);
    }

    fn emit(&mut self, event: RunEvent) {
        self.graph.record_event(event);
        let event = self
            .graph
            .events
            .last()
            .expect("event was just recorded before callback dispatch");
        for callback in &mut self.callbacks {
            callback.on_event_dyn(event, self.graph.view(self.trust.callback_read_scope()));
        }
    }
}

const MAX_ITERATIONS: usize = 1024;

pub struct EngineBuilder<P: OptimizationProblem> {
    run_id: RunId,
    budget: Budget,
    evaluators: BTreeMap<EvaluatorId, Arc<dyn DynEvaluator<P>>>,
    callbacks: Vec<Box<dyn DynCallback<P>>>,
    stoppers: Vec<EngineStopper<P>>,
    persistence: Option<Arc<dyn RunPersistence<P>>>,
    trust: TrustPolicy,
    _problem: std::marker::PhantomData<P>,
}

impl<P: OptimizationProblem> Default for EngineBuilder<P> {
    fn default() -> Self {
        Self {
            run_id: RunId::new(),
            budget: Budget::unlimited(),
            evaluators: BTreeMap::new(),
            callbacks: Vec::new(),
            stoppers: Vec::new(),
            persistence: None,
            trust: TrustPolicy::default(),
            _problem: std::marker::PhantomData,
        }
    }
}

impl<P: OptimizationProblem> EngineBuilder<P> {
    #[must_use]
    pub fn budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
        self
    }

    #[must_use]
    pub fn callback<C>(mut self, callback: C) -> Self
    where
        C: Callback<P> + 'static,
    {
        self.callbacks.push(Box::new(callback));
        self
    }

    #[must_use]
    pub fn stopper<S>(mut self, stopper: S) -> Self
    where
        S: Stopper<P> + 'static,
    {
        self.stoppers
            .push(EngineStopper::External(Box::new(stopper)));
        self
    }

    #[must_use]
    pub fn metric_call_budget_stopper(mut self, max_metric_calls: u64) -> Self {
        self.stoppers
            .push(EngineStopper::MetricCalls(max_metric_calls));
        self
    }

    #[must_use]
    pub fn evaluator<E>(mut self, evaluator: E) -> Self
    where
        E: Evaluator<P> + 'static,
    {
        let id = evaluator.id();
        self.evaluators.insert(id, Arc::new(evaluator));
        self
    }

    #[must_use]
    pub fn persistence<R>(mut self, persistence: R) -> Self
    where
        R: RunPersistence<P> + 'static,
    {
        self.persistence = Some(Arc::new(persistence));
        self
    }

    /// Set the trust policy used for optimizer contexts and callback views.
    #[must_use]
    pub fn trust_policy(mut self, trust: TrustPolicy) -> Self {
        self.trust = trust;
        self
    }

    #[must_use]
    pub fn build(self) -> Engine<P> {
        Engine {
            graph: RunGraph::new(self.run_id),
            budget: BudgetLedger::new(self.budget),
            cache: EvaluationCache::default(),
            evaluators: self.evaluators,
            callbacks: self.callbacks,
            stoppers: self.stoppers,
            persistence: self.persistence,
            trust: self.trust,
        }
    }
}

enum EngineStopper<P: OptimizationProblem> {
    External(Box<dyn DynStopper<P>>),
    MetricCalls(u64),
}

fn budget_exceeded_source<'a>(
    error: &'a (dyn std::error::Error + 'static),
) -> Option<&'a BudgetExceeded> {
    if let Some(budget_error) = error.downcast_ref::<BudgetExceeded>() {
        return Some(budget_error);
    }
    if let Some(RunContextError::Budget(budget_error)) = error.downcast_ref::<RunContextError>() {
        return Some(budget_error);
    }
    error.source().and_then(budget_exceeded_source)
}

#[derive(Clone, Debug)]
pub struct RunResult {
    pub run_id: RunId,
    pub best: Option<CandidateId>,
}

pub fn optimize<P: OptimizationProblem>() -> EngineBuilder<P> {
    Engine::builder()
}
