//! Engine shell.

use std::{collections::BTreeMap, sync::Arc};

use leaven_core::OptimizationProblem;
use leaven_kernel::{
    Budget, CandidateId, ErrorKind, ErrorRecord, EvaluatorId, IterationId, RunId, StageId,
};
use leaven_store::EvidenceStore;

use crate::{
    BudgetLedger, Callback, CaseSet, DynCallback, DynEvaluator, ErrorPolicy, EvaluationCache,
    Evaluator, Optimizer, OptimizerError, ReadScope, RunCheckpointRequest, RunContext,
    RunContextError, RunEvent, RunGraph, RunGraphView, RunPersistence, StepStatus, StopReason,
    TrustPolicy,
};

pub struct Engine<P: OptimizationProblem> {
    graph: RunGraph<P>,
    budget: BudgetLedger,
    cache: EvaluationCache,
    evaluators: BTreeMap<EvaluatorId, Arc<dyn DynEvaluator<P>>>,
    callbacks: Vec<Box<dyn DynCallback<P>>>,
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

    pub fn insert_seed(
        &mut self,
        artifact: P::Artifact,
        seed_index: usize,
    ) -> Result<CandidateId, RunContextError> {
        let candidate =
            RunContext::new(&mut self.graph, &mut self.budget).insert_seed(artifact, seed_index)?;
        self.checkpoint().map_err(RunContextError::Persistence)?;
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
        self.checkpoint().map_err(|error| {
            let error =
                OptimizerError::with_source("run checkpoint failed after initialize", error);
            self.record_optimizer_error(&error);
            error
        })?;

        for _ in 0..MAX_ITERATIONS {
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
            self.checkpoint().map_err(|error| {
                let error =
                    OptimizerError::with_source("run checkpoint failed after iteration", error);
                self.record_optimizer_error(&error);
                error
            })?;
            match status {
                Ok(StepStatus::Continue) => {}
                Ok(StepStatus::Done) => {
                    self.emit(RunEvent::OptimizationStopping {
                        reason: StopReason::OptimizerDone,
                    });
                    return self
                        .finish(optimizer.best_candidate(self.view()))
                        .map_err(|error| {
                            let error = OptimizerError::with_source(
                                "run checkpoint failed after finish",
                                error,
                            );
                            self.emit(RunEvent::Error {
                                stage: Some(StageId::custom("optimizer")),
                                error: ErrorRecord::from_error(ErrorKind::Optimizer, &error),
                                policy: ErrorPolicy::StoppedRun,
                            });
                            self.emit(RunEvent::OptimizationStopping {
                                reason: StopReason::Error,
                            });
                            error
                        });
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

    fn finish(
        &mut self,
        best: Option<CandidateId>,
    ) -> Result<RunResult, crate::RunPersistenceError> {
        let run_id = self.graph.run_id;
        let budget = self.budget.snapshot();
        self.emit(RunEvent::OptimizationEnded {
            run_id,
            best,
            budget,
        });
        self.checkpoint()?;
        Ok(RunResult { run_id, best })
    }

    fn checkpoint(&self) -> Result<(), crate::RunPersistenceError> {
        if let Some(persistence) = &self.persistence {
            persistence.checkpoint(RunCheckpointRequest::new(
                &self.graph,
                &self.budget,
                Some(&self.cache),
            ))?;
        }
        Ok(())
    }

    fn record_optimizer_error(&mut self, error: &OptimizerError) {
        self.emit(RunEvent::Error {
            stage: Some(StageId::custom("optimizer")),
            error: ErrorRecord::from_error(ErrorKind::Optimizer, error),
            policy: ErrorPolicy::StoppedRun,
        });
        self.emit(RunEvent::OptimizationStopping {
            reason: StopReason::Error,
        });
        let _ = self.finish(None);
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
            persistence: self.persistence,
            trust: self.trust,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RunResult {
    pub run_id: RunId,
    pub best: Option<CandidateId>,
}

pub fn optimize<P: OptimizationProblem>() -> EngineBuilder<P> {
    Engine::builder()
}
