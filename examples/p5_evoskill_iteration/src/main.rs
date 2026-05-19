mod checkpoint;
mod codex;
mod data;
mod error;
mod evidence;
mod proposal;
mod roles;
mod scorer;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use leaven_agent::{
    AgentInstructions, AgentLimits, AgentRunContext, AgentRunRequest, AgentRuntime, OutputContract,
    TranscriptEvent, TranscriptRole,
};
use leaven_agentic::{
    AgentCase, AgentCaseEvaluator, AgentCaseEvaluatorConfig, AgentCasePresentation,
    AgentCasePresentationInput, AgentCasePresenter, AgentCaseRunRecord, AgentCaseScoreInput,
    AgentCaseScorer, AgentPromptTarget, AgentRunPreflight, AgentWorkload, AgenticAdapterError,
    AgenticCostInspection, AgenticParseError, AgenticRepairError, AgenticRunInspection, CaseInput,
    CaseSuite, CaseTarget, PreflightSeverity, PresenterDryRun, ProposalParser,
    ProposalRepairFeedback, ProposalRepairInspection, ProposalRepairPromptBuilder,
    RepairingAgenticProposer, RepairingAgenticProposerConfig, ScorerDryRun,
};
use leaven_agentic_skill::{
    SkillBankChangeReport, SkillBankMaterializer, SkillBankProposalInput,
    SkillBankWorkspaceProposalParser, SkillWorkspaceLayout,
};
use leaven_artifact_skill::{SkillBank, SkillBankChange};
use leaven_core::{
    Artifact, AssessmentGranularity, EvaluationPurpose, EvaluationRequest, EvaluationSet,
    ExternalRef, InfoRef, OptimizationProblem, ProposalBatchSemantics,
};
use leaven_engine::{
    BudgetLedger, CachePolicy, EvaluationCache, MaterializationReport, MaterializeContext,
    MaterializeError, Materializer, OptimizerStateWrite, RenderContext, RenderError, Renderer,
    RestoredRunState, RunContext, RunEvent, RunGraph, StoreRunPersistence,
};
use leaven_evidence::ScalarEvidence;
use leaven_kernel::{
    AgentSessionId, Budget, CandidateId, CaseId, Cost, EvaluatorId, EvidenceRef, Fingerprint,
    MetadataBag, Metered, ProposerId, RunId, StageId,
};
use leaven_population::{TopKFrontier, TopKParentSelector};
use leaven_store::EvidenceStore;
use leaven_store_file::{FileCheckpointStore, FileEvidenceStore, FileStore};
use leaven_workspace::{Workspace, WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;
use serde::de::DeserializeOwned;
use std::num::NonZeroUsize;

use crate::checkpoint::EvoSkillCheckpoint;
use crate::codex::{LiveCodexRuntime, live_codex_runtime, require_live_codex};
use crate::data::{EvoSkillCase, Split, TRAIN, VALIDATION, case_set, load_cases};
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
    let restored_run = stores
        .run_persistence
        .latest_checkpoint::<EvoSkillProblem>()?;
    if stores.print_complete_resume(restored_run.as_ref())? {
        return Ok(());
    }

    let resume = stores.resume_state(restored_run.as_ref())?;
    run_iteration(stores, resume, restored_run).await
}

struct RunStores {
    run_root: PathBuf,
    evidence_store: FileEvidenceStore<EvoSkillEvidence>,
    run_persistence: StoreRunPersistence<FileStore>,
}

impl RunStores {
    fn open(run_root: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&run_root)?;
        let evidence_store = FileEvidenceStore::<EvoSkillEvidence>::open(
            "p5-evoskill-evidence",
            run_root.join("evidence"),
        )?;
        let run_persistence =
            StoreRunPersistence::new(FileStore::open(run_root.join("run-store"))?);
        Ok(Self {
            run_root,
            evidence_store,
            run_persistence,
        })
    }

    fn resume_state(
        &self,
        restored: Option<&RestoredRunState<EvoSkillProblem>>,
    ) -> Result<ResumeState> {
        ResumeState::from_checkpoint(self.load_private_checkpoint(restored)?)
    }

    fn print_complete_resume(
        &self,
        restored: Option<&RestoredRunState<EvoSkillProblem>>,
    ) -> Result<bool> {
        let Some(EvoSkillCheckpoint::IterationComplete {
            baseline_score,
            child_score,
            admitted,
            best_score,
            ..
        }) = self.load_private_checkpoint(restored)?
        else {
            return Ok(false);
        };
        println!(
            "resume: iteration already complete baseline={baseline_score:.4} child={child_score:.4} admitted={admitted} best={best_score:.4}"
        );
        println!("run_root={}", self.run_root.display());
        Ok(true)
    }

    fn load_private_checkpoint(
        &self,
        restored: Option<&RestoredRunState<EvoSkillProblem>>,
    ) -> Result<Option<EvoSkillCheckpoint>> {
        let Some(restored) = restored else {
            return Ok(None);
        };
        let checkpoint = self.run_persistence.load_optimizer_state(
            &restored.checkpoint,
            EVOSKILL_OPTIMIZER_FINGERPRINT,
            EVOSKILL_STATE_SCHEMA,
        )?;
        if checkpoint.is_none() && restored.graph.candidate_count() > 1 {
            return Err(msg(
                "restored graph contains non-seed candidates but no EvoSkill private checkpoint state",
            ));
        }
        Ok(checkpoint)
    }
}

const EVOSKILL_OPTIMIZER_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([31; 32]);
const EVOSKILL_STATE_SCHEMA: Fingerprint = Fingerprint::from_bytes([32; 32]);
const EVOSKILL_FRONTIER_SIZE: usize = 3;

fn evoskill_state_write(state: &EvoSkillCheckpoint) -> Result<OptimizerStateWrite> {
    Ok(OptimizerStateWrite::json(
        EVOSKILL_OPTIMIZER_FINGERPRINT,
        EVOSKILL_STATE_SCHEMA,
        state,
    )?)
}

#[expect(
    clippy::future_not_send,
    reason = "preflight dry-runs borrow RunContext capability views across awaits in this single-threaded example"
)]
async fn write_preflight_report(
    stores: &RunStores,
    workload: &AgentWorkload,
    seed_bank: &SkillBank,
    seed: CandidateId,
    workspace_factory: &LocalWorkspaceFactory,
    stack: &ExecutorStack,
    ctx: &RunContext<'_, EvoSkillProblem>,
) -> Result<()> {
    let Some(sample_case) = workload.cases().cases().values().next() else {
        return Err(msg("preflight workload has no cases"));
    };
    let mut synthetic_session = leaven_agent::AgentSession::succeeded(AgentSessionId::new());
    synthetic_session.transcript.push_message(
        TranscriptRole::Assistant,
        synthetic_answer_for(sample_case)?,
    );
    let synthetic_presentation = AgentCasePresentation {
        request: AgentRunRequest::new(
            AgentInstructions::task("synthetic scorer preflight"),
            OutputContract::FinalMessage,
        ),
        materialized_refs: Vec::new(),
    };
    let report = AgentRunPreflight::new()
        .artifact(seed_bank)
        .workload(workload)
        .runtime(&stack.runtime)
        .output_contract(&OutputContract::FinalMessage)
        .cache_identity(seed_bank, &CachePolicy::Never)
        .store(stores.run_persistence.store())
        .checkpoint_store(&FileCheckpointStore::open(
            stores.run_root.join("preflight-checkpoints"),
        )?)
        .presenter_dry_run(PresenterDryRun {
            candidate_id: seed,
            candidate: seed_bank,
            case: sample_case,
            factory: workspace_factory,
            workspace_config: WorkspaceConfig::default(),
            presenter: &stack.presenter,
            ctx: ctx.materialize_context(),
        })
        .await
        .scorer_dry_run(ScorerDryRun {
            candidate_id: seed,
            case: sample_case,
            presentation: &synthetic_presentation,
            session: &synthetic_session,
            workspace_files: Vec::new(),
            factory: workspace_factory,
            workspace_config: WorkspaceConfig::default(),
            scorer: &stack.scorer,
            graph: ctx.graph(),
        })
        .await
        .check();
    let path = stores.run_root.join("preflight_report.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    if report.has_errors() {
        for finding in report.findings() {
            if finding.severity == PreflightSeverity::Error {
                eprintln!("preflight error [{}]: {}", finding.check, finding.message);
            }
        }
        return Err(msg(format!("preflight failed; report={}", path.display())));
    }
    Ok(())
}

#[expect(
    clippy::too_many_lines,
    reason = "P5 keeps the one-iteration checkpoint/resume flow visible as the paper-reproduction proof"
)]
#[expect(
    clippy::future_not_send,
    reason = "the example intentionally carries RunContext capability views through async stage calls"
)]
async fn run_iteration(
    stores: RunStores,
    resume: ResumeState,
    restored_run: Option<RestoredRunState<EvoSkillProblem>>,
) -> Result<()> {
    let cases = resume.cases.unwrap_or_else(load_fixture_cases);
    let run_id = resume
        .run_id
        .or_else(|| {
            restored_run
                .as_ref()
                .map(|restored| restored.checkpoint.run_id)
        })
        .unwrap_or_default();
    let seed_bank = resume.seed_bank.unwrap_or_default();
    let case_set = case_set(&cases);

    let (mut graph, mut budget, mut cache) = match restored_run {
        Some(restored) => {
            let restored_run_id = restored.checkpoint.run_id;
            if restored_run_id != run_id {
                return Err(msg(format!(
                    "phase checkpoint run_id {run_id} does not match restored run graph {restored_run_id}"
                )));
            }
            (
                restored.graph,
                restored.budget,
                restored.cache.unwrap_or_default(),
            )
        }
        None => (
            RunGraph::<EvoSkillProblem>::new(run_id),
            BudgetLedger::new(Budget::unlimited()),
            EvaluationCache::default(),
        ),
    };
    let mut population = resume.frontier.unwrap_or_else(evoskill_frontier);
    let mut parent_selector = resume
        .parent_selector
        .unwrap_or_else(evoskill_parent_selector);
    let workspace_factory = LocalWorkspaceFactory::new(stores.run_root.join("workspaces"));
    let executor = new_executor_stack(&cases, &workspace_factory)?;

    let mut ctx = RunContext::<EvoSkillProblem>::new(&mut graph, &mut budget)
        .with_case_set(&case_set)
        .with_cache(&mut cache)
        .with_evidence_store(&stores.evidence_store)
        .with_persistence(Some(&stores.run_persistence));
    let seed = ensure_seed_candidate(&mut ctx, &seed_bank)?;
    write_preflight_report(
        &stores,
        &executor.workload,
        &seed_bank,
        seed,
        &workspace_factory,
        &executor,
        &ctx,
    )
    .await?;

    let baseline = ensure_baseline(
        &mut ctx,
        &executor.evaluator,
        &mut population,
        BaselineRequest {
            run_id,
            cases: &cases,
            seed_bank: &seed_bank,
            seed,
            parent_selector: &parent_selector,
            resume_score: resume.baseline_score,
        },
    )
    .await?;
    let parent = match resume.parent {
        Some(parent) => parent,
        None => select_frontier_parent(&mut parent_selector, &population)?,
    };
    let parent_bank = ctx
        .graph()
        .artifact(parent)
        .ok_or_else(|| msg("selected parent artifact missing"))?
        .clone();
    let failures = ensure_failures(
        &mut ctx,
        &executor.evaluator,
        FailureRequest {
            run_id,
            cases: &cases,
            seed_bank: &seed_bank,
            parent,
            baseline_score: baseline.score,
            frontier: &population,
            parent_selector: &parent_selector,
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
        &ctx,
        &workspace_factory,
        &stores.evidence_store,
        ProposalRequest {
            run_id,
            cases: &cases,
            seed_bank: &seed_bank,
            parent,
            parent_bank: &parent_bank,
            baseline_score: baseline.score,
            frontier: &population,
            parent_selector: &parent_selector,
            failures: &failures,
            resume_proposal: resume.proposal,
            resume_evidence: resume.proposer_evidence,
        },
    )
    .await?;
    let (child, child_bank, change) = ensure_child(
        &workspace_factory,
        &stores.evidence_store,
        &mut ctx,
        ChildRequest {
            run_id,
            cases: &cases,
            seed_bank: &seed_bank,
            parent,
            baseline_score: baseline.score,
            frontier: &population,
            parent_selector: &parent_selector,
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
        &executor.evaluator,
        &mut population,
        &parent_selector,
        &stores,
        CompletionRequest {
            run_id,
            seed_bank,
            seed,
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
    parent_selector: &'a TopKParentSelector,
    resume_score: Option<f64>,
}

async fn ensure_baseline(
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    evaluator: &EvoSkillEvaluator,
    population: &mut TopKFrontier,
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
    observe_frontier(ctx, population, request.seed, &validation)?;
    checkpoint_phase(
        ctx,
        &EvoSkillCheckpoint::BaselineComplete {
            run_id: request.run_id,
            cases: request.cases.to_vec(),
            seed_bank: request.seed_bank.clone(),
            baseline_score: validation.average_score,
            frontier: population.clone(),
            parent_selector: request.parent_selector.clone(),
        },
    )?;
    Ok(ExistingBaseline {
        score: validation.average_score,
    })
}

struct FailureRequest<'a> {
    run_id: RunId,
    cases: &'a [EvoSkillCase],
    seed_bank: &'a SkillBank,
    parent: CandidateId,
    baseline_score: f64,
    frontier: &'a TopKFrontier,
    parent_selector: &'a TopKParentSelector,
    resume_failures: Option<Vec<CaseExecution>>,
}

async fn ensure_failures(
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    evaluator: &EvoSkillEvaluator,
    request: FailureRequest<'_>,
) -> Result<Vec<CaseExecution>> {
    if let Some(failures) = request.resume_failures {
        return Ok(failures);
    }
    let feedback = evaluate_one(
        ctx,
        evaluator,
        request.parent,
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
    checkpoint_phase(
        ctx,
        &EvoSkillCheckpoint::FailuresCollected {
            run_id: request.run_id,
            cases: request.cases.to_vec(),
            seed_bank: request.seed_bank.clone(),
            baseline_score: request.baseline_score,
            frontier: request.frontier.clone(),
            parent_selector: request.parent_selector.clone(),
            parent: request.parent,
            failures: failures.clone(),
        },
    )?;
    Ok(failures)
}

struct ProposalRequest<'a> {
    run_id: RunId,
    cases: &'a [EvoSkillCase],
    seed_bank: &'a SkillBank,
    parent: CandidateId,
    parent_bank: &'a SkillBank,
    baseline_score: f64,
    frontier: &'a TopKFrontier,
    parent_selector: &'a TopKParentSelector,
    failures: &'a [CaseExecution],
    resume_proposal: Option<SkillProposal>,
    resume_evidence: Option<EvidenceRef>,
}

#[expect(
    clippy::future_not_send,
    reason = "checkpointing after the proposer awaits borrows the RunContext view in this example"
)]
async fn ensure_proposal(
    ctx: &RunContext<'_, EvoSkillProblem>,
    workspace_factory: &LocalWorkspaceFactory,
    evidence_store: &FileEvidenceStore<EvoSkillEvidence>,
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
        request.parent_bank,
        request.failures,
    )
    .await?;
    checkpoint_phase(
        ctx,
        &EvoSkillCheckpoint::ProposalComplete {
            run_id: request.run_id,
            cases: request.cases.to_vec(),
            seed_bank: request.seed_bank.clone(),
            baseline_score: request.baseline_score,
            frontier: request.frontier.clone(),
            parent_selector: request.parent_selector.clone(),
            parent: request.parent,
            failures: request.failures.to_vec(),
            proposal: proposal.value.clone(),
            proposer_evidence: proposal.evidence.clone(),
        },
    )?;
    Ok((proposal.value, proposal.evidence))
}

struct ChildRequest<'a> {
    run_id: RunId,
    cases: &'a [EvoSkillCase],
    seed_bank: &'a SkillBank,
    parent: CandidateId,
    baseline_score: f64,
    frontier: &'a TopKFrontier,
    parent_selector: &'a TopKParentSelector,
    failures: &'a [CaseExecution],
    skill_proposal: &'a SkillProposal,
    proposer_evidence: &'a EvidenceRef,
    resume_child_bank: Option<SkillBank>,
    resume_change: Option<SkillBankChange>,
}

async fn ensure_child(
    workspace_factory: &LocalWorkspaceFactory,
    evidence_store: &FileEvidenceStore<EvoSkillEvidence>,
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    request: ChildRequest<'_>,
) -> Result<(CandidateId, SkillBank, SkillBankChange)> {
    if let (Some(child_bank), Some(change)) = (request.resume_child_bank, request.resume_change) {
        let identity = child_bank.identity();
        if let Some(child) = ctx
            .graph()
            .candidates_with_identity(&identity)
            .first()
            .copied()
        {
            return Ok((child, child_bank, change));
        }
        let report = record_skill_change(
            ctx,
            request.parent,
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
        request.parent,
        request.skill_proposal,
        request.proposer_evidence,
    )
    .await?;
    checkpoint_phase(
        ctx,
        &EvoSkillCheckpoint::CandidateBuilt {
            run_id: request.run_id,
            cases: request.cases.to_vec(),
            seed_bank: request.seed_bank.clone(),
            baseline_score: request.baseline_score,
            frontier: request.frontier.clone(),
            parent_selector: request.parent_selector.clone(),
            parent: request.parent,
            failures: request.failures.to_vec(),
            proposal: request.skill_proposal.clone(),
            proposer_evidence: request.proposer_evidence.clone(),
            child_bank: built.child_bank.clone(),
            change: built.change.clone(),
        },
    )?;
    Ok((built.child, built.child_bank, built.change))
}

struct CompletionRequest {
    run_id: RunId,
    seed_bank: SkillBank,
    seed: CandidateId,
    baseline_score: f64,
    child: CandidateId,
    child_bank: SkillBank,
    skill_proposal: SkillProposal,
    change: SkillBankChange,
}

#[derive(serde::Serialize)]
struct P5ResultSummary {
    run_id: RunId,
    baseline_score: f64,
    child_score: f64,
    admitted: bool,
    best_score: f64,
    best_candidate: CandidateId,
    best_lineage: Vec<CandidateId>,
    proposal_action: String,
    proposal: String,
    skill_change_report: SkillBankChangeReport,
    proposal_repair_batches: usize,
    proposal_repair_attempts: usize,
    proposal_repairs: Vec<ProposalRepairInspection>,
    child_case_executions: Vec<CaseExecution>,
    case_run_records: Vec<AgentCaseRunRecord>,
    cache_events: usize,
    cache_bypasses: usize,
    child_evaluation_cost: Cost,
    costs: AgenticCostInspection,
    latest_checkpoint: Option<String>,
    checkpoint_files: Vec<String>,
    warnings: Vec<String>,
}

async fn complete_iteration(
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    evaluator: &EvoSkillEvaluator,
    population: &mut TopKFrontier,
    parent_selector: &TopKParentSelector,
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
    observe_frontier(ctx, population, request.child, &child_eval)?;

    let admitted = true;
    let best_score = request.baseline_score.max(child_eval.average_score);
    let best_candidate = if child_eval.average_score >= request.baseline_score {
        request.child
    } else {
        request.seed
    };
    let best_bank = if child_eval.average_score >= request.baseline_score {
        request.child_bank.clone()
    } else {
        request.seed_bank.clone()
    };
    let skill_change_report =
        SkillBankChangeReport::from_change(&request.seed_bank, &request.change)?;
    ctx.emit(RunEvent::OptimizationEnded {
        run_id: request.run_id,
        best: Some(best_candidate),
        budget: ctx.budget(),
    });
    checkpoint_phase(
        ctx,
        &EvoSkillCheckpoint::IterationComplete {
            run_id: request.run_id,
            baseline_score: request.baseline_score,
            child_score: child_eval.average_score,
            admitted,
            best_score,
            best_bank,
            frontier: population.clone(),
            parent_selector: parent_selector.clone(),
        },
    )?;
    let inspection = AgenticRunInspection::from_graph(&ctx.graph());
    let proposal_repair_attempts = inspection
        .proposal_repairs
        .iter()
        .map(|repair| repair.attempts.len())
        .sum();
    let warnings = inspection
        .warnings
        .iter()
        .map(|warning| format!("{warning:?}"))
        .collect::<Vec<_>>();
    let summary = P5ResultSummary {
        run_id: request.run_id,
        baseline_score: request.baseline_score,
        child_score: child_eval.average_score,
        admitted,
        best_score,
        best_candidate,
        proposal_action: format!("{:?}", request.skill_proposal.action),
        proposal: request.skill_proposal.proposed_skill.clone(),
        skill_change_report,
        best_lineage: inspection.best_lineage.clone(),
        proposal_repair_batches: inspection.proposal_repairs.len(),
        proposal_repair_attempts,
        proposal_repairs: inspection.proposal_repairs,
        child_case_executions: child_eval.evidence.cases.clone(),
        case_run_records: inspection.case_runs,
        cache_events: inspection.cache_events.len(),
        cache_bypasses: inspection
            .cache_events
            .iter()
            .filter(|event| matches!(event.cache, leaven_engine::CacheStatus::Bypassed(_)))
            .count(),
        child_evaluation_cost: child_eval.cost.clone(),
        costs: inspection.costs,
        latest_checkpoint: latest_run_checkpoint(&stores.run_root)?,
        checkpoint_files: list_run_checkpoints(&stores.run_root)?,
        warnings,
    };
    let summary_path = stores.run_root.join("result_summary.json");
    std::fs::write(&summary_path, serde_json::to_vec_pretty(&summary)?)?;

    println!(
        "evoskill iteration complete baseline={:.4} child={:.4} admitted={} best={:.4}",
        request.baseline_score, child_eval.average_score, admitted, best_score
    );
    println!("proposal_action={:?}", request.skill_proposal.action);
    println!("proposal={}", request.skill_proposal.proposed_skill);
    println!("change={:?}", request.change);
    println!("result_summary={}", summary_path.display());
    println!("run_root={}", stores.run_root.display());
    println!("evidence_root={}", stores.evidence_store.root().display());
    Ok(())
}

fn list_run_checkpoints(run_root: &Path) -> Result<Vec<String>> {
    let checkpoints = run_root.join("run-store").join("checkpoints");
    if !checkpoints.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&checkpoints)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "checkpoint")
        {
            files.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    files.sort();
    Ok(files)
}

fn latest_run_checkpoint(run_root: &Path) -> Result<Option<String>> {
    let latest = run_root
        .join("run-store")
        .join("checkpoints")
        .join("LATEST");
    if !latest.exists() {
        return Ok(None);
    }
    let value = std::fs::read_to_string(latest)?;
    Ok(Some(value.trim().to_owned()))
}

fn checkpoint_phase(
    ctx: &RunContext<'_, EvoSkillProblem>,
    state: &EvoSkillCheckpoint,
) -> Result<()> {
    Ok(ctx.checkpoint_with_optimizer_state(evoskill_state_write(state)?)?)
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
    PathBuf::from("tmp/p5_evoskill_iteration/live-cli")
}

fn load_fixture_cases() -> Vec<EvoSkillCase> {
    load_cases(Path::new(
        "examples/p5_evoskill_iteration/fixtures/treasury-notation/cases.json",
    ))
    .expect("fixture cases load")
}

type EvoSkillEvaluator = AgentCaseEvaluator<
    EvoSkillProblem,
    LocalWorkspaceFactory,
    LiveCodexRuntime,
    EvoSkillPresenter,
    EvoSkillScorer,
>;

struct ExecutorStack {
    workload: AgentWorkload,
    runtime: LiveCodexRuntime,
    presenter: EvoSkillPresenter,
    scorer: EvoSkillScorer,
    evaluator: EvoSkillEvaluator,
}

fn new_executor_stack(
    cases: &[EvoSkillCase],
    workspace_factory: &LocalWorkspaceFactory,
) -> Result<ExecutorStack> {
    let mut agent_cases = BTreeMap::new();
    let mut train = Vec::new();
    let mut validation = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let id = CaseId::from_index(index);
        match case.split {
            Split::Train => train.push(id),
            Split::Validation => validation.push(id),
        }
        agent_cases.insert(id, agent_case_from_evo(index, case));
    }
    let partitions =
        leaven_agentic::CasePartitions::with_all(agent_cases.keys().copied().collect())
            .with_partition(leaven_agentic::CasePartitionId::from(TRAIN), train)
            .with_partition(
                leaven_agentic::CasePartitionId::from(VALIDATION),
                validation,
            );
    let workload = AgentWorkload::new(CaseSuite::new(agent_cases, partitions)?);
    let runtime = live_codex_runtime(executor_developer_instructions());
    let presenter = EvoSkillPresenter {
        developer_instructions: executor_developer_instructions(),
    };
    let scorer = EvoSkillScorer {
        developer_instructions: executor_developer_instructions(),
    };
    let evaluator = AgentCaseEvaluator::new(
        AgentCaseEvaluatorConfig::new(EvaluatorId::PRIMARY, Fingerprint::from_bytes([41; 32])),
        workload.cases().clone(),
        workspace_factory.clone(),
        runtime.clone(),
        presenter.clone(),
        scorer.clone(),
    );
    Ok(ExecutorStack {
        workload,
        runtime,
        presenter,
        scorer,
        evaluator,
    })
}

fn agent_case_from_evo(index: usize, case: &EvoSkillCase) -> AgentCase {
    AgentCase {
        id: CaseId::from_index(index),
        input: CaseInput::Structured(serde_json::json!({
            "id": case.id,
            "question": case.question,
            "source": case.source,
        })),
        target: CaseTarget::Text(case.answer.clone()),
        metadata: MetadataBag::new(),
        files: Default::default(),
        setup: None,
        workspace: None,
    }
}

fn ensure_seed_candidate(
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    seed_bank: &SkillBank,
) -> Result<CandidateId> {
    let identity = seed_bank.identity();
    let existing = ctx.graph().candidates_with_identity(&identity);
    if let Some(seed) = existing.first().copied() {
        return Ok(seed);
    }
    Ok(ctx.insert_seed(seed_bank.clone(), 0)?)
}

#[derive(Default)]
struct ResumeState {
    run_id: Option<RunId>,
    cases: Option<Vec<EvoSkillCase>>,
    seed_bank: Option<SkillBank>,
    baseline_score: Option<f64>,
    frontier: Option<TopKFrontier>,
    parent_selector: Option<TopKParentSelector>,
    parent: Option<CandidateId>,
    failures: Option<Vec<CaseExecution>>,
    proposal: Option<SkillProposal>,
    proposer_evidence: Option<EvidenceRef>,
    child_bank: Option<SkillBank>,
    change: Option<SkillBankChange>,
}

impl ResumeState {
    fn from_checkpoint(checkpoint: Option<EvoSkillCheckpoint>) -> Result<Self> {
        let Some(checkpoint) = checkpoint else {
            return Ok(Self::default());
        };
        Ok(match checkpoint {
            EvoSkillCheckpoint::BaselineComplete {
                run_id,
                cases,
                seed_bank,
                baseline_score,
                frontier,
                parent_selector,
            } => Self {
                run_id: Some(run_id),
                cases: Some(cases),
                seed_bank: Some(seed_bank),
                baseline_score: Some(baseline_score),
                frontier: Some(frontier),
                parent_selector: Some(parent_selector),
                ..Self::default()
            },
            EvoSkillCheckpoint::FailuresCollected {
                run_id,
                cases,
                seed_bank,
                baseline_score,
                frontier,
                parent_selector,
                parent,
                failures,
            } => Self {
                run_id: Some(run_id),
                cases: Some(cases),
                seed_bank: Some(seed_bank),
                baseline_score: Some(baseline_score),
                frontier: Some(frontier),
                parent_selector: Some(parent_selector),
                parent: Some(parent),
                failures: Some(failures),
                ..Self::default()
            },
            EvoSkillCheckpoint::ProposalComplete {
                run_id,
                cases,
                seed_bank,
                baseline_score,
                frontier,
                parent_selector,
                parent,
                failures,
                proposal,
                proposer_evidence,
            } => Self {
                run_id: Some(run_id),
                cases: Some(cases),
                seed_bank: Some(seed_bank),
                baseline_score: Some(baseline_score),
                frontier: Some(frontier),
                parent_selector: Some(parent_selector),
                parent: Some(parent),
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
                frontier,
                parent_selector,
                parent,
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
                frontier: Some(frontier),
                parent_selector: Some(parent_selector),
                parent: Some(parent),
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
    cost: Cost,
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
                granularity: AssessmentGranularity::PerCase,
                purpose,
            },
        )
        .await?;
    let first_assessment = report
        .assessment_ids
        .first()
        .copied()
        .ok_or_else(|| msg("evaluator returned no assessment"))?;
    let mut cases = Vec::new();
    for assessment in &report.assessment_ids {
        let evidence = ctx.assessment_evidence(*assessment)?;
        let EvoSkillEvidence::Evaluation {
            cases: mut case_records,
            ..
        } = evidence
        else {
            return Err(msg("expected evaluation evidence"));
        };
        cases.append(&mut case_records);
    }
    let average_score = average_case_score(&cases);
    Ok(EvaluationOutcome {
        assessment: first_assessment,
        average_score,
        cost: report.cost,
        evidence: EvaluationEvidence { cases },
    })
}

fn observe_frontier(
    ctx: &mut RunContext<'_, EvoSkillProblem>,
    population: &mut TopKFrontier,
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

fn evoskill_frontier() -> TopKFrontier {
    TopKFrontier::new(
        NonZeroUsize::new(EVOSKILL_FRONTIER_SIZE).expect("EvoSkill frontier size is non-zero"),
    )
}

fn evoskill_parent_selector() -> TopKParentSelector {
    TopKParentSelector::best()
}

fn select_frontier_parent(
    parent_selector: &mut TopKParentSelector,
    population: &TopKFrontier,
) -> Result<CandidateId> {
    parent_selector
        .select(population)
        .ok_or_else(|| msg("EvoSkill frontier is empty; no parent can be selected"))
}

#[derive(Clone)]
struct EvoSkillPresenter {
    developer_instructions: String,
}

impl AgentCasePresenter<EvoSkillProblem> for EvoSkillPresenter {
    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([42; 32])
    }

    async fn present<'a>(
        &'a self,
        input: AgentCasePresentationInput<'a, EvoSkillProblem>,
        workspace: &'a mut leaven_workspace::WorkspaceView<'_>,
        ctx: leaven_engine::MaterializeContext<'a, EvoSkillProblem>,
    ) -> std::result::Result<Metered<AgentCasePresentation>, AgenticAdapterError> {
        let case = presented_case(input.case)?;
        SkillBankMaterializer::new(
            SkillWorkspaceLayout::new(".agents/skills")
                .map_err(|error| AgenticAdapterError::Input(error.to_string()))?,
        )
        .materialize_into(
            &SkillBankProposalInput::new(input.candidate_id),
            workspace,
            ctx,
        )
        .await
        .map_err(|error| AgenticAdapterError::Input(error.to_string()))?;
        write_json(workspace, "task/case.json", &case)
            .map_err(|error| AgenticAdapterError::Input(error.to_string()))?;
        let mut instructions = AgentInstructions::task(executor_task(&case));
        instructions.system = Some(self.developer_instructions.clone());
        let mut request = AgentRunRequest::new(instructions, OutputContract::FinalMessage);
        request.limits = AgentLimits {
            timeout: Some(Duration::from_secs(240)),
            ..AgentLimits::default()
        };
        Ok(Metered::new(
            AgentCasePresentation {
                request,
                materialized_refs: vec![
                    WorkspacePath::new("task/case.json")
                        .map_err(|error| AgenticAdapterError::Input(error.to_string()))?,
                ],
            },
            Cost::zero(),
        ))
    }
}

#[derive(Clone)]
struct EvoSkillScorer {
    developer_instructions: String,
}

impl AgentCaseScorer<EvoSkillProblem> for EvoSkillScorer {
    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([43; 32])
    }

    async fn score<'a>(
        &'a self,
        input: AgentCaseScoreInput<'a, EvoSkillProblem>,
        _workspace: &'a leaven_workspace::WorkspaceView<'_>,
    ) -> std::result::Result<Metered<EvoSkillEvidence>, AgenticAdapterError> {
        let case = presented_case(input.case)?;
        let expected = expected_answer(input.case)?;
        let answer: AgentAnswer = final_json(input.session)
            .map_err(|error| AgenticAdapterError::Input(error.to_string()))?;
        let has_relevant_skill = input
            .graph
            .artifact(input.candidate_id)
            .is_some_and(|bank| !bank.is_empty());
        let score = skill_gated_score(has_relevant_skill, &expected, &answer.final_answer);
        Ok(Metered::new(
            EvoSkillEvidence::Evaluation {
                candidate: input.candidate_id,
                split: "case".to_owned(),
                average_score: score,
                cases: vec![CaseExecution {
                    case_id: case.id,
                    question: case.question,
                    expected_answer: expected,
                    predicted_answer: answer.final_answer,
                    score,
                    passed: score >= 0.8,
                    developer_instructions: self.developer_instructions.clone(),
                    session: input.session.clone(),
                }],
            },
            Cost::zero(),
        ))
    }
}

#[derive(serde::Deserialize)]
struct AgentAnswer {
    final_answer: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct PresentedCase {
    id: String,
    question: String,
    source: String,
}

fn presented_case(case: &AgentCase) -> std::result::Result<PresentedCase, AgenticAdapterError> {
    let CaseInput::Structured(value) = &case.input else {
        return Err(AgenticAdapterError::Input(
            "EvoSkill presenter requires structured case input".to_owned(),
        ));
    };
    serde_json::from_value(value.clone()).map_err(|error| {
        AgenticAdapterError::Input(format!("invalid EvoSkill case input: {error}"))
    })
}

fn expected_answer(case: &AgentCase) -> std::result::Result<String, AgenticAdapterError> {
    let CaseTarget::Text(answer) = &case.target else {
        return Err(AgenticAdapterError::Input(
            "EvoSkill scorer requires text targets".to_owned(),
        ));
    };
    Ok(answer.clone())
}

fn synthetic_answer_for(case: &AgentCase) -> Result<String> {
    Ok(format!(
        "{{\"final_answer\":{}}}",
        serde_json::to_string(&expected_answer(case).map_err(|error| msg(error.to_string()))?)?
    ))
}

fn skill_gated_score(has_relevant_skill: bool, expected: &str, predicted: &str) -> f64 {
    if has_relevant_skill {
        multi_tolerance_score(expected, predicted)
    } else {
        0.0
    }
}

fn executor_task(case: &PresentedCase) -> String {
    format!(
        "Answer this case.\n\nQuestion:\n{}\n\nSource:\n{}\n\nThis fixture is skill-gated. Inspect `.agents/skills` mentally from the task context and use a relevant skill when one exists. If no relevant skill exists for the specialized reusable conversion procedure, final_answer must be exactly `NOT_ATTEMPTED`; do not answer from prior knowledge or source arithmetic without a relevant mounted skill.\n\nDo not call tools. Reply with JSON only: {{\"final_answer\":\"...\",\"reasoning\":\"...\"}}.",
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
        let mut instructions = AgentInstructions::task(proposer_task(failures, bank)?);
        instructions.system = Some(developer_instructions.clone());
        let mut request = AgentRunRequest::new(instructions, OutputContract::FinalMessage);
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
        let session = session.value;
        let evidence = evidence_store.put(EvoSkillEvidence::AgentRoleSession {
            role: AgentRole::Proposer,
            developer_instructions: developer_instructions.clone(),
            session,
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
    let recording_runtime = RecordingBuilderRuntime {
        inner: &runtime,
        evidence_store,
        developer_instructions: &developer_instructions,
    };
    let proposer = RepairingAgenticProposer::new(
        RepairingAgenticProposerConfig::new(
            ProposerId::new_const("evoskill/skill-builder"),
            NonZeroUsize::new(2).expect("two repair attempts is non-zero"),
        ),
        factory.clone(),
        recording_runtime,
        SkillBuilderMaterializer,
        SkillBuilderRenderer {
            developer_instructions: developer_instructions.clone(),
        },
        SkillBuilderRepairPrompt {
            developer_instructions: developer_instructions.clone(),
        },
        SkillBuilderParser {
            inner: SkillBankWorkspaceProposalParser::new(SkillWorkspaceLayout::new(
                ".agents/skills",
            )?),
        },
    );
    let mut request = leaven_agentic::AgenticRunInput::new(
        SkillBuilderInput {
            parent,
            proposal: proposal.clone(),
            proposer_evidence: proposer_evidence.clone(),
        },
        OutputContract::FinalMessage,
    );
    request.limits = AgentLimits {
        timeout: Some(Duration::from_secs(240)),
        ..AgentLimits::default()
    };

    let recorded = ctx.propose(&proposer, request).await?;
    let proposal_id = *recorded
        .proposal_ids
        .first()
        .ok_or_else(|| msg("skill builder returned no proposal"))?;
    let change = ctx
        .graph()
        .proposal(proposal_id)
        .and_then(|proposal| match proposal.effect() {
            leaven_core::ProposalEffect::Change { change, .. } => Some(change.clone()),
            leaven_core::ProposalEffect::Create { .. } => None,
        })
        .ok_or_else(|| msg("builder parser returned no skill-bank change"))?;
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

#[derive(Clone, Debug)]
struct SkillBuilderInput {
    parent: CandidateId,
    proposal: SkillProposal,
    proposer_evidence: EvidenceRef,
}

struct SkillBuilderMaterializer;

impl Materializer<EvoSkillProblem, SkillBuilderInput> for SkillBuilderMaterializer {
    async fn materialize_into(
        &self,
        value: &SkillBuilderInput,
        workspace: &mut leaven_workspace::WorkspaceView<'_>,
        ctx: MaterializeContext<'_, EvoSkillProblem>,
    ) -> std::result::Result<Metered<MaterializationReport>, MaterializeError> {
        prepare_builder_workspace(workspace, ctx, value.parent, &value.proposal)
            .await
            .map_err(|error| MaterializeError::Message(error.to_string()))?;
        Ok(Metered::new(MaterializationReport::default(), Cost::zero()))
    }
}

struct SkillBuilderRenderer {
    developer_instructions: String,
}

impl Renderer<EvoSkillProblem, SkillBuilderInput, AgentPromptTarget> for SkillBuilderRenderer {
    type View = AgentInstructions;

    async fn render(
        &self,
        value: &SkillBuilderInput,
        _target: AgentPromptTarget,
        _ctx: RenderContext<'_, EvoSkillProblem>,
    ) -> std::result::Result<Metered<Self::View>, RenderError> {
        let mut instructions = AgentInstructions::task(
            builder_task(&value.proposal, 1, None)
                .map_err(|error| RenderError::Message(error.to_string()))?,
        );
        instructions.system = Some(self.developer_instructions.clone());
        Ok(Metered::new(instructions, Cost::zero()))
    }
}

struct SkillBuilderRepairPrompt {
    developer_instructions: String,
}

impl ProposalRepairPromptBuilder<SkillBuilderInput> for SkillBuilderRepairPrompt {
    fn build_repair(
        &self,
        input: &SkillBuilderInput,
        feedback: ProposalRepairFeedback<'_>,
    ) -> std::result::Result<AgentInstructions, AgenticRepairError> {
        let mut instructions = AgentInstructions::task(
            builder_task(
                &input.proposal,
                feedback.failed_attempt.get() + 1,
                Some(&feedback.parse_error.to_string()),
            )
            .map_err(|error| AgenticRepairError::Prompt(error.to_string()))?,
        );
        instructions.system = Some(self.developer_instructions.clone());
        Ok(instructions)
    }
}

struct RecordingBuilderRuntime<'a> {
    inner: &'a LiveCodexRuntime,
    evidence_store: &'a FileEvidenceStore<EvoSkillEvidence>,
    developer_instructions: &'a str,
}

impl AgentRuntime for RecordingBuilderRuntime<'_> {
    fn id(&self) -> leaven_kernel::AgentRuntimeId {
        self.inner.id()
    }

    fn fingerprint(&self) -> Fingerprint {
        self.inner.fingerprint()
    }

    fn capabilities(&self) -> leaven_agent::AgentRuntimeCapabilities {
        self.inner.capabilities()
    }

    async fn run_session(
        &self,
        workspace: &mut leaven_workspace::WorkspaceView<'_>,
        request: AgentRunRequest,
        ctx: AgentRunContext<'_>,
    ) -> std::result::Result<Metered<leaven_agent::AgentSession>, leaven_agent::AgentRuntimeError>
    {
        let session = self.inner.run_session(workspace, request, ctx).await?;
        self.evidence_store
            .put(EvoSkillEvidence::AgentRoleSession {
                role: AgentRole::SkillBuilder,
                developer_instructions: self.developer_instructions.to_owned(),
                session: session.value.clone(),
            })
            .map_err(|error| {
                leaven_agent::AgentRuntimeError::with_source(
                    "failed to store skill-builder session evidence",
                    error,
                )
            })?;
        Ok(session)
    }
}

struct SkillBuilderParser {
    inner: SkillBankWorkspaceProposalParser,
}

impl ProposalParser<EvoSkillProblem, SkillBuilderInput> for SkillBuilderParser {
    async fn parse_proposals(
        &self,
        workspace: &mut leaven_workspace::WorkspaceView<'_>,
        session: &leaven_agent::AgentSession,
        input: &SkillBuilderInput,
        graph: leaven_engine::RunGraphView<'_, EvoSkillProblem>,
    ) -> std::result::Result<Metered<leaven_core::ProposalBatch<EvoSkillProblem>>, AgenticParseError>
    {
        let generated: GeneratedSkill =
            final_json(session).map_err(|error| AgenticParseError::Message(error.to_string()))?;
        write_generated_skill(workspace, &generated)
            .and_then(|()| write_json(workspace, "output/skill-builder.json", &generated))
            .map_err(|error| AgenticParseError::Message(error.to_string()))?;
        let mut parsed = self
            .inner
            .parse_proposals(
                workspace,
                session,
                &SkillBankProposalInput::new(input.parent),
                graph,
            )
            .await?;
        annotate_builder_proposals(&mut parsed.value.proposals, input);
        parsed.value.semantics = ProposalBatchSemantics::Alternatives;
        Ok(parsed)
    }
}

fn annotate_builder_proposals(
    proposals: &mut [leaven_core::Proposal<EvoSkillProblem>],
    input: &SkillBuilderInput,
) {
    for item in proposals {
        item.annotations = EvoSkillProposalAnnotations {
            proposal: Some(input.proposal.clone()),
            proposer_evidence: Some(input.proposer_evidence.clone()),
        };
        let _proposal_payload_present = item.annotations.proposal.is_some();
        item.provenance = item
            .provenance
            .clone()
            .informed_by([InfoRef::External(ExternalRef {
                kind: "evidence".to_owned(),
                id: format!(
                    "{}:{}",
                    input.proposer_evidence.store, input.proposer_evidence.key
                ),
            })]);
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
    let informed_by: Vec<InfoRef> = proposer_evidence
        .iter()
        .map(|reference| {
            InfoRef::External(ExternalRef {
                kind: "evidence".to_owned(),
                id: format!("{}:{}", reference.store, reference.key),
            })
        })
        .collect();
    let item = leaven_core::Proposal::<EvoSkillProblem>::mutate(parent, change.clone())
        .annotations(EvoSkillProposalAnnotations {
            proposal,
            proposer_evidence,
        })
        .informed_by(informed_by)
        .build();
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

#[cfg(test)]
mod tests {
    use leaven_evidence::ScalarEvidence;
    use leaven_kernel::{AssessmentId, CandidateId, RunId};
    use leaven_population::{TopKParentSelectionPolicy, TopKParentSelector};

    use super::{EvoSkillCheckpoint, ResumeState, evoskill_frontier, skill_gated_score};

    #[test]
    fn skill_gated_score_rejects_model_prior_without_skill() {
        assert_float_eq(skill_gated_score(false, "99.5", "99.5"), 0.0);
        assert_float_eq(skill_gated_score(false, "99.5", "NOT_ATTEMPTED"), 0.0);
        assert_float_eq(skill_gated_score(true, "99.5", "99.5"), 1.0);
    }

    #[test]
    fn resume_state_restores_frontier_parent_and_selector_cursor() {
        let mut frontier = evoskill_frontier();
        let parent = CandidateId::new();
        frontier.observe(
            parent,
            AssessmentId::new(),
            ScalarEvidence::new(0.7).unwrap(),
        );
        let parent_selector =
            TopKParentSelector::with_cursor(TopKParentSelectionPolicy::RoundRobin, 1);

        let resume = ResumeState::from_checkpoint(Some(EvoSkillCheckpoint::FailuresCollected {
            run_id: RunId::default(),
            cases: Vec::new(),
            seed_bank: Default::default(),
            baseline_score: 0.7,
            frontier: frontier.clone(),
            parent_selector: parent_selector.clone(),
            parent,
            failures: Vec::new(),
        }))
        .unwrap();

        let restored_frontier = resume.frontier.unwrap();
        assert_eq!(restored_frontier.members(), frontier.members());
        assert_eq!(resume.parent_selector, Some(parent_selector));
        assert_eq!(resume.parent, Some(parent));
    }

    fn assert_float_eq(left: f64, right: f64) {
        assert!((left - right).abs() < f64::EPSILON);
    }
}
