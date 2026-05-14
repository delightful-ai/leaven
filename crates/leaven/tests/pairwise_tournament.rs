use futures::executor::block_on;
use leaven::stdlib::{
    evidence::{PairwiseJudgment, PairwiseJudgmentEvidence},
    populations::{BradleyTerryFit, TournamentPopulation},
};
use leaven::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget, Budget,
    CachePolicy, CandidateId, ContentId, Cost, EvaluationRequest, Evaluator, InfoRef, Optimizer,
    PairOrder, Proposal, ProposalBatch, ProposalBatchSemantics, RunEvent,
};
use leaven_core::{EvaluationPurpose, ResolvedEvaluationRequest, ResolvedRequestKind};
use leaven_engine::{CaseSet, EvaluationContext, EvaluationError, OptimizerError, RunContext};
use leaven_kernel::{
    AssessmentId, EvaluatorId, Fingerprint, FiniteF64, MetadataBag, Metered, StageId,
};
use leaven_store_inline::InlineEvidenceStore;
use std::convert::Infallible;

#[test]
fn engine_runs_pairwise_tournament_end_to_end() {
    block_on(async {
        let store = InlineEvidenceStore::<PairwiseJudgmentEvidence>::new("inline");
        let cases = CaseSet::new(vec![()]);
        let mut engine = leaven::engine::optimize::<TournamentProblem>()
            .budget(Budget::metric_calls(10))
            .evaluator(LengthPairwiseJudge)
            .build();
        let seed = engine.insert_seed(TextArtifact("a".to_owned()), 0).unwrap();
        let mut optimizer = PairwiseTournamentOptimizer {
            seed,
            done: false,
            population: TournamentPopulation::new(BradleyTerryFit::new(
                FiniteF64::new(0.2).unwrap(),
            )),
            contender: None,
            assessment: None,
            judgment: None,
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        let winner = result.best.expect("tournament should choose a winner");
        let contender = optimizer.contender.expect("contender should be created");
        let assessment_id = optimizer.assessment.expect("assessment should be recorded");
        let assessment = engine
            .view()
            .assessment(assessment_id)
            .expect("assessment should be graph visible");
        assert_eq!(winner, contender);
        assert_eq!(engine.view().artifact(winner).unwrap().0, "aaa");
        assert_eq!(optimizer.population.best(), Some(winner));
        assert_eq!(optimizer.judgment, Some(PairwiseJudgment::Right));
        assert_eq!(engine.view().evaluation_request_count(), 1);
        assert_eq!(engine.view().assessment_count(), 1);
        assert_eq!(assessment.pairwise_candidates(), Some((seed, contender)));
        assert_eq!(
            engine.view().pairwise_assessments(seed, contender).ids(),
            vec![assessment_id]
        );
        assert_event_subsequence(
            &engine.view().events().collect::<Vec<_>>(),
            &[
                EventKind::OptimizationStarted,
                EventKind::IterationStarted,
                EventKind::BudgetCharged,
                EventKind::ProposalBatchProduced,
                EventKind::ProposalRecorded,
                EventKind::ApplySucceeded,
                EventKind::EvaluationRequested,
                EventKind::EvaluationCompleted,
                EventKind::PopulationUpdated,
                EventKind::OptimizationEnded,
            ],
        );
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextArtifact(String);

impl Artifact for TextArtifact {
    type Change = String;
    type ApplyError = Infallible;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(content_id(self.0.as_bytes()))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(change.clone()))
    }
}

struct TournamentProblem;

impl leaven::OptimizationProblem for TournamentProblem {
    type Artifact = TextArtifact;
    type Case = ();
    type Evidence = PairwiseJudgmentEvidence;
    type ProposalAnnotations = ();
}

struct PairwiseTournamentOptimizer {
    seed: CandidateId,
    done: bool,
    population: TournamentPopulation,
    contender: Option<CandidateId>,
    assessment: Option<AssessmentId>,
    judgment: Option<PairwiseJudgment>,
}

impl Optimizer<TournamentProblem> for PairwiseTournamentOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TournamentProblem>,
    ) -> Result<leaven::StepStatus, OptimizerError> {
        if self.done {
            return Ok(leaven::StepStatus::Done);
        }
        let create = ctx
            .record_proposal_batch(
                StageId::custom("p2/create-contender"),
                ProposalBatch {
                    proposals: vec![
                        Proposal::create(TextArtifact("aaa".to_owned()))
                            .informed_by([InfoRef::Candidate(self.seed)])
                            .build(),
                    ],
                    semantics: ProposalBatchSemantics::Alternatives,
                    metadata: MetadataBag::new(),
                },
                Cost::metric_calls(1),
            )
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let applied = ctx
            .apply_batch(create.batch_id)
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let contender = applied
            .successful_candidates()
            .next()
            .expect("created contender should apply");

        let evaluation = ctx
            .evaluate(
                EvaluatorId::PAIRWISE_JUDGE,
                EvaluationRequest::Pairwise {
                    left: self.seed,
                    right: contender,
                    set: leaven::EvaluationSet::All,
                    granularity: AssessmentGranularity::Aggregate,
                    purpose: EvaluationPurpose::Selection,
                    order: PairOrder::Ordered,
                },
            )
            .await
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        assert_eq!(evaluation.assessment_ids.len(), 1);
        let assessment = evaluation.assessment_ids[0];
        let evidence = ctx
            .assessment_evidence(assessment)
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let events = self
            .population
            .observe_pairwise(self.seed, contender, assessment, &evidence);
        ctx.emit(RunEvent::PopulationUpdated {
            population_id: self.population.id(),
            events,
        });
        self.contender = Some(contender);
        self.assessment = Some(assessment);
        self.judgment = Some(evidence.judgment());
        self.done = true;
        Ok(leaven::StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven::RunGraphView<'_, TournamentProblem>,
    ) -> Option<CandidateId> {
        self.population.best()
    }
}

struct LengthPairwiseJudge;

impl Evaluator<TournamentProblem> for LengthPairwiseJudge {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PAIRWISE_JUDGE
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([2; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'_, TournamentProblem>,
    ) -> Result<Metered<Vec<Assessment<TournamentProblem>>>, EvaluationError> {
        let ResolvedRequestKind::Pairwise { left, right, order } = request.kind else {
            return Err(EvaluationError::Message(
                "expected pairwise request".to_owned(),
            ));
        };
        assert_eq!(order, PairOrder::Ordered);
        let left_text = ctx.graph().artifact(left).expect("left exists");
        let right_text = ctx.graph().artifact(right).expect("right exists");
        let judgment = match left_text.0.len().cmp(&right_text.0.len()) {
            std::cmp::Ordering::Less => PairwiseJudgment::Right,
            std::cmp::Ordering::Equal => PairwiseJudgment::Tie,
            std::cmp::Ordering::Greater => PairwiseJudgment::Left,
        };

        Ok(Metered::new(
            vec![Assessment::Pairwise {
                left,
                right,
                target: AssessmentTarget::Unscoped,
                evidence: PairwiseJudgmentEvidence::with_rationale(
                    judgment,
                    "longer text wins this deterministic fixture",
                ),
                cost: Cost::metric_calls(1),
                metadata: MetadataBag::new(),
            }],
            Cost::metric_calls(1),
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    OptimizationEnded,
}

impl EventKind {
    fn from_event(event: &RunEvent) -> Option<Self> {
        match event {
            RunEvent::OptimizationStarted { .. } => Some(Self::OptimizationStarted),
            RunEvent::IterationStarted { .. } => Some(Self::IterationStarted),
            RunEvent::BudgetCharged { .. } => Some(Self::BudgetCharged),
            RunEvent::ProposalBatchProduced { .. } => Some(Self::ProposalBatchProduced),
            RunEvent::ProposalRecorded { .. } => Some(Self::ProposalRecorded),
            RunEvent::ApplySucceeded { .. } => Some(Self::ApplySucceeded),
            RunEvent::EvaluationRequested { .. } => Some(Self::EvaluationRequested),
            RunEvent::EvaluationCompleted { .. } => Some(Self::EvaluationCompleted),
            RunEvent::PopulationUpdated { .. } => Some(Self::PopulationUpdated),
            RunEvent::OptimizationEnded { .. } => Some(Self::OptimizationEnded),
            RunEvent::ApplyFailed { .. }
            | RunEvent::Error { .. }
            | RunEvent::IterationEnded { .. }
            | RunEvent::StageAttemptRecorded { .. }
            | RunEvent::OptimizationStopping { .. } => None,
        }
    }
}

fn assert_event_subsequence(events: &[&RunEvent], expected: &[EventKind]) {
    let mut cursor = 0;
    for event in events {
        if EventKind::from_event(event).is_some_and(|actual| actual == expected[cursor]) {
            cursor += 1;
            if cursor == expected.len() {
                return;
            }
        }
    }
    panic!("missing expected event subsequence at index {cursor}");
}

fn content_id(bytes: &[u8]) -> ContentId {
    let mut id = [0; ContentId::BYTES];
    let len = bytes.len().min(ContentId::BYTES);
    id[..len].copy_from_slice(&bytes[..len]);
    ContentId::from_bytes(id)
}
