use std::collections::{BTreeMap, BTreeSet};

use futures::executor::block_on;
use leaven::extend::{
    AssessmentGranularity, AssessmentTarget, CachePolicy, CandidateId, EvaluationRequest,
    Evaluator, Optimizer, Proposal, ProposalBatch, ProposalBatchSemantics, RunEvent, TrustPolicy,
};
use leaven::plumbing::ContentId;
use leaven::prelude::{Artifact, ArtifactIdentity, Assessment, Budget, Cost};
use leaven_core::{
    EvaluationPurpose, EvaluationSet, OptimizationProblem, PartitionId, ResolvedEvaluationRequest,
    ResolvedRequestKind,
};
use leaven_engine::{CaseSet, EvaluationContext, EvaluationError, OptimizerError, RunContext};
use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence};
use leaven_gepa::{GateDecision, Gepa, SurfaceProposer, test_support::FixedSurfaceEdit};
use leaven_kernel::{CaseId, EvaluatorId, Fingerprint, MetadataBag, Metered, StageId};
use leaven_population::ParetoFrontier;
use leaven_store_inline::InlineEvidenceStore;
use leaven_surface::{EditSurface, Part, PartAddress, SurfaceError, SurfaceFingerprint};

const TRAIN: &str = "TRAIN";
const VALIDATION: &str = "VALIDATION";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    block_on(async {
        let store = InlineEvidenceStore::<CasewiseEvidence<ScalarEvidence>>::new("inline");
        let cases = CaseSet::new(vec![CaseSpec, CaseSpec, CaseSpec])
            .with_partition(
                PartitionId::from(TRAIN),
                vec![CaseId::new(0), CaseId::new(1)],
            )
            .with_partition(PartitionId::from(VALIDATION), vec![CaseId::new(2)]);
        let mut engine = leaven::engine::optimize::<PartMapProblem>()
            .budget(Budget::metric_calls(20))
            .trust_policy(
                TrustPolicy::default()
                    .hide_from_optimizers([PartitionId::from(VALIDATION)])
                    .hide_from_proposers([PartitionId::from(VALIDATION)]),
            )
            .evaluator(PartMapEvaluator)
            .build();
        let seed = engine.insert_seed(
            PartMapArtifact(BTreeMap::from([
                ("answer".to_owned(), "draft answer".to_owned()),
                ("search".to_owned(), "stable search query".to_owned()),
            ])),
            0,
        )?;
        let mut optimizer = GepaParityOptimizer {
            gepa: Gepa::new(
                PartMapSurface,
                ParetoFrontier::by_case()
                    .partition_filter(BTreeSet::from([PartitionId::from(TRAIN)]))
                    .build(),
                FixedSurfaceEdit::new(PartMapEdit::Replace("improved answer".to_owned())),
            ),
            proposer: FixedSurfaceEdit::new(PartMapEdit::Replace("improved answer".to_owned())),
            seed,
            best: None,
            done: false,
        };

        let result = engine.run(&mut optimizer, &cases, &store).await?;
        let best = result.best.expect("gepa parity should choose a winner");
        let best_artifact = engine.view().artifact(best).expect("best exists");

        assert_eq!(optimizer.best, Some(best));
        assert_eq!(best_artifact.0.get("answer").unwrap(), "improved answer");
        assert_eq!(
            best_artifact.0.get("search").unwrap(),
            "stable search query"
        );
        assert_eq!(engine.view().evaluation_request_count(), 2);
        assert_eq!(engine.view().assessment_count(), 2);

        println!(
            "p3 gepa parity: seed={seed} best={best} answer={}",
            best_artifact.0.get("answer").unwrap()
        );
        Ok(())
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PartMapArtifact(BTreeMap<String, String>);

#[derive(Clone, Debug, Eq, PartialEq)]
struct PartMapChange {
    part: String,
    value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PartMapEdit {
    Replace(String),
}

#[derive(Debug)]
struct PartMapError;

impl std::fmt::Display for PartMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("part map artifact error")
    }
}

impl std::error::Error for PartMapError {}

impl Artifact for PartMapArtifact {
    type Change = PartMapChange;
    type ApplyError = PartMapError;

    fn identity(&self) -> ArtifactIdentity {
        let bytes = self
            .0
            .iter()
            .flat_map(|(key, value)| [key.as_bytes(), value.as_bytes()].concat())
            .collect::<Vec<_>>();
        ArtifactIdentity::Content(content_id(&bytes))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        let mut next = self.0.clone();
        if !next.contains_key(&change.part) {
            return Err(PartMapError);
        }
        next.insert(change.part.clone(), change.value.clone());
        Ok(Self(next))
    }
}

struct PartMapProblem;

impl OptimizationProblem for PartMapProblem {
    type Artifact = PartMapArtifact;
    type Case = CaseSpec;
    type Evidence = CasewiseEvidence<ScalarEvidence>;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug)]
struct CaseSpec;

#[derive(Clone, Debug)]
struct PartMapSurface;

impl EditSurface<PartMapArtifact> for PartMapSurface {
    type PartId = String;
    type Address = PartAddress;
    type View<'a> = &'a str;
    type Edit = PartMapEdit;

    fn fingerprint(&self) -> SurfaceFingerprint {
        SurfaceFingerprint(Fingerprint::from_bytes([3; 32]))
    }

    fn parts<'a>(
        &self,
        artifact: &'a PartMapArtifact,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        Ok(artifact
            .0
            .iter()
            .map(|(id, value)| Part {
                id: id.clone(),
                address: PartAddress(id.clone()),
                view: value.as_str(),
            })
            .collect())
    }

    fn change_part(
        &self,
        artifact: &PartMapArtifact,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<PartMapChange, SurfaceError> {
        if artifact.0.contains_key(&id) {
            let PartMapEdit::Replace(value) = edit;
            Ok(PartMapChange { part: id, value })
        } else {
            Err(SurfaceError::UnknownPart)
        }
    }
}

struct GepaParityOptimizer {
    gepa: Gepa<PartMapSurface, ParetoFrontier, FixedSurfaceEdit<PartMapEdit>>,
    proposer: FixedSurfaceEdit<PartMapEdit>,
    seed: CandidateId,
    best: Option<CandidateId>,
    done: bool,
}

impl Optimizer<PartMapProblem> for GepaParityOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, PartMapProblem>,
    ) -> Result<leaven::extend::StepStatus, OptimizerError> {
        if self.done {
            return Ok(leaven::extend::StepStatus::Done);
        }

        let baseline = self
            .evaluate_casewise(ctx, self.seed, EvaluationPurpose::SeedBaseline)
            .await?;
        let baseline_evidence = ctx
            .assessment_evidence(baseline.assessment)
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let baseline_events = self
            .gepa
            .population_mut()
            .observe_partitioned_casewise_scalar(
                &PartitionId::from(TRAIN),
                self.seed,
                baseline.assessment,
                &baseline_evidence,
            );
        ctx.emit(RunEvent::PopulationUpdated {
            population_id: self.gepa.population().id(),
            events: baseline_events,
        });

        let parent = self
            .gepa
            .select_candidate(ctx.graph())
            .expect("baseline puts seed in frontier");
        let artifact = ctx
            .graph()
            .artifact(parent)
            .expect("selected artifact exists")
            .clone();
        let part = self
            .gepa
            .select_part(&artifact)
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let edit = self
            .proposer
            .propose_edit(&artifact, self.gepa.surface(), &part)
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let change = self
            .gepa
            .change_part(&artifact, part, edit)
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let proposal = ctx
            .record_proposal_batch(
                StageId::custom("p3/reflective-mutation"),
                ProposalBatch {
                    proposals: vec![
                        Proposal::mutate(parent, change)
                            .informed_by([leaven::extend::InfoRef::Candidate(parent)])
                            .build(),
                    ],
                    semantics: ProposalBatchSemantics::Alternatives,
                    metadata: MetadataBag::new(),
                },
                Cost::metric_calls(1),
            )
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let applied = ctx
            .apply_batch(proposal.batch_id)
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let candidate = applied
            .successful_candidates()
            .next()
            .expect("surface-lowered change should apply");

        let screened = self
            .evaluate_casewise(ctx, candidate, EvaluationPurpose::Search)
            .await?;
        let candidate_evidence = ctx
            .assessment_evidence(screened.assessment)
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let decision = self
            .gepa
            .decide(baseline.average_score, screened.average_score);
        assert_eq!(decision, GateDecision::Accept);
        let events = self
            .gepa
            .population_mut()
            .observe_partitioned_casewise_scalar(
                &PartitionId::from(TRAIN),
                candidate,
                screened.assessment,
                &candidate_evidence,
            );
        ctx.emit(RunEvent::PopulationUpdated {
            population_id: self.gepa.population().id(),
            events,
        });
        self.best = self.gepa.population().best();
        self.done = true;
        Ok(leaven::extend::StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven::extend::RunGraphView<'_, PartMapProblem>,
    ) -> Option<CandidateId> {
        self.best
    }
}

impl GepaParityOptimizer {
    async fn evaluate_casewise(
        &self,
        ctx: &mut RunContext<'_, PartMapProblem>,
        candidate: CandidateId,
        purpose: EvaluationPurpose,
    ) -> Result<CasewiseReport, OptimizerError> {
        let report = ctx
            .evaluate(
                EvaluatorId::PRIMARY,
                EvaluationRequest::Independent {
                    candidates: vec![candidate],
                    set: EvaluationSet::Partition(PartitionId::from(TRAIN)),
                    granularity: AssessmentGranularity::PerCase,
                    purpose,
                },
            )
            .await
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let assessment = report.assessment_ids[0];
        let evidence = ctx
            .assessment_evidence(assessment)
            .map_err(|err| OptimizerError::Message(err.to_string()))?;
        let average_score = average_score(&evidence);
        Ok(CasewiseReport {
            assessment,
            average_score,
        })
    }
}

struct CasewiseReport {
    assessment: leaven_kernel::AssessmentId,
    average_score: f64,
}

struct PartMapEvaluator;

impl Evaluator<PartMapProblem> for PartMapEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([4; 32])
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'_, PartMapProblem>,
    ) -> Result<Metered<Vec<Assessment<PartMapProblem>>>, EvaluationError> {
        assert_eq!(request.granularity, AssessmentGranularity::PerCase);
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        let mut assessments = Vec::new();
        for candidate in candidates {
            let artifact = ctx.graph().artifact(candidate).expect("candidate exists");
            let answer = artifact.0.get("answer").expect("answer part exists");
            let score = if answer == "improved answer" {
                1.0
            } else {
                0.2
            };
            let evidence = CasewiseEvidence::new(
                request
                    .set
                    .case_ids
                    .iter()
                    .map(|case| {
                        CaseOutcome::new(*case, ScalarEvidence::new(score).expect("finite score"))
                    })
                    .collect(),
            );
            assessments.push(Assessment::Independent {
                candidate,
                target: AssessmentTarget::EvaluationSet(leaven_kernel::EvaluationSetId::new()),
                evidence,
                cost: Cost::metric_calls(1),
                metadata: MetadataBag::new(),
            });
        }
        Ok(Metered::new(assessments, Cost::metric_calls(1)))
    }
}

fn average_score(evidence: &CasewiseEvidence<ScalarEvidence>) -> f64 {
    let total: f64 = evidence
        .outcomes()
        .iter()
        .map(|outcome| outcome.evidence().score())
        .sum();
    let count = u32::try_from(evidence.outcomes().len()).expect("case count fits into u32");
    total / f64::from(count)
}

fn content_id(bytes: &[u8]) -> ContentId {
    ContentId::hash_bytes(bytes)
}
