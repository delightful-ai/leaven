use futures::executor::block_on;
use leaven::extend::{
    CachePolicy, EvaluationRequest, Evaluator, Optimizer, Proposal, ProposalBatch,
    ProposalBatchSemantics, ProposalContext, Proposer, RunContext, RunEvent,
};
use leaven::plumbing::ContentId;
use leaven::prelude::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget, Budget,
    CandidateId, Cost,
};
use leaven::stdlib::{evidence::ScalarEvidence, populations::KeepBest};
use leaven_core::{EvaluationPurpose, ResolvedEvaluationRequest, ResolvedRequestKind};
use leaven_engine::{
    Callback, CaseSet, EvaluationContext, EvaluationError, OptimizerError, ProposalError,
    RunGraphView, StepStatus,
};
use leaven_kernel::{EvaluatorId, Fingerprint, MetadataBag, Metered, ProposerId};
use leaven_store_inline::InlineEvidenceStore;
use std::sync::{Arc, Mutex};

#[test]
fn engine_runs_scalar_keep_best_end_to_end() {
    block_on(async {
        let store = InlineEvidenceStore::<ScalarEvidence>::new("inline");
        let cases = CaseSet::new(vec![()]);
        let callback = RecordingCallback::default();
        let callback_events = callback.events.clone();
        let callback_candidate_counts = callback.candidate_counts.clone();
        let mut engine = leaven::engine::optimize::<ScalarProblem>()
            .budget(Budget::metric_calls(10))
            .callback(callback)
            .build();
        let seed = engine.insert_seed(TextArtifact("a".to_owned()), 0).unwrap();
        let mut optimizer = ScalarKeepBestOptimizer {
            seed,
            done: false,
            population: KeepBest::new(),
            proposer: TwoMutations,
            evaluator: TextLengthEvaluator,
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        let best = result.best.expect("optimizer should choose a candidate");
        assert_eq!(engine.view().artifact(best).unwrap().0, "aaa");
        assert_eq!(optimizer.population.best(), Some(best));
        assert_eq!(optimizer.population.best_score(), Some(3.0));

        let events = engine.view().events().collect::<Vec<_>>();
        assert_event_subsequence(
            &events,
            &[
                EventKind::OptimizationStarted,
                EventKind::IterationStarted,
                EventKind::BudgetCharged,
                EventKind::ProposalBatchProduced,
                EventKind::ProposalRecorded,
                EventKind::ProposalRecorded,
                EventKind::ApplySucceeded,
                EventKind::ApplySucceeded,
                EventKind::EvaluationRequested,
                EventKind::EvaluationCompleted,
                EventKind::PopulationUpdated,
                EventKind::PopulationUpdated,
                EventKind::IterationEnded,
                EventKind::OptimizationEnded,
            ],
        );
        let callback_events = callback_events.lock().unwrap();
        assert_event_kind_subsequence(
            &callback_events,
            &[
                EventKind::OptimizationStarted,
                EventKind::IterationStarted,
                EventKind::ProposalBatchProduced,
                EventKind::ProposalRecorded,
                EventKind::ApplySucceeded,
                EventKind::EvaluationRequested,
                EventKind::EvaluationCompleted,
                EventKind::PopulationUpdated,
                EventKind::IterationEnded,
                EventKind::OptimizationEnded,
            ],
        );
        assert_eq!(
            callback_events.len(),
            callback_candidate_counts.lock().unwrap().len()
        );
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextArtifact(String);

#[derive(Clone, Debug, Eq, PartialEq)]
enum TextChange {
    Append(&'static str),
}

#[derive(Debug)]
struct TextError;

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("text error")
    }
}

impl std::error::Error for TextError {}

impl Artifact for TextArtifact {
    type Change = TextChange;
    type ApplyError = TextError;

    fn identity(&self) -> ArtifactIdentity {
        let mut bytes = [0; 32];
        let raw = self.0.as_bytes();
        let len = raw.len().min(32);
        bytes[..len].copy_from_slice(&raw[..len]);
        ArtifactIdentity::Content(ContentId::from_bytes(bytes))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        match change {
            TextChange::Append(suffix) => Ok(Self(format!("{}{suffix}", self.0))),
        }
    }
}

struct ScalarProblem;

impl leaven::prelude::OptimizationProblem for ScalarProblem {
    type Artifact = TextArtifact;
    type Case = ();
    type Evidence = ScalarEvidence;
    type ProposalAnnotations = ();
}

struct TwoMutations;

impl Proposer<ScalarProblem> for TwoMutations {
    type Request = CandidateId;

    fn id(&self) -> ProposerId {
        ProposerId::from("two-mutations")
    }

    async fn propose(
        &self,
        target: Self::Request,
        _ctx: ProposalContext<'_, ScalarProblem>,
    ) -> Result<Metered<ProposalBatch<ScalarProblem>>, ProposalError> {
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![
                    Proposal::mutate(target, TextChange::Append("b")).build(),
                    Proposal::mutate(target, TextChange::Append("aa")).build(),
                ],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::llm_calls(1),
        ))
    }
}

struct TextLengthEvaluator;

impl Evaluator<ScalarProblem> for TextLengthEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([1; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'_, ScalarProblem>,
    ) -> Result<Metered<Vec<Assessment<ScalarProblem>>>, EvaluationError> {
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        let assessments = candidates
            .into_iter()
            .map(|candidate| {
                let artifact = ctx
                    .graph()
                    .artifact(candidate)
                    .expect("evaluation candidate exists");
                let score = f64::from(u32::try_from(artifact.0.len()).unwrap());
                Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Unscoped,
                    evidence: ScalarEvidence::new(score).expect("text length score is finite"),
                    cost: Cost::metric_calls(1),
                    metadata: MetadataBag::new(),
                }
            })
            .collect::<Vec<_>>();
        Ok(Metered::new(
            assessments,
            Cost::metric_calls(u64::try_from(request.set.case_ids.len()).unwrap_or(1)),
        ))
    }
}

struct ScalarKeepBestOptimizer {
    seed: CandidateId,
    done: bool,
    population: KeepBest,
    proposer: TwoMutations,
    evaluator: TextLengthEvaluator,
}

impl Optimizer<ScalarProblem> for ScalarKeepBestOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, ScalarProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        if self.done {
            return Ok(StepStatus::Done);
        }
        let proposals = ctx
            .propose(&self.proposer, self.seed)
            .await
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let apply = ctx
            .apply_batch(proposals.batch_id)
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let candidates = apply.successful_candidates().collect::<Vec<_>>();
        let evaluation = ctx
            .evaluate_with(
                &self.evaluator,
                EvaluationRequest::Independent {
                    candidates,
                    set: leaven::extend::EvaluationSet::All,
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::Search,
                },
            )
            .await
            .map_err(|err| OptimizerError::Message(err.to_string()))?;

        for assessment_id in evaluation.assessment_ids {
            let assessment = ctx
                .graph()
                .assessment(assessment_id)
                .expect("assessment should be graph-visible");
            let candidate = assessment
                .independent_candidate()
                .expect("P1 uses independent scalar assessments");
            let evidence = ctx
                .assessment_evidence(assessment_id)
                .map_err(|err| OptimizerError::Message(err.to_string()))?;
            let events = self.population.observe(candidate, assessment_id, evidence);
            ctx.emit(RunEvent::PopulationUpdated {
                population_id: self.population.id(),
                events,
            });
        }
        self.done = true;
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven::extend::RunGraphView<'_, ScalarProblem>,
    ) -> Option<CandidateId> {
        self.population.best()
    }
}

#[derive(Default)]
struct RecordingCallback {
    events: Arc<Mutex<Vec<EventKind>>>,
    candidate_counts: Arc<Mutex<Vec<usize>>>,
}

impl Callback<ScalarProblem> for RecordingCallback {
    fn on_event(&mut self, event: &RunEvent, graph: RunGraphView<'_, ScalarProblem>) {
        if let Some(kind) = EventKind::from_event(event) {
            self.events.lock().unwrap().push(kind);
            self.candidate_counts
                .lock()
                .unwrap()
                .push(graph.candidate_count());
        }
    }
}

#[derive(Clone, Copy)]
enum EventKind {
    OptimizationStarted,
    IterationStarted,
    BudgetCharged,
    ProposalBatchProduced,
    ProposalRecorded,
    ApplySucceeded,
    EvaluationRequested,
    EvaluationCompleted,
    PopulationUpdated,
    IterationEnded,
    OptimizationEnded,
}

impl EventKind {
    fn from_event(event: &RunEvent) -> Option<Self> {
        Some(match event {
            RunEvent::OptimizationStarted { .. } => Self::OptimizationStarted,
            RunEvent::IterationStarted { .. } => Self::IterationStarted,
            RunEvent::BudgetCharged { .. } => Self::BudgetCharged,
            RunEvent::ProposalBatchProduced { .. } => Self::ProposalBatchProduced,
            RunEvent::ProposalRecorded { .. } => Self::ProposalRecorded,
            RunEvent::ApplySucceeded { .. } => Self::ApplySucceeded,
            RunEvent::EvaluationRequested { .. } => Self::EvaluationRequested,
            RunEvent::EvaluationCompleted { .. } => Self::EvaluationCompleted,
            RunEvent::PopulationUpdated { .. } => Self::PopulationUpdated,
            RunEvent::IterationEnded { .. } => Self::IterationEnded,
            RunEvent::OptimizationEnded { .. } => Self::OptimizationEnded,
            RunEvent::OptimizationStopping { .. }
            | RunEvent::StageAttemptRecorded { .. }
            | RunEvent::ApplyFailed { .. }
            | RunEvent::Error { .. } => return None,
        })
    }
}

fn assert_event_subsequence(events: &[&RunEvent], expected: &[EventKind]) {
    let mut cursor = 0;
    for event in events {
        if cursor < expected.len() && event_matches(event, expected[cursor]) {
            cursor += 1;
        }
    }
    assert_eq!(cursor, expected.len(), "missing expected event subsequence");
}

fn assert_event_kind_subsequence(events: &[EventKind], expected: &[EventKind]) {
    let mut cursor = 0;
    for event in events {
        if cursor < expected.len() && *event as u8 == expected[cursor] as u8 {
            cursor += 1;
        }
    }
    assert_eq!(
        cursor,
        expected.len(),
        "missing expected callback event subsequence"
    );
}

fn event_matches(event: &RunEvent, kind: EventKind) -> bool {
    matches!(
        (event, kind),
        (
            RunEvent::OptimizationStarted { .. },
            EventKind::OptimizationStarted
        ) | (
            RunEvent::IterationStarted { .. },
            EventKind::IterationStarted
        ) | (RunEvent::BudgetCharged { .. }, EventKind::BudgetCharged)
            | (
                RunEvent::ProposalBatchProduced { .. },
                EventKind::ProposalBatchProduced
            )
            | (
                RunEvent::ProposalRecorded { .. },
                EventKind::ProposalRecorded
            )
            | (RunEvent::ApplySucceeded { .. }, EventKind::ApplySucceeded)
            | (
                RunEvent::EvaluationRequested { .. },
                EventKind::EvaluationRequested
            )
            | (
                RunEvent::EvaluationCompleted { .. },
                EventKind::EvaluationCompleted
            )
            | (
                RunEvent::PopulationUpdated { .. },
                EventKind::PopulationUpdated
            )
            | (RunEvent::IterationEnded { .. }, EventKind::IterationEnded)
            | (
                RunEvent::OptimizationEnded { .. },
                EventKind::OptimizationEnded
            )
    )
}
