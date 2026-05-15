use std::{
    num::NonZeroUsize,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use futures::executor::block_on;
use leaven_core::{
    Artifact, ArtifactIdentity, AssessmentGranularity, EvaluationPurpose, EvaluationRequest,
    EvaluationSet,
};
use leaven_engine::{
    Callback, Optimizer, OptimizerError, RunCheckpointRequest, RunContext, RunEvent, RunGraphView,
    RunPersistence, RunPersistenceError, StepStatus,
};
use leaven_evidence::{CaseAssessmentEvidence, CasewiseEvidence};
use leaven_kernel::{Budget, CandidateId, CaseId, ContentId, EvaluatorId};
use leaven_run::{
    OptimizationStopReason, OptimizeError, OptimizeStore, RunOutput, RunProblem, RunStorage, Score,
    ScoreContext, ScoreError, optimize,
};
use leaven_store::{EvidenceStore, StoreError};
use leaven_store_inline::InlineEvidenceStore;

#[test]
fn run_builder_requires_explicit_budget() {
    let error = block_on(
        optimize(TextArtifact(40))
            .train(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .run(),
    )
    .unwrap_err();

    assert!(matches!(error, OptimizeError::MissingBudget));
}

#[test]
fn run_builder_accepts_explicit_unlimited_budget() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .evaluation_parallelism(NonZeroUsize::new(1).unwrap())
            .run(),
    )
    .unwrap();

    assert_eq!(result.best(), &TextArtifact(40));
    assert_eq!(
        result.report().stop_reason,
        OptimizationStopReason::OptimizerDone
    );
    assert_eq!(
        result.report().storage,
        RunStorage::Ephemeral {
            run_id: result.run_id
        }
    );
}

#[test]
fn public_stop_reason_preserves_all_engine_stop_variants() {
    let cases = [
        (
            leaven_engine::StopReason::OptimizerDone,
            OptimizationStopReason::OptimizerDone,
        ),
        (
            leaven_engine::StopReason::BudgetReached,
            OptimizationStopReason::BudgetReached,
        ),
        (
            leaven_engine::StopReason::BudgetExceeded,
            OptimizationStopReason::BudgetExceeded,
        ),
        (
            leaven_engine::StopReason::StopperTriggered,
            OptimizationStopReason::StopperTriggered,
        ),
        (
            leaven_engine::StopReason::External,
            OptimizationStopReason::External,
        ),
        (
            leaven_engine::StopReason::Error,
            OptimizationStopReason::Error,
        ),
    ];

    for (engine_reason, public_reason) in cases {
        assert_eq!(OptimizationStopReason::from(engine_reason), public_reason);
    }
}

#[test]
fn run_builder_reports_final_train_scores_when_optimizer_does_not_evaluate_train() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::unlimited())
            .evaluation_parallelism(NonZeroUsize::new(1).unwrap())
            .run(),
    )
    .unwrap();

    assert_eq!(result.report().baseline_train_score, Some(42.0));
    assert_eq!(result.report().optimized_train_score, Some(42.0));
}

#[test]
fn run_builder_accepts_cloned_evidence_only_store() {
    let evidence_store = CountingEvidenceStore::new("builder-evidence-only");
    let store =
        OptimizeStore::<RunProblem<TextArtifact, TextCase>>::evidence(evidence_store.clone());
    let cloned_store = store.clone();

    let result = block_on(
        optimize(TextArtifact(40))
            .train(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(EvaluateSeed::default())
            .budget(Budget::metric_calls(16))
            .store(store)
            .run(),
    )
    .unwrap();
    let cloned_result = block_on(
        optimize(TextArtifact(41))
            .train(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(EvaluateSeed::default())
            .budget(Budget::metric_calls(16))
            .store(cloned_store)
            .run(),
    )
    .unwrap();

    assert_eq!(result.best(), &TextArtifact(40));
    assert_eq!(cloned_result.best(), &TextArtifact(41));
    assert!(evidence_store.puts() > 0);
    assert!(evidence_store.gets() > 0);
}

#[test]
fn run_builder_rejects_held_out_cases_without_train_cases() {
    let error = block_on(
        optimize(TextArtifact(40))
            .train(Vec::<TextCase>::new())
            .validation(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(SeedBest::default())
            .budget(Budget::metric_calls(8))
            .run(),
    )
    .unwrap_err();

    assert!(matches!(error, OptimizeError::HeldOutWithoutTrain));
}

#[test]
fn run_builder_accepts_empty_train_when_no_held_out_sets_exist() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train(Vec::<TextCase>::new())
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(EvaluateSeed::default())
            .budget(Budget::metric_calls(8))
            .run(),
    )
    .unwrap();

    assert_eq!(result.best(), &TextArtifact(40));
    assert_eq!(result.report().baseline_train_score, None);
    assert_eq!(result.report().optimized_train_score, None);
    assert_eq!(result.report().optimization_cost.metric_calls, 0);
    assert_eq!(result.report().final_report_cost.metric_calls, 0);
}

#[test]
fn run_builder_separates_optimization_cost_from_final_report_cost() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train(vec![TextCase(2)])
            .validation(vec![TextCase(3)])
            .test(vec![TextCase(4)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(EvaluateSeed::default())
            .budget(Budget::metric_calls(16))
            .evaluation_parallelism(NonZeroUsize::new(1).unwrap())
            .run(),
    )
    .unwrap();

    assert_eq!(result.report().optimization_budget.spent.metric_calls, 1);
    assert_eq!(result.report().optimization_cost.metric_calls, 1);
    assert_eq!(result.report().baseline_train_score, Some(42.0));
    assert_eq!(result.report().optimized_train_score, Some(42.0));
    assert_eq!(result.report().final_report_cost.metric_calls, 6);
    assert_eq!(result.report().budget.spent.metric_calls, 7);
    assert_eq!(result.report().cost.metric_calls, 7);
    assert_eq!(result.report().baseline_validation_score, Some(43.0));
    assert_eq!(result.report().validation_score, Some(43.0));
    assert_eq!(result.report().baseline_test_score, Some(44.0));
    assert_eq!(result.report().test_score, Some(44.0));
}

#[test]
fn run_builder_reports_budget_stop_reason_from_metric_call_budget() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(ContinueAfterSeedEvaluation::default())
            .budget(Budget::metric_calls(1))
            .evaluation_parallelism(NonZeroUsize::new(1).unwrap())
            .run(),
    )
    .unwrap();

    assert_eq!(result.best(), &TextArtifact(40));
    assert_eq!(
        result.report().stop_reason,
        OptimizationStopReason::BudgetReached
    );
    assert_eq!(result.report().optimization_cost.metric_calls, 1);
    assert_eq!(result.report().final_report_cost.metric_calls, 2);
}

#[test]
fn run_builder_runs_final_reports_after_metric_budget_stop() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train(vec![TextCase(2)])
            .validation(vec![TextCase(3)])
            .test(vec![TextCase(4)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(ContinueAfterSeedEvaluation::default())
            .budget(Budget::metric_calls(1))
            .evaluation_parallelism(NonZeroUsize::new(1).unwrap())
            .run(),
    )
    .unwrap();

    assert_eq!(result.best(), &TextArtifact(40));
    assert_eq!(
        result.report().stop_reason,
        OptimizationStopReason::BudgetReached
    );
    assert_eq!(result.report().optimization_cost.metric_calls, 1);
    assert_eq!(result.report().final_report_cost.metric_calls, 6);
    assert_eq!(result.report().cost.metric_calls, 7);
    assert_eq!(result.report().baseline_validation_score, Some(43.0));
    assert_eq!(result.report().validation_score, Some(43.0));
    assert_eq!(result.report().baseline_test_score, Some(44.0));
    assert_eq!(result.report().test_score, Some(44.0));
}

#[test]
fn run_builder_reports_case_ids_output_and_feedback_for_case_level_rows() {
    let result = block_on(
        optimize(TextArtifact(40))
            .train(vec![TextCase(2), TextCase(3)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(EvaluateSeed::default())
            .budget(Budget::metric_calls(16))
            .evaluation_parallelism(NonZeroUsize::new(1).unwrap())
            .run(),
    )
    .unwrap();

    let train = result
        .report()
        .evaluation
        .splits_reported
        .iter()
        .find(|split| split.partition.0 == "TRAIN")
        .expect("train split is reported");
    let candidate = train
        .candidates
        .iter()
        .find(|candidate| candidate.candidate == result.best)
        .expect("best candidate train summary exists");

    assert_eq!(candidate.average_score, Some(42.5));
    assert_eq!(candidate.cases.len(), 2);
    assert_eq!(candidate.cases[0].case_id, CaseId::from_index(0));
    assert_eq!(candidate.cases[0].output, "42");
    assert_eq!(candidate.cases[0].feedback, "case 2");
    assert_eq!(candidate.cases[1].case_id, CaseId::from_index(1));
    assert_eq!(candidate.cases[1].output, "43");
    assert_eq!(candidate.cases[1].feedback, "case 3");
}

#[test]
fn run_builder_rejects_optimizer_that_reports_no_best_candidate() {
    let error = block_on(
        optimize(TextArtifact(40))
            .train(vec![TextCase(2)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(NoBest)
            .budget(Budget::metric_calls(8))
            .run(),
    )
    .unwrap_err();

    assert!(matches!(error, OptimizeError::Optimizer(_)));
    assert_eq!(
        error.to_string(),
        "optimizer failed: optimizer finished without a best candidate"
    );
}

#[test]
fn run_builder_dispatches_callbacks_and_supplied_store_capabilities() {
    let evidence_store = CountingEvidenceStore::new("builder-test");
    let persistence = CountingPersistence::default();
    let events = Arc::new(Mutex::new(Vec::new()));

    let result = block_on(
        optimize(TextArtifact(40))
            .train(vec![TextCase(2), TextCase(3)])
            .runner(|artifact, case| async move { text_runner(&artifact, &case) })
            .score(text_score)
            .using(EvaluateSeed::default())
            .budget(Budget::metric_calls(32))
            .store(OptimizeStore::durable(
                evidence_store.clone(),
                persistence.clone(),
            ))
            .on_event(RecordingCallback {
                events: Arc::clone(&events),
            })
            .run(),
    )
    .unwrap();

    assert_eq!(result.best(), &TextArtifact(40));
    assert!(evidence_store.puts() > 0);
    assert!(evidence_store.gets() > 0);
    assert!(persistence.checkpoints() > 0);
    assert_eq!(
        result.report().storage,
        RunStorage::Stored {
            run_id: result.run_id,
            resumable: false,
        }
    );
    let events = events.lock().unwrap();
    assert!(events.contains(&"optimization_started"));
    assert!(events.contains(&"optimization_ended"));
}

#[derive(Default)]
struct SeedBest {
    best: Option<CandidateId>,
}

impl Optimizer<RunProblem<TextArtifact, TextCase>> for SeedBest {
    async fn initialize(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<(), OptimizerError> {
        self.best = ctx.graph().candidate_tree().roots().first().copied();
        Ok(())
    }

    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Option<CandidateId> {
        self.best
            .or_else(|| graph.candidate_tree().roots().first().copied())
    }
}

struct NoBest;

impl Optimizer<RunProblem<TextArtifact, TextCase>> for NoBest {
    async fn step(
        &mut self,
        _ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<StepStatus, OptimizerError> {
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: RunGraphView<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Option<CandidateId> {
        None
    }
}

#[derive(Default)]
struct ContinueAfterSeedEvaluation {
    best: Option<CandidateId>,
    evaluated: bool,
}

impl Optimizer<RunProblem<TextArtifact, TextCase>> for ContinueAfterSeedEvaluation {
    async fn initialize(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<(), OptimizerError> {
        self.best = ctx.graph().candidate_tree().roots().first().copied();
        Ok(())
    }

    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<StepStatus, OptimizerError> {
        if self.evaluated {
            return Ok(StepStatus::Continue);
        }
        self.evaluated = true;
        let seed = self
            .best
            .or_else(|| ctx.graph().candidate_tree().roots().first().copied())
            .ok_or_else(|| OptimizerError::Message("missing seed".to_owned()))?;
        ctx.evaluate(
            EvaluatorId::PRIMARY,
            EvaluationRequest::Independent {
                candidates: vec![seed],
                set: EvaluationSet::Partition("TRAIN".into()),
                granularity: AssessmentGranularity::PerCase,
                purpose: EvaluationPurpose::Search,
            },
        )
        .await
        .map_err(|source| OptimizerError::with_source("seed evaluation failed", source))?;
        Ok(StepStatus::Continue)
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Option<CandidateId> {
        self.best
            .or_else(|| graph.candidate_tree().roots().first().copied())
    }
}

#[derive(Default)]
struct EvaluateSeed {
    best: Option<CandidateId>,
}

impl Optimizer<RunProblem<TextArtifact, TextCase>> for EvaluateSeed {
    async fn initialize(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<(), OptimizerError> {
        self.best = ctx.graph().candidate_tree().roots().first().copied();
        Ok(())
    }

    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<StepStatus, OptimizerError> {
        let seed = self
            .best
            .or_else(|| ctx.graph().candidate_tree().roots().first().copied())
            .ok_or_else(|| OptimizerError::Message("missing seed".to_owned()))?;
        ctx.evaluate(
            EvaluatorId::PRIMARY,
            EvaluationRequest::Independent {
                candidates: vec![seed],
                set: EvaluationSet::Partition("TRAIN".into()),
                granularity: AssessmentGranularity::PerCase,
                purpose: EvaluationPurpose::Search,
            },
        )
        .await
        .map_err(|source| OptimizerError::with_source("seed evaluation failed", source))?;
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Option<CandidateId> {
        self.best
            .or_else(|| graph.candidate_tree().roots().first().copied())
    }
}

#[derive(Clone)]
struct CountingEvidenceStore {
    inner: Arc<CountingEvidenceInner>,
}

struct CountingEvidenceInner {
    store: InlineEvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>>,
    puts: AtomicUsize,
    gets: AtomicUsize,
}

impl CountingEvidenceStore {
    fn new(name: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(CountingEvidenceInner {
                store: InlineEvidenceStore::new(name),
                puts: AtomicUsize::new(0),
                gets: AtomicUsize::new(0),
            }),
        }
    }

    fn puts(&self) -> usize {
        self.inner.puts.load(Ordering::SeqCst)
    }

    fn gets(&self) -> usize {
        self.inner.gets.load(Ordering::SeqCst)
    }
}

impl EvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>> for CountingEvidenceStore {
    fn put(
        &self,
        evidence: CasewiseEvidence<CaseAssessmentEvidence>,
    ) -> Result<leaven_kernel::EvidenceRef, StoreError> {
        self.inner.puts.fetch_add(1, Ordering::SeqCst);
        self.inner.store.put(evidence)
    }

    fn get(
        &self,
        reference: &leaven_kernel::EvidenceRef,
    ) -> Result<CasewiseEvidence<CaseAssessmentEvidence>, StoreError> {
        self.inner.gets.fetch_add(1, Ordering::SeqCst);
        self.inner.store.get(reference)
    }
}

#[derive(Clone, Default)]
struct CountingPersistence {
    checkpoints: Arc<AtomicUsize>,
}

impl CountingPersistence {
    fn checkpoints(&self) -> usize {
        self.checkpoints.load(Ordering::SeqCst)
    }
}

impl RunPersistence<RunProblem<TextArtifact, TextCase>> for CountingPersistence {
    fn checkpoint(
        &self,
        _request: RunCheckpointRequest<'_, RunProblem<TextArtifact, TextCase>>,
    ) -> Result<(), RunPersistenceError> {
        self.checkpoints.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct RecordingCallback {
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl Callback<RunProblem<TextArtifact, TextCase>> for RecordingCallback {
    fn on_event(
        &mut self,
        event: &RunEvent,
        _graph: RunGraphView<'_, RunProblem<TextArtifact, TextCase>>,
    ) {
        let name = match event {
            RunEvent::OptimizationStarted { .. } => "optimization_started",
            RunEvent::OptimizationEnded { .. } => "optimization_ended",
            _ => "other",
        };
        self.events.lock().unwrap().push(name);
    }
}

fn text_runner(artifact: &TextArtifact, case: &TextCase) -> RunOutput {
    RunOutput::new((artifact.0 + case.0).to_string())
}

#[allow(clippy::needless_pass_by_value)]
async fn text_score(ctx: ScoreContext<TextArtifact, TextCase>) -> Result<Score, ScoreError> {
    let ScoreContext { case, output, .. } = ctx;
    let value = output.output.parse::<f64>().unwrap();
    Ok(Score::new(value, format!("case {}", case.0)))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextArtifact(i32);

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextCase(i32);

#[derive(Debug)]
struct TextArtifactError;

impl std::fmt::Display for TextArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("text artifact error")
    }
}

impl std::error::Error for TextArtifactError {}

impl Artifact for TextArtifact {
    type Change = i32;
    type ApplyError = TextArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        let mut bytes = [0; ContentId::BYTES];
        bytes[..std::mem::size_of::<i32>()].copy_from_slice(&self.0.to_le_bytes());
        ArtifactIdentity::Content(ContentId::from_bytes(bytes))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(*change))
    }
}
