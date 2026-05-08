mod checkpoint;
mod codex;
mod data;
mod error;
mod evidence;
mod proposal;
mod roles;
mod scorer;

use std::path::{Path, PathBuf};
use std::time::Duration;

use leaven_agent::{
    AgentInstructions, AgentLimits, AgentRunContext, AgentRunRequest, AgentRuntime, OutputContract,
    TranscriptEvent, TranscriptRole,
};
use leaven_agentic::ProposalParser;
use leaven_agentic_skill::{
    SkillBankMaterializer, SkillBankProposalInput, SkillBankWorkspaceProposalParser,
    SkillWorkspaceLayout,
};
use leaven_artifact_skill::{SkillBank, SkillBankChange};
use leaven_core::{
    Assessment, AssessmentGranularity, AssessmentTarget, EvaluationPurpose, EvaluationRequest,
    EvaluationSet, ExternalRef, InfoRef, OptimizationProblem, ProposalBatchSemantics,
    ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_engine::{
    BudgetLedger, CachePolicy, EvaluationContext, EvaluationError, Evaluator, Materializer,
    RunContext, RunEvent, RunGraph,
};
use leaven_evidence::ScalarEvidence;
use leaven_kernel::{
    AgentSessionId, Budget, CandidateId, Cost, EvaluationSetId, EvaluatorId, EvidenceRef,
    Fingerprint, MetadataBag, Metered, ProposerId, RunId, StageId,
};
use leaven_population::KeepBest;
use leaven_store::EvidenceStore;
use leaven_store_file::{FileCheckpointStore, FileEvidenceStore};
use leaven_workspace::{Workspace, WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;
use serde::de::DeserializeOwned;

use crate::checkpoint::{Checkpoints, EvoSkillCheckpoint};
use crate::codex::{LiveCodexRuntime, live_codex_runtime, require_live_codex};
use crate::data::{EvoSkillCase, TRAIN, VALIDATION, case_by_id, case_set, load_cases};
use crate::error::{ExampleError, Result, msg};
use crate::evidence::{AgentRole, CaseExecution, EvoSkillEvidence};
use crate::proposal::{EvoSkillProposalAnnotations, SkillProposal};
use crate::roles::{
    brainstorming_meta_skill, executor_developer_instructions, proposer_developer_instructions,
    skill_builder_developer_instructions, skill_creator_meta_skill,
};
use crate::scorer::multi_tolerance_score;

struct EvoSkillProblem;

impl OptimizationProblem for EvoSkillProblem {
    type Artifact = SkillBank;
    type Case = EvoSkillCase;
    type Evidence = EvoSkillEvidence;
    type ProposalAnnotations = EvoSkillProposalAnnotations;
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse()?;
    if args.live_codex {
        require_live_codex()?;
    }

    let stores = RunStores::open(args.run_dir.unwrap_or_else(default_run_dir))?;
    if stores.print_complete_resume()? {
        return Ok(());
    }

    let resume = ResumeState::from_checkpoint(stores.checkpoints.latest()?)?;
    run_iteration(stores, resume).await
}

struct RunStores {
    run_root: PathBuf,
    evidence_store: FileEvidenceStore<EvoSkillEvidence>,
    checkpoints: Checkpoints,
}

impl RunStores {
    fn open(run_root: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&run_root)?;
        let evidence_store = FileEvidenceStore::<EvoSkillEvidence>::open(
            "p5-evoskill-evidence",
            run_root.join("evidence"),
        )?;
        let checkpoints =
            Checkpoints::open(FileCheckpointStore::open(run_root.join("checkpoints"))?);
        Ok(Self {
            run_root,
            evidence_store,
            checkpoints,
        })
    }

    fn print_complete_resume(&self) -> Result<bool> {
        let Some((
            _id,
            EvoSkillCheckpoint::IterationComplete {
                baseline_score,
                child_score,
                admitted,
                best_score,
                ..
            },
        )) = self.checkpoints.latest()?
        else {
            return Ok(false);
        };
        println!(
            "resume: iteration already complete baseline={baseline_score:.4} child={child_score:.4} admitted={admitted} best={best_score:.4}"
        );
        println!("run_root={}", self.run_root.display());
        Ok(true)
    }
}

async fn run_iteration(stores: RunStores, resume: ResumeState) -> Result<()> {
    let cases = resume.cases.unwrap_or_else(load_fixture_cases);
    let run_id = resume.run_id.unwrap_or_default();
    let seed_bank = resume.seed_bank.unwrap_or_default();
    let case_set = case_set(&cases);

    let mut graph = RunGraph::<EvoSkillProblem>::new(run_id);
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let mut population = KeepBest::new();
    let workspace_factory = LocalWorkspaceFactory::new(stores.run_root.join("workspaces"));
    let evaluator = new_executor_evaluator(&cases, &workspace_factory);

    let mut ctx = RunContext::<EvoSkillProblem>::new(&mut graph, &mut budget)
        .with_case_set(&case_set)
        .with_evidence_store(&stores.evidence_store);
    let seed = ctx.insert_seed(seed_bank.clone(), 0)?;

    let baseline = ensure_baseline(
        &mut ctx,
        &evaluator,
        &mut population,
        &stores.checkpoints,
        BaselineRequest {
            run_id,
            cases: &cases,
            seed_bank: &seed_bank,
            seed,
            resume_score: resume.baseline_score,
        },
    )
    .await?;
    let failures = ensure_failures(
        &mut ctx,
        &evaluator,
        &stores.checkpoints,
        FailureRequest {
            run_id,
            cases: &cases,
            seed_bank: &seed_bank,
            seed,
            baseline_score: baseline.score,
            resume_failures: resume.failures,
        },
    )
    .await?;

    if failures.is_empty() {
        return Err(msg(
            "EvoSkill live fixture produced no failures, so no mutation iteration could run",
        ));
    }

    let (skill_proposal, proposer_evidence) = ensure_proposal(
        &workspace_factory,
        &stores.evidence_store,
        &stores.checkpoints,
        ProposalRequest {
            run_id,
            cases: &cases,
            seed_bank: &seed_bank,
            baseline_score: baseline.score,
            failures: &failures,
            resume_proposal: resume.proposal,
            resume_evidence: resume.proposer_evidence,
        },
    )
    .await?;
    let (child, child_bank, change) = ensure_child(
        &workspace_factory,
        &stores.evidence_store,
        &stores.checkpoints,
        &mut ctx,
        ChildRequest {
            run_id,
            cases: &cases,
            seed_bank: &seed_bank,
            seed,
            baseline_score: baseline.score,
            failures: &failures,
            skill_proposal: &skill_proposal,
            proposer_evidence: &proposer_evidence,
            resume_child_bank: resume.child_bank,
            resume_change: resume.change,
        },
    )
    .await?;

    complete_iteration(
        &mut ctx,
        &evaluator,
        &mut population,
        &stores,
        CompletionRequest {
            run_id,
            seed_bank,
            baseline_score: baseline.score,
            child,
            child_bank,
            skill_proposal,
            change,
        },
    )
    .await
}

struct BaselineRequest<'a> {
    run_id: RunId,
    cases: &'a [EvoSkillCase],
    seed_bank: &'a SkillBank,
    seed: CandidateId,
    resume_score: Option<f64>,
}

async fn ensure_baseline(
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    evaluator: &EvoSkillEvaluator,
    population: &mut KeepBest,
    checkpoints: &Checkpoints,
    request: BaselineRequest<'_>,
) -> Result<ExistingBaseline> {
    if let Some(score) = request.resume_score {
        return Ok(ExistingBaseline { score });
    }
    let validation = evaluate_one(
        ctx,
        evaluator,
        request.seed,
        VALIDATION,
        EvaluationPurpose::SeedBaseline,
    )
    .await?;
    observe_keep_best(ctx, population, request.seed, &validation)?;
    checkpoints.save(&EvoSkillCheckpoint::BaselineComplete {
        run_id: request.run_id,
        cases: request.cases.to_vec(),
        seed_bank: request.seed_bank.clone(),
        baseline_score: validation.average_score,
    })?;
    Ok(ExistingBaseline {
        score: validation.average_score,
    })
}

struct FailureRequest<'a> {
    run_id: RunId,
    cases: &'a [EvoSkillCase],
    seed_bank: &'a SkillBank,
    seed: CandidateId,
    baseline_score: f64,
    resume_failures: Option<Vec<CaseExecution>>,
}

async fn ensure_failures(
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    evaluator: &EvoSkillEvaluator,
    checkpoints: &Checkpoints,
    request: FailureRequest<'_>,
) -> Result<Vec<CaseExecution>> {
    if let Some(failures) = request.resume_failures {
        return Ok(failures);
    }
    let feedback = evaluate_one(
        ctx,
        evaluator,
        request.seed,
        TRAIN,
        EvaluationPurpose::Feedback,
    )
    .await?;
    let failures = feedback
        .evidence
        .cases
        .into_iter()
        .filter(|case| !case.passed)
        .collect::<Vec<_>>();
    checkpoints.save(&EvoSkillCheckpoint::FailuresCollected {
        run_id: request.run_id,
        cases: request.cases.to_vec(),
        seed_bank: request.seed_bank.clone(),
        baseline_score: request.baseline_score,
        failures: failures.clone(),
    })?;
    Ok(failures)
}

struct ProposalRequest<'a> {
    run_id: RunId,
    cases: &'a [EvoSkillCase],
    seed_bank: &'a SkillBank,
    baseline_score: f64,
    failures: &'a [CaseExecution],
    resume_proposal: Option<SkillProposal>,
    resume_evidence: Option<EvidenceRef>,
}

async fn ensure_proposal(
    workspace_factory: &LocalWorkspaceFactory,
    evidence_store: &FileEvidenceStore<EvoSkillEvidence>,
    checkpoints: &Checkpoints,
    request: ProposalRequest<'_>,
) -> Result<(SkillProposal, EvidenceRef)> {
    if let (Some(proposal), Some(proposer_evidence)) =
        (request.resume_proposal, request.resume_evidence)
    {
        return Ok((proposal, proposer_evidence));
    }
    let proposal = run_skill_proposer(
        workspace_factory,
        evidence_store,
        request.seed_bank,
        request.failures,
    )
    .await?;
    checkpoints.save(&EvoSkillCheckpoint::ProposalComplete {
        run_id: request.run_id,
        cases: request.cases.to_vec(),
        seed_bank: request.seed_bank.clone(),
        baseline_score: request.baseline_score,
        failures: request.failures.to_vec(),
        proposal: proposal.value.clone(),
        proposer_evidence: proposal.evidence.clone(),
    })?;
    Ok((proposal.value, proposal.evidence))
}

struct ChildRequest<'a> {
    run_id: RunId,
    cases: &'a [EvoSkillCase],
    seed_bank: &'a SkillBank,
    seed: CandidateId,
    baseline_score: f64,
    failures: &'a [CaseExecution],
    skill_proposal: &'a SkillProposal,
    proposer_evidence: &'a EvidenceRef,
    resume_child_bank: Option<SkillBank>,
    resume_change: Option<SkillBankChange>,
}

async fn ensure_child(
    workspace_factory: &LocalWorkspaceFactory,
    evidence_store: &FileEvidenceStore<EvoSkillEvidence>,
    checkpoints: &Checkpoints,
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    request: ChildRequest<'_>,
) -> Result<(CandidateId, SkillBank, SkillBankChange)> {
    if let (Some(child_bank), Some(change)) = (request.resume_child_bank, request.resume_change) {
        let report = record_skill_change(
            ctx,
            request.seed,
            change.clone(),
            Some(request.skill_proposal.clone()),
            Some(request.proposer_evidence.clone()),
            Cost::zero(),
        )?;
        return Ok((report.child, child_bank, change));
    }
    let built = run_skill_builder(
        workspace_factory,
        evidence_store,
        ctx,
        request.seed,
        request.skill_proposal,
        request.proposer_evidence,
    )
    .await?;
    checkpoints.save(&EvoSkillCheckpoint::CandidateBuilt {
        run_id: request.run_id,
        cases: request.cases.to_vec(),
        seed_bank: request.seed_bank.clone(),
        baseline_score: request.baseline_score,
        failures: request.failures.to_vec(),
        proposal: request.skill_proposal.clone(),
        proposer_evidence: request.proposer_evidence.clone(),
        child_bank: built.child_bank.clone(),
        change: built.change.clone(),
    })?;
    Ok((built.child, built.child_bank, built.change))
}

struct CompletionRequest {
    run_id: RunId,
    seed_bank: SkillBank,
    baseline_score: f64,
    child: CandidateId,
    child_bank: SkillBank,
    skill_proposal: SkillProposal,
    change: SkillBankChange,
}

async fn complete_iteration(
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    evaluator: &EvoSkillEvaluator,
    population: &mut KeepBest,
    stores: &RunStores,
    request: CompletionRequest,
) -> Result<()> {
    let child_eval = evaluate_one(
        ctx,
        evaluator,
        request.child,
        VALIDATION,
        EvaluationPurpose::Validation,
    )
    .await?;
    observe_keep_best(ctx, population, request.child, &child_eval)?;

    let admitted = true;
    let best_score = request.baseline_score.max(child_eval.average_score);
    let best_bank = if child_eval.average_score >= request.baseline_score {
        request.child_bank
    } else {
        request.seed_bank
    };
    stores
        .checkpoints
        .save(&EvoSkillCheckpoint::IterationComplete {
            run_id: request.run_id,
            baseline_score: request.baseline_score,
            child_score: child_eval.average_score,
            admitted,
            best_score,
            best_bank,
        })?;

    println!(
        "evoskill iteration complete baseline={:.4} child={:.4} admitted={} best={:.4}",
        request.baseline_score, child_eval.average_score, admitted, best_score
    );
    println!("proposal_action={:?}", request.skill_proposal.action);
    println!("proposal={}", request.skill_proposal.proposed_skill);
    println!("change={:?}", request.change);
    println!("run_root={}", stores.run_root.display());
    println!("evidence_root={}", stores.evidence_store.root().display());
    Ok(())
}
#[derive(Default)]
struct CliArgs {
    live_codex: bool,
    run_dir: Option<PathBuf>,
}

impl CliArgs {
    fn parse() -> Result<Self> {
        let mut parsed = Self::default();
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--live-codex" => parsed.live_codex = true,
                "--run-dir" => {
                    let Some(value) = args.next() else {
                        return Err(msg("--run-dir requires a path"));
                    };
                    parsed.run_dir = Some(PathBuf::from(value));
                }
                other => return Err(msg(format!("unknown argument `{other}`"))),
            }
        }
        Ok(parsed)
    }
}

fn default_run_dir() -> PathBuf {
    PathBuf::from("tmp/p5_evoskill_iteration/live")
}

fn load_fixture_cases() -> Vec<EvoSkillCase> {
    load_cases(Path::new(
        "examples/p5_evoskill_iteration/fixtures/treasury-notation/cases.json",
    ))
    .expect("fixture cases load")
}

fn new_executor_evaluator(
    cases: &[EvoSkillCase],
    workspace_factory: &LocalWorkspaceFactory,
) -> EvoSkillEvaluator {
    EvoSkillEvaluator {
        cases: cases.to_vec(),
        workspace_factory: workspace_factory.clone(),
        runtime: live_codex_runtime(executor_developer_instructions()),
        developer_instructions: executor_developer_instructions(),
    }
}

#[derive(Default)]
struct ResumeState {
    run_id: Option<RunId>,
    cases: Option<Vec<EvoSkillCase>>,
    seed_bank: Option<SkillBank>,
    baseline_score: Option<f64>,
    failures: Option<Vec<CaseExecution>>,
    proposal: Option<SkillProposal>,
    proposer_evidence: Option<EvidenceRef>,
    child_bank: Option<SkillBank>,
    change: Option<SkillBankChange>,
}

impl ResumeState {
    fn from_checkpoint(
        checkpoint: Option<(leaven_kernel::CheckpointId, EvoSkillCheckpoint)>,
    ) -> Result<Self> {
        let Some((_id, checkpoint)) = checkpoint else {
            return Ok(Self::default());
        };
        Ok(match checkpoint {
            EvoSkillCheckpoint::BaselineComplete {
                run_id,
                cases,
                seed_bank,
                baseline_score,
            } => Self {
                run_id: Some(run_id),
                cases: Some(cases),
                seed_bank: Some(seed_bank),
                baseline_score: Some(baseline_score),
                ..Self::default()
            },
            EvoSkillCheckpoint::FailuresCollected {
                run_id,
                cases,
                seed_bank,
                baseline_score,
                failures,
            } => Self {
                run_id: Some(run_id),
                cases: Some(cases),
                seed_bank: Some(seed_bank),
                baseline_score: Some(baseline_score),
                failures: Some(failures),
                ..Self::default()
            },
            EvoSkillCheckpoint::ProposalComplete {
                run_id,
                cases,
                seed_bank,
                baseline_score,
                failures,
                proposal,
                proposer_evidence,
            } => Self {
                run_id: Some(run_id),
                cases: Some(cases),
                seed_bank: Some(seed_bank),
                baseline_score: Some(baseline_score),
                failures: Some(failures),
                proposal: Some(proposal),
                proposer_evidence: Some(proposer_evidence),
                ..Self::default()
            },
            EvoSkillCheckpoint::CandidateBuilt {
                run_id,
                cases,
                seed_bank,
                baseline_score,
                failures,
                proposal,
                proposer_evidence,
                child_bank,
                change,
            } => Self {
                run_id: Some(run_id),
                cases: Some(cases),
                seed_bank: Some(seed_bank),
                baseline_score: Some(baseline_score),
                failures: Some(failures),
                proposal: Some(proposal),
                proposer_evidence: Some(proposer_evidence),
                child_bank: Some(child_bank),
                change: Some(change),
            },
            EvoSkillCheckpoint::IterationComplete { .. } => {
                return Err(msg(
                    "complete checkpoint should be handled before resume state",
                ));
            }
        })
    }
}

struct ExistingBaseline {
    score: f64,
}

struct EvaluationOutcome {
    assessment: leaven_kernel::AssessmentId,
    average_score: f64,
    evidence: EvaluationEvidence,
}

struct EvaluationEvidence {
    cases: Vec<CaseExecution>,
}

async fn evaluate_one(
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    evaluator: &EvoSkillEvaluator,
    candidate: CandidateId,
    partition: &'static str,
    purpose: EvaluationPurpose,
) -> Result<EvaluationOutcome> {
    let report = ctx
        .evaluate_with(
            evaluator,
            EvaluationRequest::Independent {
                candidates: vec![candidate],
                set: EvaluationSet::Partition(leaven_core::PartitionId::from(partition)),
                granularity: AssessmentGranularity::Aggregate,
                purpose,
            },
        )
        .await?;
    let assessment = report
        .assessment_ids
        .first()
        .copied()
        .ok_or_else(|| msg("evaluator returned no assessment"))?;
    let evidence = ctx.assessment_evidence(assessment)?;
    let EvoSkillEvidence::Evaluation {
        average_score,
        cases,
        ..
    } = evidence
    else {
        return Err(msg("expected evaluation evidence"));
    };
    Ok(EvaluationOutcome {
        assessment,
        average_score,
        evidence: EvaluationEvidence { cases },
    })
}

fn observe_keep_best(
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    population: &mut KeepBest,
    candidate: CandidateId,
    evaluation: &EvaluationOutcome,
) -> Result<()> {
    let score = ScalarEvidence::new(evaluation.average_score)?;
    let events = population.observe(candidate, evaluation.assessment, score);
    ctx.emit(RunEvent::PopulationUpdated {
        population_id: population.id(),
        events,
    });
    Ok(())
}

struct EvoSkillEvaluator {
    cases: Vec<EvoSkillCase>,
    workspace_factory: LocalWorkspaceFactory,
    runtime: LiveCodexRuntime,
    developer_instructions: String,
}

impl Evaluator<EvoSkillProblem> for EvoSkillEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        self.runtime.fingerprint()
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'_, EvoSkillProblem>,
    ) -> std::result::Result<Metered<Vec<Assessment<EvoSkillProblem>>>, EvaluationError> {
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "expected independent request".to_owned(),
            ));
        };
        let mut assessments = Vec::new();
        let case_ids = request.set.case_ids.clone();
        for candidate in candidates {
            let mut executions = Vec::new();
            let mut cost = Cost::zero();
            for case_id in &case_ids {
                let case = case_by_id(&self.cases, *case_id).ok_or_else(|| {
                    EvaluationError::Message(format!("unknown case id {case_id}"))
                })?;
                let execution = self
                    .run_case(candidate, case, ctx.materialize_context())
                    .await
                    .map_err(|error| {
                        EvaluationError::with_source("executor agent failed", error)
                    })?;
                cost = cost.combine(&execution.cost);
                executions.push(execution.value);
            }
            let average_score = average_case_score(&executions);
            assessments.push(Assessment::Independent {
                candidate,
                target: AssessmentTarget::EvaluationSet(EvaluationSetId::new()),
                evidence: EvoSkillEvidence::Evaluation {
                    candidate,
                    split: format!("{:?}", request.purpose),
                    average_score,
                    cases: executions,
                },
                cost: cost.clone(),
                metadata: MetadataBag::new(),
            });
        }
        Ok(Metered::new(
            assessments,
            Cost::llm_calls(case_ids.len() as u64),
        ))
    }
}

impl EvoSkillEvaluator {
    async fn run_case(
        &self,
        candidate: CandidateId,
        case: &EvoSkillCase,
        materialize_context: leaven_engine::MaterializeContext<'_, EvoSkillProblem>,
    ) -> Result<Metered<CaseExecution>> {
        let mut workspace = self
            .workspace_factory
            .allocate(WorkspaceConfig::default())
            .await?;
        let stage_result = async {
            let mut view = workspace.view();
            SkillBankMaterializer::new(SkillWorkspaceLayout::new(".agents/skills")?)
                .materialize_into(
                    &SkillBankProposalInput::new(candidate),
                    &mut view,
                    materialize_context,
                )
                .await
                .map_err(|error| msg(error.to_string()))?;
            write_json(&mut view, "task/case.json", case)?;
            let mut request = AgentRunRequest::new(
                AgentInstructions::task(executor_task(case)),
                OutputContract::FinalMessage,
            );
            request.limits = AgentLimits {
                timeout: Some(Duration::from_secs(240)),
                ..AgentLimits::default()
            };
            let budget = leaven_kernel::BudgetSnapshot::default();
            let session = self
                .runtime
                .run_session(
                    &mut view,
                    request,
                    AgentRunContext::new(AgentSessionId::new(), &budget),
                )
                .await?;
            let answer: AgentAnswer = final_json(&session.value)?;
            let score = multi_tolerance_score(&case.answer, &answer.final_answer);
            Ok(Metered::new(
                CaseExecution {
                    case_id: case.id.clone(),
                    question: case.question.clone(),
                    expected_answer: case.answer.clone(),
                    predicted_answer: answer.final_answer,
                    score,
                    passed: score >= 0.8,
                    developer_instructions: self.developer_instructions.clone(),
                    session: session.value.clone(),
                },
                session.cost,
            ))
        }
        .await;
        finish_workspace(workspace, stage_result).await
    }
}

#[derive(serde::Deserialize)]
struct AgentAnswer {
    final_answer: String,
}

fn executor_task(case: &EvoSkillCase) -> String {
    format!(
        "Answer this case.\n\nQuestion:\n{}\n\nSource:\n{}\n\nInspect `.agents/skills` mentally from the task context and use a relevant skill when one exists. If no relevant skill exists for the specialized reusable conversion procedure, final_answer must be exactly `NOT_ATTEMPTED`.\n\nDo not call tools. Reply with JSON only: {{\"final_answer\":\"...\",\"reasoning\":\"...\"}}.",
        case.question, case.source
    )
}

struct StoredProposal {
    value: SkillProposal,
    evidence: EvidenceRef,
}

async fn run_skill_proposer(
    factory: &LocalWorkspaceFactory,
    evidence_store: &FileEvidenceStore<EvoSkillEvidence>,
    bank: &SkillBank,
    failures: &[CaseExecution],
) -> Result<StoredProposal> {
    let developer_instructions = proposer_developer_instructions();
    let runtime = live_codex_runtime(developer_instructions.clone());
    let mut workspace = factory.allocate(WorkspaceConfig::default()).await?;
    let stage_result = async {
        let mut view = workspace.view();
        materialize_skill_bank_direct(bank, &mut view)?;
        write_file(
            &mut view,
            ".claude/skills/brainstorming/SKILL.md",
            brainstorming_meta_skill().as_bytes(),
        )?;
        write_json(&mut view, "task/failures.json", failures)?;
        write_file(
            &mut view,
            "task/existing-skills.md",
            existing_skills_markdown(bank).as_bytes(),
        )?;
        let mut request = AgentRunRequest::new(
            AgentInstructions::task(proposer_task(failures, bank)?),
            OutputContract::FinalMessage,
        );
        request.limits = AgentLimits {
            timeout: Some(Duration::from_secs(240)),
            ..AgentLimits::default()
        };
        let budget = leaven_kernel::BudgetSnapshot::default();
        let session = runtime
            .run_session(
                &mut view,
                request,
                AgentRunContext::new(AgentSessionId::new(), &budget),
            )
            .await?;
        let proposal: SkillProposal = final_json(&session.value)?;
        let evidence = evidence_store.put(EvoSkillEvidence::AgentRoleSession {
            role: AgentRole::Proposer,
            developer_instructions: developer_instructions.clone(),
            session: session.value.clone(),
        })?;
        Ok(StoredProposal {
            value: proposal,
            evidence,
        })
    }
    .await;
    finish_workspace(workspace, stage_result).await
}

fn proposer_task(failures: &[CaseExecution], bank: &SkillBank) -> Result<String> {
    Ok(format!(
        "Follow your EvoSkill Proposer instructions.\n\
         The current skill inventory is:\n{}\n\
         Previous feedback history is empty for this one-iteration reproduction.\n\n\
         Current failures JSON:\n{}\n\n\
         Do not call tools. Reply with JSON only, with keys: action, target_skill, proposed_skill, justification, related_iterations.",
        existing_skills_markdown(bank),
        serde_json::to_string_pretty(failures)?
    ))
}

struct BuiltCandidate {
    child: CandidateId,
    child_bank: SkillBank,
    change: SkillBankChange,
}

async fn run_skill_builder(
    factory: &LocalWorkspaceFactory,
    evidence_store: &FileEvidenceStore<EvoSkillEvidence>,
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    parent: CandidateId,
    proposal: &SkillProposal,
    proposer_evidence: &EvidenceRef,
) -> Result<BuiltCandidate> {
    let developer_instructions = skill_builder_developer_instructions();
    let runtime = live_codex_runtime(developer_instructions.clone());
    let mut workspace = factory.allocate(WorkspaceConfig::default()).await?;
    let stage_result = async {
        let mut view = workspace.view();
        prepare_builder_workspace(&mut view, ctx.materialize_context(), parent, proposal).await?;
        let parser =
            SkillBankWorkspaceProposalParser::new(SkillWorkspaceLayout::new(".agents/skills")?);
        run_builder_repair_loop(
            &mut view,
            ctx,
            BuilderLoop {
                runtime: &runtime,
                evidence_store,
                developer_instructions: &developer_instructions,
                parser: &parser,
                parent,
                proposal,
                proposer_evidence,
            },
        )
        .await
    }
    .await;
    finish_workspace(workspace, stage_result).await
}

async fn prepare_builder_workspace(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    materialize_context: leaven_engine::MaterializeContext<'_, EvoSkillProblem>,
    parent: CandidateId,
    proposal: &SkillProposal,
) -> Result<()> {
    SkillBankMaterializer::new(SkillWorkspaceLayout::new(".agents/skills")?)
        .materialize_into(
            &SkillBankProposalInput::new(parent),
            view,
            materialize_context,
        )
        .await
        .map_err(|error| msg(error.to_string()))?;
    write_file(
        view,
        ".claude/skills/skill-creator/SKILL.md",
        skill_creator_meta_skill().as_bytes(),
    )?;
    write_json(view, "task/proposal.json", proposal)
}

struct BuilderLoop<'a> {
    runtime: &'a LiveCodexRuntime,
    evidence_store: &'a FileEvidenceStore<EvoSkillEvidence>,
    developer_instructions: &'a str,
    parser: &'a SkillBankWorkspaceProposalParser,
    parent: CandidateId,
    proposal: &'a SkillProposal,
    proposer_evidence: &'a EvidenceRef,
}

async fn run_builder_repair_loop(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    loop_state: BuilderLoop<'_>,
) -> Result<BuiltCandidate> {
    let mut last_error = None;
    for attempt in 1..=2 {
        let session = run_builder_attempt(
            view,
            ctx.budget(),
            &loop_state,
            attempt,
            last_error.as_deref(),
        )
        .await?;
        store_builder_session(&session.value, &loop_state)?;
        let generated: GeneratedSkill = final_json(&session.value)?;
        write_generated_skill(view, &generated)?;
        write_json(view, "output/skill-builder.json", &generated)?;

        match parse_builder_session(view, ctx, &session, &loop_state).await {
            Ok(candidate) => return Ok(candidate),
            Err(error) if attempt == 1 => last_error = Some(error.to_string()),
            Err(error) => return Err(msg(format!("skill builder exhausted repair: {error}"))),
        }
    }
    Err(msg("skill builder exhausted repair"))
}

async fn run_builder_attempt(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    budget: leaven_kernel::BudgetSnapshot,
    loop_state: &BuilderLoop<'_>,
    attempt: usize,
    last_error: Option<&str>,
) -> Result<Metered<leaven_agent::AgentSession>> {
    let mut request = AgentRunRequest::new(
        AgentInstructions::task(builder_task(loop_state.proposal, attempt, last_error)?),
        OutputContract::FinalMessage,
    );
    request.limits = AgentLimits {
        timeout: Some(Duration::from_secs(240)),
        ..AgentLimits::default()
    };
    Ok(loop_state
        .runtime
        .run_session(
            view,
            request,
            AgentRunContext::new(AgentSessionId::new(), &budget),
        )
        .await?)
}

fn store_builder_session(
    session: &leaven_agent::AgentSession,
    loop_state: &BuilderLoop<'_>,
) -> Result<EvidenceRef> {
    Ok(loop_state
        .evidence_store
        .put(EvoSkillEvidence::AgentRoleSession {
            role: AgentRole::SkillBuilder,
            developer_instructions: loop_state.developer_instructions.to_owned(),
            session: session.clone(),
        })?)
}

async fn parse_builder_session(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    session: &Metered<leaven_agent::AgentSession>,
    loop_state: &BuilderLoop<'_>,
) -> Result<BuiltCandidate> {
    let mut parsed = loop_state
        .parser
        .parse_proposals(
            view,
            &session.value,
            &SkillBankProposalInput::new(loop_state.parent),
            ctx.graph(),
        )
        .await
        .map_err(|error| msg(error.to_string()))?;
    annotate_builder_proposals(&mut parsed.value.proposals, loop_state);
    let change = parsed
        .value
        .proposals
        .first()
        .and_then(proposal_change)
        .ok_or_else(|| msg("builder parser returned no skill-bank change"))?;
    parsed.value.semantics = ProposalBatchSemantics::Alternatives;
    let recorded = ctx.record_proposal_batch(
        StageId::from_proposer(ProposerId::new_const("evoskill/skill-builder")),
        parsed.value,
        session.cost.clone().combine(&parsed.cost),
    )?;
    let applied = ctx.apply_batch(recorded.batch_id)?;
    let child = applied
        .successful_candidates()
        .next()
        .ok_or_else(|| msg("skill proposal did not apply"))?;
    let child_bank = ctx
        .graph()
        .artifact(child)
        .ok_or_else(|| msg("applied child artifact missing"))?
        .clone();
    Ok(BuiltCandidate {
        child,
        child_bank,
        change,
    })
}

fn annotate_builder_proposals(
    proposals: &mut [leaven_core::Proposal<EvoSkillProblem>],
    loop_state: &BuilderLoop<'_>,
) {
    for item in proposals {
        item.annotations = EvoSkillProposalAnnotations {
            proposal: Some(loop_state.proposal.clone()),
            proposer_evidence: Some(loop_state.proposer_evidence.clone()),
        };
        let _proposal_payload_present = item.annotations.proposal.is_some();
        item.provenance
            .informed_by
            .push(InfoRef::External(ExternalRef {
                kind: "evidence".to_owned(),
                id: format!(
                    "{}:{}",
                    loop_state.proposer_evidence.store, loop_state.proposer_evidence.key
                ),
            }));
    }
}

fn builder_task(
    proposal: &SkillProposal,
    attempt: usize,
    last_error: Option<&str>,
) -> Result<String> {
    let mut task = format!(
        "Follow your EvoSkill Skill-Builder instructions.\n\
         The proposal JSON is:\n{}\n\n\
         Do not call tools. Reply with JSON only:\n\
         {{\"skill_name\":\"skill-name\",\"files\":[{{\"path\":\"SKILL.md\",\"content\":\"...\",\"executable\":false}}],\"generated_skill\":\"...\",\"reasoning\":\"...\"}}.\n\
         File paths are relative to the skill root. The `SKILL.md` content must be a valid Agent Skill with YAML frontmatter and a non-empty body.",
        serde_json::to_string_pretty(proposal)?
    );
    if attempt > 1 {
        task.push_str("\n\nThe previous mutation was invalid. Repair the existing workspace in place. Validation error:\n");
        task.push_str(last_error.unwrap_or("unknown validation error"));
    }
    Ok(task)
}

#[derive(serde::Deserialize, serde::Serialize)]
struct GeneratedSkill {
    skill_name: String,
    files: Vec<GeneratedSkillFile>,
    #[serde(rename = "generated_skill")]
    generated_text: String,
    reasoning: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct GeneratedSkillFile {
    path: String,
    content: String,
    #[serde(default)]
    executable: bool,
}

fn write_generated_skill(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    generated: &GeneratedSkill,
) -> Result<()> {
    for file in &generated.files {
        let relative = generated_file_relative_path(&generated.skill_name, &file.path);
        let path = WorkspacePath::new(format!(
            ".agents/skills/{}/{}",
            generated.skill_name, relative
        ))?;
        view.write_file(&path, file.content.as_bytes())?;
        if file.executable {
            view.set_executable(&path, true)?;
        }
    }
    Ok(())
}

fn generated_file_relative_path<'a>(skill_name: &str, path: &'a str) -> &'a str {
    path.strip_prefix(".agents/skills/")
        .and_then(|rest| rest.strip_prefix(skill_name))
        .and_then(|rest| rest.strip_prefix('/'))
        .or_else(|| path.strip_prefix("./"))
        .unwrap_or(path)
}

fn final_json<T: DeserializeOwned>(session: &leaven_agent::AgentSession) -> Result<T> {
    let message = final_assistant_message(session)
        .ok_or_else(|| msg("agent session did not contain an assistant final message"))?;
    let json = extract_json_object(message)?;
    Ok(serde_json::from_str(json)?)
}

fn final_assistant_message(session: &leaven_agent::AgentSession) -> Option<&str> {
    session.transcript.events.iter().rev().find_map(|event| {
        if let TranscriptEvent::Message {
            role: TranscriptRole::Assistant,
            content,
        } = event
        {
            let trimmed = content.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        } else {
            None
        }
    })
}

fn extract_json_object(message: &str) -> Result<&str> {
    let trimmed = message.trim();
    if trimmed.starts_with('{') {
        return Ok(trimmed);
    }
    if let Some(rest) = trimmed.strip_prefix("```json")
        && let Some((json, _)) = rest.trim_start().split_once("```")
    {
        return Ok(json.trim());
    }
    if let Some(rest) = trimmed.strip_prefix("```")
        && let Some((json, _)) = rest.trim_start().split_once("```")
    {
        return Ok(json.trim());
    }
    let start = trimmed
        .find('{')
        .ok_or_else(|| msg("assistant final message did not contain JSON"))?;
    let end = trimmed
        .rfind('}')
        .ok_or_else(|| msg("assistant final message did not contain a complete JSON object"))?;
    Ok(&trimmed[start..=end])
}

fn proposal_change(proposal: &leaven_core::Proposal<EvoSkillProblem>) -> Option<SkillBankChange> {
    match &proposal.effect {
        leaven_core::ProposalEffect::Change { change, .. } => Some(change.clone()),
        leaven_core::ProposalEffect::Create { .. } => None,
    }
}

fn record_skill_change(
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    parent: CandidateId,
    change: SkillBankChange,
    proposal: Option<SkillProposal>,
    proposer_evidence: Option<EvidenceRef>,
    cost: Cost,
) -> Result<BuiltCandidate> {
    let child_bank = ctx
        .graph()
        .artifact(parent)
        .ok_or_else(|| msg("parent artifact missing"))?
        .apply_skill_change(&change)?;
    let mut item = leaven_core::Proposal::<EvoSkillProblem>::mutate(parent, change.clone())
        .annotations(EvoSkillProposalAnnotations {
            proposal,
            proposer_evidence,
        })
        .build();
    if let Some(reference) = &item.annotations.proposer_evidence {
        item.provenance
            .informed_by
            .push(InfoRef::External(ExternalRef {
                kind: "evidence".to_owned(),
                id: format!("{}:{}", reference.store, reference.key),
            }));
    }
    let batch = leaven_core::ProposalBatch {
        proposals: vec![item],
        semantics: ProposalBatchSemantics::Alternatives,
        metadata: MetadataBag::new(),
    };
    let recorded = ctx.record_proposal_batch(
        StageId::from_proposer(ProposerId::new_const("evoskill/resume")),
        batch,
        cost,
    )?;
    let applied = ctx.apply_batch(recorded.batch_id)?;
    let child = applied
        .successful_candidates()
        .next()
        .ok_or_else(|| msg("resumed skill proposal did not apply"))?;
    Ok(BuiltCandidate {
        child,
        child_bank,
        change,
    })
}

fn average_case_score(executions: &[CaseExecution]) -> f64 {
    let total = executions.iter().map(|case| case.score).sum::<f64>();
    let count = u32::try_from(executions.len()).expect("fixture case count fits in u32");
    total / f64::from(count)
}

fn materialize_skill_bank_direct(
    bank: &SkillBank,
    view: &mut leaven_workspace::WorkspaceView<'_>,
) -> Result<()> {
    for (skill_name, folder) in bank.folders() {
        for (path, file) in folder.entries() {
            let workspace_path = WorkspacePath::new(format!(".agents/skills/{skill_name}/{path}"))?;
            view.write_file(&workspace_path, file.bytes())?;
            if file.permissions().executable {
                view.set_executable(&workspace_path, true)?;
            }
        }
    }
    Ok(())
}

fn existing_skills_markdown(bank: &SkillBank) -> String {
    if bank.is_empty() {
        return "None".to_owned();
    }
    let mut out = String::new();
    for (name, folder) in bank.folders() {
        out.push_str("- ");
        out.push_str(name.as_str());
        out.push_str(": ");
        out.push_str(folder.manifest().description.as_str());
        out.push('\n');
    }
    out
}

fn write_json<T: serde::Serialize + ?Sized>(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    path: &str,
    value: &T,
) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_file(view, path, &bytes)
}

fn write_file(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    path: &str,
    bytes: &[u8],
) -> Result<()> {
    Ok(view.write_file(&WorkspacePath::new(path)?, bytes)?)
}

async fn finish_workspace<T>(workspace: Workspace, stage_result: Result<T>) -> Result<T> {
    let cleanup = workspace.cleanup().await;
    match (stage_result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(cleanup)) => Err(ExampleError::Workspace(cleanup)),
        (Err(stage), Ok(())) => Err(stage),
        (Err(stage), Err(cleanup)) => Err(msg(format!(
            "stage failed and workspace cleanup failed: {stage}; cleanup: {cleanup}"
        ))),
    }
}
