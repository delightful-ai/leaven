use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

use futures::executor::block_on;
use leaven::extend::{
    CachePolicy, EvaluationRequest, Evaluator, InfoRef, Optimizer, Proposal, ProposalBatch,
    ProposalBatchSemantics, ProposalEffect, RunEvent, TrustPolicy,
};
use leaven::gepa::{
    Gepa, GepaReflectiveDataset, GepaReflector, ReflectRequest, ReflectiveValue, SurfaceProposer,
    test_support::FixedSurfaceEdit,
};
use leaven::plumbing::ContentId;
use leaven::prelude::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget, Budget,
    CandidateId, Cost, RunOutput, Score, ScoreContext, optimize,
};
use leaven::stdlib::{
    evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence},
    populations::ParetoFrontier,
};
use leaven_core::{
    EvaluationPurpose, EvaluationSet, OptimizationProblem, PartitionId, ResolvedEvaluationRequest,
    ResolvedRequestKind,
};
use leaven_engine::{CaseSet, EvaluationContext, EvaluationError, OptimizerError, RunContext};
use leaven_eval::NoTarget;
use leaven_kernel::{
    AssessmentId, CaseId, EvaluationRequestId, EvaluatorId, Fingerprint, MetadataBag, Metered,
    StageId,
};
use leaven_store_inline::InlineEvidenceStore;
use leaven_surface::{EditSurface, Part, PartAddress, SurfaceError, SurfaceFingerprint};

const TRAIN: &str = "TRAIN";
const VALIDATION: &str = "VALIDATION";

#[test]
fn engine_runs_gepa_parity_end_to_end() {
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
        let seed = engine
            .insert_seed(
                PartMapArtifact(BTreeMap::from([
                    ("answer".to_owned(), "draft answer".to_owned()),
                    ("search".to_owned(), "stable search query".to_owned()),
                ])),
                0,
            )
            .unwrap();
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
            candidate: None,
            requests: Vec::new(),
            assessments: Vec::new(),
            evidence_cases: Vec::new(),
            done: false,
        };

        let result = engine.run(&mut optimizer, &cases, &store).await.unwrap();

        let best = result.best.expect("gepa parity should choose a winner");
        let best_artifact = engine.view().artifact(best).expect("best exists");
        assert_eq!(optimizer.best, Some(best));
        assert_eq!(optimizer.candidate, Some(best));
        assert_eq!(best_artifact.0.get("answer").unwrap(), "improved answer");
        assert_eq!(
            best_artifact.0.get("search").unwrap(),
            "stable search query"
        );
        assert_eq!(engine.view().evaluation_request_count(), 2);
        assert_eq!(engine.view().assessment_count(), 2);
        assert_eq!(optimizer.requests.len(), 2);
        assert_eq!(optimizer.assessments.len(), 2);
        assert_eq!(
            optimizer.evidence_cases,
            vec![
                vec![CaseId::new(0), CaseId::new(1)],
                vec![CaseId::new(0), CaseId::new(1)]
            ]
        );
        for request in &optimizer.requests {
            let request = engine.view().evaluation_request(*request).unwrap();
            assert_train_per_case_request(&request);
        }
        let proposal = engine
            .view()
            .proposal_that_created(best)
            .expect("best should be proposal-created");
        assert!(matches!(
            proposal.effect(),
            ProposalEffect::Change { target, .. } if *target == seed
        ));
        assert_event_subsequence(
            &engine.view().events().collect::<Vec<_>>(),
            &[
                EventKind::OptimizationStarted,
                EventKind::IterationStarted,
                EventKind::EvaluationRequested,
                EventKind::EvaluationCompleted,
                EventKind::PopulationUpdated,
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

#[test]
#[allow(clippy::too_many_lines)]
fn public_run_gepa_path_reflects_rendered_typed_output() {
    #[derive(Clone, Debug)]
    struct TypedAnswer {
        answer: String,
        private_reasoning: String,
    }

    #[derive(Clone, Debug)]
    struct CapturingReflector {
        outputs: Arc<Mutex<Vec<Option<String>>>>,
    }

    impl GepaReflector<leaven::run::RunProblem<PartMapArtifact, String, NoTarget>, PartMapSurface>
        for CapturingReflector
    {
        async fn reflect_candidate(
            &mut self,
            ctx: &mut RunContext<'_, leaven::run::RunProblem<PartMapArtifact, String, NoTarget>>,
            surface: &PartMapSurface,
            request: ReflectRequest<String>,
        ) -> Result<Option<CandidateId>, OptimizerError> {
            self.outputs.lock().expect("captured outputs lock").extend(
                request
                    .examples
                    .iter()
                    .flat_map(|case| case.runs.iter())
                    .map(|run| match run.produced.as_ref() {
                        Some(ReflectiveValue::Text(text)) => Some(text.clone()),
                        Some(_) | None => None,
                    }),
            );
            FixedSurfaceEdit::new(PartMapEdit::Replace("improved answer".to_owned()))
                .reflect_candidate(ctx, surface, request)
                .await
        }
    }

    let captured_outputs = Arc::new(Mutex::new(Vec::new()));
    let result = block_on(
        optimize(PartMapArtifact(BTreeMap::from([
            ("answer".to_owned(), "draft answer".to_owned()),
            ("search".to_owned(), "stable search query".to_owned()),
        ])))
        .train_inputs(vec!["TRAIN example".to_owned()])
        .validation_inputs(vec!["VALIDATION example".to_owned()])
        .runner(
            |artifact: PartMapArtifact, case: leaven::run::RunCase<String>| async move {
                let answer = artifact
                    .0
                    .get("answer")
                    .expect("answer part exists")
                    .clone();
                Ok(RunOutput::typed(TypedAnswer {
                    answer,
                    private_reasoning: format!("typed reasoning for {}", case.input()),
                }))
            },
        )
        .score(
            |ctx: ScoreContext<PartMapArtifact, String, NoTarget, TypedAnswer>| async move {
                assert!(ctx.output.output.private_reasoning.contains("example"));
                let value = if ctx.output.output.answer == "improved answer" {
                    0.9
                } else {
                    0.2
                };
                Ok(Score::new(value, "typed score").with_output(
                    ctx.report_text_output(format!(
                        "rendered answer: {}",
                        ctx.output.output.answer
                    )),
                ))
            },
        )
        .using(
            Gepa::new(
                PartMapSurface,
                ParetoFrontier::by_case().build(),
                CapturingReflector {
                    outputs: Arc::clone(&captured_outputs),
                },
            )
            .reflective_dataset(GepaReflectiveDataset::with_case_input(
                |case: &leaven_eval::Case<String, NoTarget>| case.input.clone(),
            ))
            .max_iterations(1),
        )
        .budget(Budget::metric_calls(16))
        .ephemeral()
        .run(),
    )
    .unwrap();

    assert_eq!(
        result.best().and_then(|artifact| artifact.0.get("answer")),
        Some(&"improved answer".to_owned())
    );
    let captured = captured_outputs.lock().expect("captured outputs lock");
    assert!(!captured.is_empty());
    assert!(
        captured
            .iter()
            .all(|output| output.as_deref() == Some("rendered answer: draft answer"))
    );

    let report_outputs = result
        .report()
        .splits_reported
        .iter()
        .flat_map(|split| split.candidates.iter())
        .flat_map(|candidate| candidate.cases.iter())
        .map(|case| case.output.as_str())
        .collect::<Vec<_>>();
    assert!(
        report_outputs.contains(&"rendered answer: improved answer"),
        "expected public report to contain rendered best output, got {report_outputs:?}"
    );
    assert!(
        report_outputs
            .iter()
            .all(|output| !output.contains("TypedAnswer") && !output.contains("private_reasoning")),
        "public report must not expose typed/debug internals: {report_outputs:?}"
    );
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
struct PartMapArtifact(BTreeMap<String, String>);

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
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
    candidate: Option<CandidateId>,
    requests: Vec<EvaluationRequestId>,
    assessments: Vec<AssessmentId>,
    evidence_cases: Vec<Vec<CaseId>>,
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
        self.record_observation(baseline.request, baseline.assessment, &baseline_evidence);
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
                StageId::custom("p3/fixed-surface-edit"),
                ProposalBatch {
                    proposals: vec![
                        Proposal::mutate(parent, change)
                            .informed_by([InfoRef::Candidate(parent)])
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
        self.record_observation(screened.request, screened.assessment, &candidate_evidence);
        assert!(
            self.gepa
                .decide(baseline.average_score, screened.average_score)
                .is_accept()
        );
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
        self.candidate = Some(candidate);
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
        Ok(CasewiseReport {
            request: report.request_id,
            assessment,
            average_score: average_score(&evidence),
        })
    }

    fn record_observation(
        &mut self,
        request: EvaluationRequestId,
        assessment: AssessmentId,
        evidence: &CasewiseEvidence<ScalarEvidence>,
    ) {
        self.requests.push(request);
        self.assessments.push(assessment);
        self.evidence_cases
            .push(evidence.outcomes().iter().map(CaseOutcome::case).collect());
    }
}

struct CasewiseReport {
    request: EvaluationRequestId,
    assessment: AssessmentId,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EventKind {
    OptimizationStarted,
    IterationStarted,
    EvaluationRequested,
    EvaluationCompleted,
    PopulationUpdated,
    ProposalBatchProduced,
    ProposalRecorded,
    ApplySucceeded,
    OptimizationEnded,
}

impl EventKind {
    fn from_event(event: &RunEvent) -> Option<Self> {
        match event {
            RunEvent::OptimizationStarted { .. } => Some(Self::OptimizationStarted),
            RunEvent::IterationStarted { .. } => Some(Self::IterationStarted),
            RunEvent::EvaluationRequested { .. } => Some(Self::EvaluationRequested),
            RunEvent::EvaluationCompleted { .. } => Some(Self::EvaluationCompleted),
            RunEvent::PopulationUpdated { .. } => Some(Self::PopulationUpdated),
            RunEvent::ProposalBatchProduced { .. } => Some(Self::ProposalBatchProduced),
            RunEvent::ProposalRecorded { .. } => Some(Self::ProposalRecorded),
            RunEvent::ApplySucceeded { .. } => Some(Self::ApplySucceeded),
            RunEvent::OptimizationEnded { .. } => Some(Self::OptimizationEnded),
            RunEvent::ApplyFailed { .. }
            | RunEvent::BudgetCharged { .. }
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

fn assert_train_per_case_request(request: &leaven_engine::EvaluationRequestView<'_>) {
    match request.request() {
        EvaluationRequest::Independent {
            set,
            granularity,
            purpose,
            ..
        } => {
            assert!(matches!(
                set,
                EvaluationSet::Partition(partition) if partition == &PartitionId::from(TRAIN)
            ));
            assert_eq!(*granularity, AssessmentGranularity::PerCase);
            assert!(matches!(
                purpose,
                EvaluationPurpose::SeedBaseline | EvaluationPurpose::Search
            ));
        }
        EvaluationRequest::Pairwise { .. } | EvaluationRequest::Listwise { .. } => {
            panic!("expected independent GEPA parity evaluation")
        }
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
    let mut id = [0; ContentId::BYTES];
    let len = bytes.len().min(ContentId::BYTES);
    id[..len].copy_from_slice(&bytes[..len]);
    ContentId::from_bytes(id)
}
