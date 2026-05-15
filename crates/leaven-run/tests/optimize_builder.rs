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
use leaven_evidence::{CasewiseEvidence, ScoredFeedbackEvidence};
use leaven_kernel::{Budget, CandidateId, ContentId, EvaluatorId};
use leaven_run::{
    OptimizeError, OptimizeStore, RunOutput, RunProblem, Score, ScoreContext, optimize,
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
    assert!(result.report().baseline_train_score.abs() < f64::EPSILON);
    assert!(result.report().optimized_train_score.abs() < f64::EPSILON);
    assert_eq!(result.report().cost.metric_calls, 0);
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
    store: InlineEvidenceStore<CasewiseEvidence<ScoredFeedbackEvidence>>,
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

impl EvidenceStore<CasewiseEvidence<ScoredFeedbackEvidence>> for CountingEvidenceStore {
    fn put(
        &self,
        evidence: CasewiseEvidence<ScoredFeedbackEvidence>,
    ) -> Result<leaven_kernel::EvidenceRef, StoreError> {
        self.inner.puts.fetch_add(1, Ordering::SeqCst);
        self.inner.store.put(evidence)
    }

    fn get(
        &self,
        reference: &leaven_kernel::EvidenceRef,
    ) -> Result<CasewiseEvidence<ScoredFeedbackEvidence>, StoreError> {
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
    RunOutput::new(
        (artifact.0 + case.0).to_string(),
        vec!["runner trace".to_owned()],
    )
}

#[allow(clippy::needless_pass_by_value)]
fn text_score(ctx: ScoreContext<'_, TextArtifact, TextCase>) -> Score {
    let ScoreContext { case, output, .. } = ctx;
    let value = output.output.parse::<f64>().unwrap();
    Score::new(value, format!("case {}", case.0))
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
