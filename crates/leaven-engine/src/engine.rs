//! Engine shell.

use leaven_core::OptimizationProblem;
use leaven_kernel::{Budget, CandidateId, ErrorKind, ErrorRecord, IterationId, RunId, StageId};
use leaven_store::EvidenceStore;

use crate::{
    BudgetLedger, Callback, CaseSet, DynCallback, ErrorPolicy, EvaluationCache, Optimizer,
    OptimizerError, ReadScope, RunContext, RunContextError, RunEvent, RunGraph, RunGraphView,
    StepStatus, StopReason,
};

pub struct Engine<P: OptimizationProblem> {
    graph: RunGraph<P>,
    budget: BudgetLedger,
    cache: EvaluationCache,
    callbacks: Vec<Box<dyn DynCallback<P>>>,
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
        RunContext::new(&mut self.graph, &mut self.budget).insert_seed(artifact, seed_index)
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
                .with_callbacks(self.callbacks.as_mut_slice());
            if let Err(error) = optimizer.initialize(&mut ctx).await {
                self.record_optimizer_error(&error);
                return Err(error);
            }
        }

        for _ in 0..MAX_ITERATIONS {
            let iteration = IterationId::new();
            self.emit(RunEvent::IterationStarted { iteration });
            let status = {
                let mut ctx = RunContext::new(&mut self.graph, &mut self.budget)
                    .with_case_set(case_set)
                    .with_cache(&mut self.cache)
                    .with_evidence_store(evidence_store)
                    .with_callbacks(self.callbacks.as_mut_slice())
                    .with_iteration(iteration);
                optimizer.step(&mut ctx).await
            };
            self.emit(RunEvent::IterationEnded { iteration });
            match status {
                Ok(StepStatus::Continue) => {}
                Ok(StepStatus::Done) => {
                    self.emit(RunEvent::OptimizationStopping {
                        reason: StopReason::OptimizerDone,
                    });
                    return Ok(self.finish(optimizer.best_candidate(self.view())));
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

    fn finish(&mut self, best: Option<CandidateId>) -> RunResult {
        let run_id = self.graph.run_id;
        let budget = self.budget.snapshot();
        self.emit(RunEvent::OptimizationEnded {
            run_id,
            best,
            budget,
        });
        RunResult { run_id, best }
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
            callback.on_event_dyn(event, self.graph.view(ReadScope::default()));
        }
    }
}

const MAX_ITERATIONS: usize = 1024;

pub struct EngineBuilder<P: OptimizationProblem> {
    run_id: RunId,
    budget: Budget,
    callbacks: Vec<Box<dyn DynCallback<P>>>,
    _problem: std::marker::PhantomData<P>,
}

impl<P: OptimizationProblem> Default for EngineBuilder<P> {
    fn default() -> Self {
        Self {
            run_id: RunId::new(),
            budget: Budget::unlimited(),
            callbacks: Vec::new(),
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
    pub fn build(self) -> Engine<P> {
        Engine {
            graph: RunGraph::new(self.run_id),
            budget: BudgetLedger::new(self.budget),
            cache: EvaluationCache::default(),
            callbacks: self.callbacks,
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
