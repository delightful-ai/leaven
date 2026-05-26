use futures::executor::block_on;
use leaven::extend::{
    AssessmentGranularity, AssessmentTarget, CachePolicy, CandidateId, EvaluationRequest,
    Evaluator, Optimizer, Proposal, ProposalBatch, ProposalBatchSemantics, ProposalContext,
    Proposer, RunEvent,
};
use leaven::plumbing::ContentId;
use leaven::prelude::{Artifact, ArtifactIdentity, Assessment, Budget, Cost};
use leaven::stdlib::{evidence::ScalarEvidence, populations::KeepBest};
use leaven_core::{EvaluationPurpose, ResolvedEvaluationRequest, ResolvedRequestKind};
use leaven_engine::{
    CaseSet, EvaluationContext, EvaluationError, OptimizerError, ProposalError, RunContext,
    RunGraphView, StepStatus,
};
use leaven_kernel::{EvaluatorId, Fingerprint, MetadataBag, Metered, ProposerId};
use leaven_store_inline::InlineEvidenceStore;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    block_on(async {
        let store = InlineEvidenceStore::<ScalarEvidence>::new("inline");
        let cases = CaseSet::new(vec![()]);
        let mut engine = leaven::engine::optimize::<ScalarProblem>()
            .budget(Budget::metric_calls(10))
            .build();
        let seed = engine.insert_seed(TextArtifact("a".to_owned()), 0)?;
        let mut optimizer = ScalarKeepBestOptimizer {
            seed,
            done: false,
            population: KeepBest::new(),
            proposer: TwoMutations,
            evaluator: TextLengthEvaluator,
        };

        let result = engine.run(&mut optimizer, &cases, &store).await?;
        let best = result
            .best
            .expect("optimizer should choose a best candidate");
        let best_artifact = engine.view().artifact(best).expect("best candidate exists");

        assert_eq!(best_artifact.0, "aaa");
        assert_eq!(optimizer.population.best(), Some(best));
        assert_eq!(optimizer.population.best_score(), Some(3.0));
        assert_eq!(engine.view().assessment_count(), 2);

        println!(
            "p1 keep-best: best={best} artifact={} score={:?}",
            best_artifact.0,
            optimizer.population.best_score()
        );
        Ok(())
    })
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
        f.write_str("text artifact error")
    }
}

impl std::error::Error for TextError {}

impl Artifact for TextArtifact {
    type Change = TextChange;
    type ApplyError = TextError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::hash_bytes(self.0.as_bytes()))
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
                let score =
                    f64::from(u32::try_from(artifact.0.len()).expect("fixture length fits u32"));
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

    fn best_candidate(&self, _graph: RunGraphView<'_, ScalarProblem>) -> Option<CandidateId> {
        self.population.best()
    }
}
