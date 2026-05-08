mod support;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use futures::executor::block_on;
use leaven_core::PartitionId;
use leaven_engine::{
    Callback, CaseSet, Engine, Optimizer, OptimizerError, RunContext, RunEvent, RunGraphView,
    RunPersistence, RunPersistenceError, StepStatus, TrustPolicy, optimize,
};
use leaven_kernel::{Budget, ErrorKind};
use leaven_store_inline::InlineEvidenceStore;

use support::{TestEvidence, TestProblem, TextArtifact};

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
        let persistence = CountingPersistence {
            checkpoints: checkpoints.clone(),
        };
        let mut engine = Engine::<TestProblem>::builder()
            .persistence(persistence)
            .build();
        let cases = CaseSet::new(vec!["case"]);
        let store = InlineEvidenceStore::<TestEvidence>::new("inline");
        let mut optimizer = ContinueThenDone { steps: 0 };

        engine.run(&mut optimizer, &cases, &store).await.unwrap();

        assert_eq!(checkpoints.load(Ordering::SeqCst), 4);
    });
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

struct CountingCallback {
    seen: Arc<AtomicUsize>,
}

struct CountingPersistence {
    checkpoints: Arc<AtomicUsize>,
}

impl RunPersistence<TestProblem> for CountingPersistence {
    fn checkpoint(
        &self,
        _graph: &leaven_engine::RunGraph<TestProblem>,
    ) -> Result<(), RunPersistenceError> {
        self.checkpoints.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct FailingPersistence;

impl RunPersistence<TestProblem> for FailingPersistence {
    fn checkpoint(
        &self,
        _graph: &leaven_engine::RunGraph<TestProblem>,
    ) -> Result<(), RunPersistenceError> {
        Err(RunPersistenceError::CheckpointFailed {
            reason: "disk full".to_owned(),
            retryable: Some(true),
        })
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
