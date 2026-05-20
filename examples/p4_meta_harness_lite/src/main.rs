use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use futures::executor::block_on;
use leaven::extend::{
    Arity, CachePolicy, CausalInputs, EvaluationRequest, EvaluationSet, Evaluator, InfoRef,
    MaterializationReport, MaterializeContext, MaterializeError, Materializer, Optimizer, Proposal,
    ProposalBatch, ProposalBatchSemantics, ProposalContext, ProposalEffect, Proposer, RunEvent,
    RunGraphView, StepStatus, TrustPolicy,
};
use leaven::plumbing::{ContentId, MetadataBag};
use leaven::prelude::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget, Budget, Cost,
    OptimizationProblem,
};
use leaven_core::{
    EvaluationPurpose, ExternalRef, PartitionId, ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_engine::{CaseSet, EvaluationContext, EvaluationError, OptimizerError, ProposalError};
use leaven_evidence::{
    AgentTrajectoryEvidence, AgentTrajectoryEvidenceInput, AgentTrajectoryOutcome, CommandEvidence,
    CommandRecord, OutputRecord, ScalarEvidence,
};
use leaven_kernel::{
    AgentSessionId, AssessmentId, BlobRef, CandidateId, EvaluatorId, EvidenceRef, Fingerprint,
    FingerprintBuilder, Metered, ProposerId,
};
use leaven_population::KeepBest;
use leaven_store_inline::InlineEvidenceStore;
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory, WorkspacePath, WorkspaceView};
use leaven_workspace_local::LocalWorkspaceFactory;

const SEARCH: &str = "SEARCH";
const TEST: &str = "TEST";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    block_on(async {
        let counters = CleanupCounters::default();
        let evidence_store = InlineEvidenceStore::<HarnessEvidence>::new("p4-inline");
        let case_set = CaseSet::new(vec!["search-case", "held-out-case"])
            .with_partition(
                PartitionId::from(SEARCH),
                vec![leaven_kernel::CaseId::new(0)],
            )
            .with_partition(PartitionId::from(TEST), vec![leaven_kernel::CaseId::new(1)]);
        let evaluator = HarnessEvaluator {
            factory: LocalWorkspaceFactory::default(),
            cleanup_count: counters.evaluator.clone(),
        };
        let mut engine = leaven::engine::optimize::<MetaHarnessProblem>()
            .budget(Budget::metric_calls(50))
            .trust_policy(TrustPolicy::default().hide_from_proposers([PartitionId::from(TEST)]))
            .evaluator(evaluator)
            .build();
        let seed = engine.insert_seed(
            HarnessArtifact {
                source: "def score(x):\n    return 0\n".to_owned(),
                notes: "baseline harness".to_owned(),
            },
            0,
        )?;
        let mut optimizer = MetaHarnessOptimizer {
            seed,
            proposer: AgenticHarnessProposer {
                factory: LocalWorkspaceFactory::default(),
                history_materializer: HistoryMaterializer {
                    artifact_materializer: HarnessArtifactMaterializer,
                    evidence_materializer: HarnessEvidencePointerMaterializer,
                },
                cleanup_count: counters.proposer.clone(),
            },
            population: KeepBest::new(),
            best: None,
            done: false,
            created: None,
            hidden_assessment: None,
        };

        let result = engine
            .run(&mut optimizer, &case_set, &evidence_store)
            .await?;
        let best = result.best.expect("P4 optimizer produces a best candidate");
        let graph = engine.view();
        let artifact = graph.artifact(best).expect("best artifact exists");
        assert!(artifact.source.contains("return 1"));
        assert!(artifact.notes.contains("fresh harness"));
        assert_eq!(best, optimizer.created.expect("created candidate recorded"));
        assert_eq!(counters.proposer(), 1);
        assert_eq!(counters.evaluator(), 3);
        assert!(optimizer.hidden_assessment.is_some());
        assert_eq!(graph.assessment_count(), 3);
        assert_eq!(graph.proposal_count(), 1);

        println!(
            "p4 meta-harness lite: candidate={best} materialized=true cleanup=true evidence_refs=true hidden_test_filtered=true"
        );
        Ok(())
    })
}

#[derive(Clone, Default)]
struct CleanupCounters {
    proposer: Arc<AtomicUsize>,
    evaluator: Arc<AtomicUsize>,
}

impl CleanupCounters {
    fn proposer(&self) -> usize {
        self.proposer.load(Ordering::SeqCst)
    }

    fn evaluator(&self) -> usize {
        self.evaluator.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HarnessArtifact {
    source: String,
    notes: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HarnessChange;

#[derive(Debug)]
struct HarnessError;

impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("harness artifact does not support in-place changes in this example")
    }
}

impl std::error::Error for HarnessError {}

impl Artifact for HarnessArtifact {
    type Change = HarnessChange;
    type ApplyError = HarnessError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(content_id(
            format!("{}\n{}", self.source, self.notes).as_bytes(),
        ))
    }

    fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Err(HarnessError)
    }
}

struct MetaHarnessProblem;

impl OptimizationProblem for MetaHarnessProblem {
    type Artifact = HarnessArtifact;
    type Case = &'static str;
    type Evidence = HarnessEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug, PartialEq)]
enum HarnessEvidence {
    RepoTask {
        score: ScalarEvidence,
        command: CommandEvidence,
        trajectory: AgentTrajectoryEvidence,
    },
}

impl leaven::prelude::Evidence for HarnessEvidence {}

impl HarnessEvidence {
    const fn score(&self) -> ScalarEvidence {
        match self {
            Self::RepoTask { score, .. } => *score,
        }
    }

    fn command(&self) -> &CommandEvidence {
        match self {
            Self::RepoTask { command, .. } => command,
        }
    }

    fn trajectory(&self) -> &AgentTrajectoryEvidence {
        match self {
            Self::RepoTask { trajectory, .. } => trajectory,
        }
    }
}

struct MetaHarnessOptimizer {
    seed: CandidateId,
    proposer: AgenticHarnessProposer,
    population: KeepBest,
    best: Option<CandidateId>,
    done: bool,
    created: Option<CandidateId>,
    hidden_assessment: Option<AssessmentId>,
}

impl Optimizer<MetaHarnessProblem> for MetaHarnessOptimizer {
    async fn step(
        &mut self,
        ctx: &mut leaven::extend::RunContext<'_, MetaHarnessProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        if self.done {
            return Ok(StepStatus::Done);
        }

        let search_assessment = evaluate_one(ctx, self.seed, SEARCH, EvaluationPurpose::Search)
            .await
            .map_err(|error| OptimizerError::with_source("search seed evaluation failed", error))?;
        let search_score = ctx
            .assessment_evidence(search_assessment)
            .map_err(|error| OptimizerError::with_source("seed evidence lookup failed", error))?
            .score();
        emit_population_events(
            ctx,
            self.population
                .observe(self.seed, search_assessment, search_score),
        );

        let hidden_assessment = evaluate_one(ctx, self.seed, TEST, EvaluationPurpose::FinalTest)
            .await
            .map_err(|error| OptimizerError::with_source("hidden seed evaluation failed", error))?;
        self.hidden_assessment = Some(hidden_assessment);

        let proposal_report = ctx
            .propose(
                &self.proposer,
                HistoryProposalRequest {
                    visible_candidates: vec![self.seed],
                    assessment_refs: vec![search_assessment, hidden_assessment],
                },
            )
            .await
            .map_err(|error| OptimizerError::with_source("agentic proposal failed", error))?;
        let apply_report = ctx
            .apply_batch(proposal_report.batch_id)
            .map_err(|error| OptimizerError::with_source("fresh proposal apply failed", error))?;
        let created = apply_report.successful_candidates().next().ok_or_else(|| {
            OptimizerError::Message("fresh harness proposal did not apply".to_owned())
        })?;

        assert!(ctx.graph().parents(created).is_empty());
        assert_eq!(ctx.graph().children(self.seed), []);
        assert!(
            ctx.graph()
                .informed_by(created)
                .contains(&InfoRef::Candidate(self.seed))
        );
        assert!(
            ctx.graph()
                .informed_by(created)
                .contains(&InfoRef::Assessment(search_assessment))
        );
        assert!(
            !ctx.graph()
                .informed_by(created)
                .contains(&InfoRef::Assessment(hidden_assessment))
        );
        let proposal = ctx
            .graph()
            .proposal_that_created(created)
            .expect("created candidate has proposal");
        assert!(matches!(proposal.effect(), ProposalEffect::Create { .. }));
        assert!(matches!(proposal.provenance().causal(), CausalInputs::None));

        let created_assessment = evaluate_one(ctx, created, SEARCH, EvaluationPurpose::Search)
            .await
            .map_err(|error| OptimizerError::with_source("created evaluation failed", error))?;
        let created_evidence = ctx
            .assessment_evidence(created_assessment)
            .map_err(|error| {
                OptimizerError::with_source("created evidence lookup failed", error)
            })?;
        assert!(!created_evidence.command().records().is_empty());
        assert!(matches!(
            created_evidence.trajectory().transcript(),
            OutputRecord::BlobRef(_)
        ));
        emit_population_events(
            ctx,
            self.population
                .observe(created, created_assessment, created_evidence.score()),
        );

        self.best = self.population.best();
        self.created = Some(created);
        self.done = true;
        Ok(StepStatus::Done)
    }

    fn best_candidate(&self, _graph: RunGraphView<'_, MetaHarnessProblem>) -> Option<CandidateId> {
        self.best
    }
}

async fn evaluate_one(
    ctx: &mut leaven::extend::RunContext<'_, MetaHarnessProblem>,
    candidate: CandidateId,
    partition: &'static str,
    purpose: EvaluationPurpose,
) -> Result<AssessmentId, leaven_engine::RunContextError> {
    Ok(ctx
        .evaluate(
            EvaluatorId::PRIMARY,
            EvaluationRequest::Independent {
                candidates: vec![candidate],
                set: EvaluationSet::Partition(PartitionId::from(partition)),
                granularity: AssessmentGranularity::Aggregate,
                purpose,
            },
        )
        .await?
        .assessment_ids[0])
}

fn emit_population_events(
    ctx: &mut leaven::extend::RunContext<'_, MetaHarnessProblem>,
    events: Vec<leaven_engine::PopulationEvent>,
) {
    if !events.is_empty() {
        ctx.emit(RunEvent::PopulationUpdated {
            population_id: match &events[0] {
                leaven_engine::PopulationEvent::Inserted { population, .. }
                | leaven_engine::PopulationEvent::Replaced { population, .. }
                | leaven_engine::PopulationEvent::Removed { population, .. }
                | leaven_engine::PopulationEvent::Ignored { population, .. }
                | leaven_engine::PopulationEvent::Reweighted { population, .. } => *population,
            },
            events,
        });
    }
}

struct AgenticHarnessProposer {
    factory: LocalWorkspaceFactory,
    history_materializer: HistoryMaterializer,
    cleanup_count: Arc<AtomicUsize>,
}

impl Proposer<MetaHarnessProblem> for AgenticHarnessProposer {
    type Request = HistoryProposalRequest;

    fn id(&self) -> ProposerId {
        ProposerId::from("p4/meta-harness-agent")
    }

    fn arity(&self) -> Arity {
        Arity::None
    }

    async fn propose(
        &self,
        request: Self::Request,
        ctx: ProposalContext<'_, MetaHarnessProblem>,
    ) -> Result<Metered<ProposalBatch<MetaHarnessProblem>>, ProposalError> {
        let mut workspace = self
            .factory
            .allocate(WorkspaceConfig::default())
            .await
            .map_err(|error| ProposalError::with_source("workspace allocation failed", error))?;
        let snapshot = HistorySnapshot {
            candidates: request.visible_candidates,
            assessments: request.assessment_refs,
        };
        let materialized = {
            let mut view = workspace.view();
            self.history_materializer
                .materialize_into(&snapshot, &mut view, ctx.materialize_context())
                .await
                .map_err(|error| {
                    ProposalError::with_source("history materialization failed", error)
                })?
        };
        let (artifact, history_refs) = {
            let mut view = workspace.view();
            fake_agent_author_harness(&mut view)
                .map_err(|error| ProposalError::with_source("agent workspace pass failed", error))?
        };
        workspace
            .cleanup()
            .await
            .map_err(|error| ProposalError::with_source("workspace cleanup failed", error))?;
        self.cleanup_count.fetch_add(1, Ordering::SeqCst);

        let proposal = Proposal::create(artifact).informed_by(history_refs).build();
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![proposal],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            materialized.cost.combine(&Cost::llm_calls(1)),
        ))
    }
}

struct HistoryProposalRequest {
    visible_candidates: Vec<CandidateId>,
    assessment_refs: Vec<AssessmentId>,
}

struct HistorySnapshot {
    candidates: Vec<CandidateId>,
    assessments: Vec<AssessmentId>,
}

struct HistoryMaterializer {
    artifact_materializer: HarnessArtifactMaterializer,
    evidence_materializer: HarnessEvidencePointerMaterializer,
}

impl Materializer<MetaHarnessProblem, HistorySnapshot> for HistoryMaterializer {
    async fn materialize_into(
        &self,
        value: &HistorySnapshot,
        workspace: &mut WorkspaceView<'_>,
        ctx: MaterializeContext<'_, MetaHarnessProblem>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        let mut report = MaterializationReport::default();
        let candidate_manifest = value
            .candidates
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        write_metered(
            workspace,
            &WorkspacePath::new("history/candidates.txt")?,
            candidate_manifest.as_bytes(),
            &mut report,
        )?;

        for (index, candidate) in value.candidates.iter().copied().enumerate() {
            if let Some(artifact) = ctx.graph().artifact(candidate) {
                let path = if index == 0 {
                    WorkspacePath::new("artifact/seed")?
                } else {
                    WorkspacePath::new(format!("artifact/candidate-{index}"))?
                };
                let mut artifact_view = workspace.subdir(path)?;
                merge_report(
                    &mut report,
                    self.artifact_materializer
                        .materialize_into(artifact, &mut artifact_view, ctx.clone())
                        .await?,
                );
            }
        }

        let mut visible_assessments = Vec::new();
        let mut filtered = 0_usize;
        for assessment in &value.assessments {
            let Some(view) = ctx.graph().assessment(*assessment) else {
                filtered += 1;
                continue;
            };
            visible_assessments.push(*assessment);
            let mut evidence_view =
                workspace.subdir(WorkspacePath::new(format!("evidence/{assessment}"))?)?;
            let pointer = EvidencePointer {
                assessment: *assessment,
                reference: view.evidence_ref().clone(),
            };
            merge_report(
                &mut report,
                self.evidence_materializer
                    .materialize_into(&pointer, &mut evidence_view, ctx.clone())
                    .await?,
            );
        }

        let visible_manifest = visible_assessments
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        write_metered(
            workspace,
            &WorkspacePath::new("history/visible_assessments.txt")?,
            visible_manifest.as_bytes(),
            &mut report,
        )?;
        write_metered(
            workspace,
            &WorkspacePath::new("history/filtered_assessment_count.txt")?,
            filtered.to_string().as_bytes(),
            &mut report,
        )?;

        Ok(Metered::new(report, Cost::metric_calls(1)))
    }
}

struct HarnessArtifactMaterializer;

impl Materializer<MetaHarnessProblem, HarnessArtifact> for HarnessArtifactMaterializer {
    async fn materialize_into(
        &self,
        value: &HarnessArtifact,
        workspace: &mut WorkspaceView<'_>,
        _ctx: MaterializeContext<'_, MetaHarnessProblem>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        let mut report = MaterializationReport::default();
        write_metered(
            workspace,
            &WorkspacePath::new("harness.py")?,
            value.source.as_bytes(),
            &mut report,
        )?;
        write_metered(
            workspace,
            &WorkspacePath::new("notes.md")?,
            value.notes.as_bytes(),
            &mut report,
        )?;
        Ok(Metered::new(report, Cost::metric_calls(1)))
    }
}

struct HarnessEvidencePointerMaterializer;

impl Materializer<MetaHarnessProblem, EvidencePointer> for HarnessEvidencePointerMaterializer {
    async fn materialize_into(
        &self,
        value: &EvidencePointer,
        workspace: &mut WorkspaceView<'_>,
        _ctx: MaterializeContext<'_, MetaHarnessProblem>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        let body = format!(
            "assessment={}\nevidence_ref={}:{}\n",
            value.assessment, value.reference.store, value.reference.key
        );
        let mut report = MaterializationReport::default();
        write_metered(
            workspace,
            &WorkspacePath::new("evidence_ref.txt")?,
            body.as_bytes(),
            &mut report,
        )?;
        Ok(Metered::new(report, Cost::zero()))
    }
}

struct EvidencePointer {
    assessment: AssessmentId,
    reference: EvidenceRef,
}

fn fake_agent_author_harness(
    workspace: &mut WorkspaceView<'_>,
) -> Result<(HarnessArtifact, Vec<InfoRef>), MaterializeError> {
    let seed_source =
        String::from_utf8(workspace.read_file(&WorkspacePath::new("artifact/seed/harness.py")?)?)
            .map_err(|error| MaterializeError::Message(error.to_string()))?;
    let candidates =
        String::from_utf8(workspace.read_file(&WorkspacePath::new("history/candidates.txt")?)?)
            .map_err(|error| MaterializeError::Message(error.to_string()))?;
    let visible = String::from_utf8(
        workspace.read_file(&WorkspacePath::new("history/visible_assessments.txt")?)?,
    )
    .map_err(|error| MaterializeError::Message(error.to_string()))?;
    let filtered = String::from_utf8(workspace.read_file(&WorkspacePath::new(
        "history/filtered_assessment_count.txt",
    )?)?)
    .map_err(|error| MaterializeError::Message(error.to_string()))?;
    assert_eq!(filtered.trim(), "1");
    assert_eq!(visible.lines().count(), 1);

    let authored = seed_source.replace("return 0", "return 1");
    let notes = "fresh harness authored from materialized history\n".to_owned();
    workspace.write_file(
        &WorkspacePath::new("output/harness_0.py")?,
        authored.as_bytes(),
    )?;
    workspace.write_file(&WorkspacePath::new("output/notes_0.md")?, notes.as_bytes())?;

    let artifact = HarnessArtifact {
        source: String::from_utf8(
            workspace.read_file(&WorkspacePath::new("output/harness_0.py")?)?,
        )
        .map_err(|error| MaterializeError::Message(error.to_string()))?,
        notes: String::from_utf8(workspace.read_file(&WorkspacePath::new("output/notes_0.md")?)?)
            .map_err(|error| MaterializeError::Message(error.to_string()))?,
    };
    let mut refs = candidates
        .lines()
        .map(|line| {
            line.parse::<uuid::Uuid>()
                .map(|uuid| InfoRef::Candidate(CandidateId::from_uuid(uuid)))
                .map_err(|error| MaterializeError::Message(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    refs.extend(
        visible
            .lines()
            .map(|line| {
                line.parse::<uuid::Uuid>()
                    .map(|uuid| InfoRef::Assessment(AssessmentId::from_uuid(uuid)))
                    .map_err(|error| MaterializeError::Message(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?,
    );
    refs.push(InfoRef::External(ExternalRef {
        kind: "workspace".to_owned(),
        id: "meta-harness-lite".to_owned(),
    }));
    Ok((artifact, refs))
}

struct HarnessEvaluator {
    factory: LocalWorkspaceFactory,
    cleanup_count: Arc<AtomicUsize>,
}

impl Evaluator<MetaHarnessProblem> for HarnessEvaluator {
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
        ctx: EvaluationContext<'_, MetaHarnessProblem>,
    ) -> Result<Metered<Vec<Assessment<MetaHarnessProblem>>>, EvaluationError> {
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "P4 evaluator only handles independent requests".to_owned(),
            ));
        };
        let mut assessments = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let artifact = ctx
                .graph()
                .artifact(candidate)
                .ok_or_else(|| EvaluationError::Message("candidate not visible".to_owned()))?
                .clone();
            let mut workspace = self
                .factory
                .allocate(WorkspaceConfig::default())
                .await
                .map_err(|error| {
                    EvaluationError::with_source("workspace allocation failed", error)
                })?;
            let score = {
                let mut view = workspace.view();
                let path = WorkspacePath::new("repo/harness.py").map_err(|error| {
                    EvaluationError::with_source("workspace path failed", error)
                })?;
                view.write_file(&path, artifact.source.as_bytes())
                    .map_err(|error| {
                        EvaluationError::with_source("workspace write failed", error)
                    })?;
                if artifact.source.contains("return 1") {
                    1.0
                } else {
                    0.2
                }
            };
            workspace
                .cleanup()
                .await
                .map_err(|error| EvaluationError::with_source("workspace cleanup failed", error))?;
            self.cleanup_count.fetch_add(1, Ordering::SeqCst);

            let scalar = ScalarEvidence::new(score).map_err(|error| {
                EvaluationError::with_source("score construction failed", error)
            })?;
            let command = CommandEvidence::new(vec![CommandRecord::new(
                "python repo/harness.py",
                Some(0),
                OutputRecord::inline(format!("score={score}")),
                OutputRecord::inline(""),
                Duration::from_millis(1),
            )]);
            let mut fingerprint_builder = FingerprintBuilder::new();
            fingerprint_builder.update("p4-meta-harness-lite");
            let model_config_fingerprint = fingerprint_builder.finish();
            let trajectory = AgentTrajectoryEvidence::new(AgentTrajectoryEvidenceInput {
                session_id: AgentSessionId::new(),
                case_id: None,
                task_id: candidate.to_string(),
                outcome: if score >= 1.0 {
                    AgentTrajectoryOutcome::Success
                } else {
                    AgentTrajectoryOutcome::Failure {
                        reason: "local harness score below acceptance threshold".to_owned(),
                    }
                },
                model_id: "p4-local-harness".to_owned(),
                model_config_fingerprint,
                transcript: OutputRecord::blob(BlobRef {
                    store: "p4-transcripts".to_owned(),
                    key: format!("candidate-{candidate}"),
                }),
                commands: command.clone(),
            });
            assessments.push(Assessment::Independent {
                candidate,
                target: AssessmentTarget::EvaluationSet(leaven_kernel::EvaluationSetId::new()),
                evidence: HarnessEvidence::RepoTask {
                    score: scalar,
                    command,
                    trajectory,
                },
                cost: Cost::metric_calls(1),
                metadata: MetadataBag::new(),
            });
        }
        Ok(Metered::new(assessments, Cost::metric_calls(1)))
    }
}

fn write_metered(
    workspace: &mut WorkspaceView<'_>,
    path: &WorkspacePath,
    bytes: &[u8],
    report: &mut MaterializationReport,
) -> Result<(), MaterializeError> {
    workspace.write_file(path, bytes)?;
    report.files_written += 1;
    report.bytes_written += u64::try_from(bytes.len()).expect("fixture byte count fits u64");
    Ok(())
}

fn merge_report(report: &mut MaterializationReport, metered: Metered<MaterializationReport>) {
    report.files_written += metered.value.files_written;
    report.bytes_written += metered.value.bytes_written;
    report.truncations.extend(metered.value.truncations);
}

fn content_id(bytes: &[u8]) -> ContentId {
    let mut id = [0; ContentId::BYTES];
    let len = bytes.len().min(ContentId::BYTES);
    id[..len].copy_from_slice(&bytes[..len]);
    ContentId::from_bytes(id)
}
