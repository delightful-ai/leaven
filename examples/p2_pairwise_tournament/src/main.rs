use futures::executor::block_on;
use leaven::extend::{
    CachePolicy, EvaluationRequest, Evaluator, Optimizer, Proposal, ProposalBatch,
    ProposalBatchSemantics, RunEvent,
};
use leaven::plumbing::ContentId;
use leaven::prelude::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget, Budget,
    CandidateId, Cost, PairOrder,
};
use leaven::stdlib::{
    evidence::{PairwiseJudgment, PairwiseJudgmentEvidence},
    populations::{BradleyTerryFit, TournamentPopulation},
};
use leaven_core::{EvaluationPurpose, ResolvedEvaluationRequest, ResolvedRequestKind};
use leaven_engine::{CaseSet, EvaluationContext, EvaluationError, OptimizerError, RunContext};
use leaven_kernel::{EvaluatorId, Fingerprint, FiniteF64, MetadataBag, Metered, StageId};
use leaven_store_inline::InlineEvidenceStore;
use std::convert::Infallible;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    block_on(async {
        let store = InlineEvidenceStore::<PairwiseJudgmentEvidence>::new("inline");
        let cases = CaseSet::new(vec![()]);
        let mut engine = leaven::engine::optimize::<TournamentProblem>()
            .budget(Budget::metric_calls(10))
            .evaluator(LengthPairwiseJudge)
            .build();
        let seed = engine.insert_seed(TextArtifact("a".to_owned()), 0)?;
        let mut optimizer = PairwiseTournamentOptimizer {
            seed,
            done: false,
            population: TournamentPopulation::new(BradleyTerryFit::new(
                FiniteF64::new(0.2).expect("learning rate is finite"),
            )),
        };

        let result = engine.run(&mut optimizer, &cases, &store).await?;
        let winner = result.best.expect("tournament should choose a winner");
        let winner_artifact = engine
            .view()
            .artifact(winner)
            .expect("winner candidate exists");

        assert_eq!(winner_artifact.0, "aaa");
        assert_eq!(optimizer.population.best(), Some(winner));
        assert_eq!(engine.view().assessment_count(), 1);

        println!(
            "p2 pairwise tournament: winner={winner} artifact={} left_ability={} right_ability={}",
            winner_artifact.0,
            optimizer.population.ability(seed).as_f64(),
            optimizer.population.ability(winner).as_f64()
        );
        Ok(())
    })
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

impl leaven::prelude::OptimizationProblem for TournamentProblem {
    type Artifact = TextArtifact;
    type Case = ();
    type Evidence = PairwiseJudgmentEvidence;
    type ProposalAnnotations = ();
}

struct PairwiseTournamentOptimizer {
    seed: CandidateId,
    done: bool,
    population: TournamentPopulation,
}

impl Optimizer<TournamentProblem> for PairwiseTournamentOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, TournamentProblem>,
    ) -> Result<leaven::extend::StepStatus, OptimizerError> {
        if self.done {
            return Ok(leaven::extend::StepStatus::Done);
        }
        let create = ctx
            .record_proposal_batch(
                StageId::custom("p2/create-contender"),
                ProposalBatch {
                    proposals: vec![
                        Proposal::create(TextArtifact("aaa".to_owned()))
                            .informed_by([leaven::extend::InfoRef::Candidate(self.seed)])
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
                    set: leaven::extend::EvaluationSet::All,
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
        self.done = true;
        Ok(leaven::extend::StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven::extend::RunGraphView<'_, TournamentProblem>,
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

fn content_id(bytes: &[u8]) -> ContentId {
    let mut id = [0; ContentId::BYTES];
    let len = bytes.len().min(ContentId::BYTES);
    id[..len].copy_from_slice(&bytes[..len]);
    ContentId::from_bytes(id)
}
