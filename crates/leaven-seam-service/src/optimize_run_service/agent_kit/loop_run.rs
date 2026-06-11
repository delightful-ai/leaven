//! Git-backed `AgentKit` optimization loop.
//!
//! This is the parallel kit path to the prompt loop in the parent module. It
//! seeds a run-scoped Git repository from the lowered kit file map, runs the
//! real GEPA loop over a `GitProgramArtifact` with the agentic reflection path,
//! dispatches per-case runner and scorer stages to the configured command
//! worker (projecting each candidate revision into a `candidate_agent_kit` flat
//! file map the worker reads), and returns the durable `Optimized` result plus
//! the seed stores so the result projection can read frontier revisions back to
//! flat wire parts.
//!
//! It is a parallel path rather than a genericization of the prompt loop because
//! the candidate projection, edit surface, reflector, and result readback are
//! all artifact-specific; the shared worker transport, budget axes, and
//! profile/knob lowering are reused from the parent module.

use std::sync::{Arc, Mutex};

use leaven_agent::AgentRuntime;
use leaven_agentic::AgenticProposerConfig;
use leaven_agentic_git::{
    GitProgramMaterializer, GitProgramReadback, GitProgramSeed, GitProgramStores,
    build_program_seed, read_revision_files,
};
use leaven_artifact_git::{GitPath, GitProgramArtifact, RepoKey};
use leaven_gepa::{Gepa, GepaProfile, GepaReport};
use leaven_gepa_agentic_git::{GepaGitProgramAgenticReflector, GitProgramPathSurface};
use leaven_kernel::{ProposerId, RunId};
use leaven_run::Optimized;
use leaven_workspace_local::LocalWorkspaceFactory;
use serde_json::{Value, json};

use super::super::error::OptimizeRunHostError;
use super::super::lowering::LoweredRequest;
use super::super::worker::{WorkerDispatch, run_runner_stage, run_scorer_stage};
use super::instrumentation::{KitArtifactSnapshot, KitArtifacts, kit_repo_key};
use super::projection::{kit_parts_from_files, kit_wire_artifact};

/// Workspace layout subpath the kit repo materializes under.
const KIT_LAYOUT: &str = "repos/agent_kit";

/// The result of a kit optimization loop: the durable optimized result, its
/// GEPA report, and the seed stores (for reading frontier revisions back).
pub(in crate::optimize_run_service) struct KitLoopOutput {
    pub(in crate::optimize_run_service) optimized: Optimized<GitProgramArtifact>,
    pub(in crate::optimize_run_service) report: GepaReport,
    pub(in crate::optimize_run_service) seed: GitProgramSeed,
    pub(in crate::optimize_run_service) artifacts: KitArtifacts,
}

/// Inputs the kit loop composes into the GEPA run.
pub(in crate::optimize_run_service) struct KitLoopInputs<'a, Runtime> {
    pub(in crate::optimize_run_service) lowered: &'a LoweredRequest,
    pub(in crate::optimize_run_service) kit_files: std::collections::BTreeMap<GitPath, Vec<u8>>,
    pub(in crate::optimize_run_service) dispatch: WorkerDispatch,
    pub(in crate::optimize_run_service) runtime: Runtime,
    pub(in crate::optimize_run_service) run_dir: std::path::PathBuf,
}

/// Runs the Git-backed `AgentKit` optimization loop with the supplied agent
/// runtime.
///
/// The runtime is a type parameter so the production path can pass the
/// configured `CodexCliRuntime` and deterministic tests can pass a scripted
/// `FakeAgentRuntime` through test-support configuration; neither is a public
/// scaffold.
pub(in crate::optimize_run_service) fn run_kit_loop<Runtime>(
    inputs: KitLoopInputs<'_, Runtime>,
) -> Result<KitLoopOutput, OptimizeRunHostError>
where
    Runtime: AgentRuntime + Clone + 'static,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            OptimizeRunHostError::optimization(format!(
                "optimize.run kit loop runtime build failed: {error}"
            ))
        })?;
    runtime.block_on(run_kit_loop_async(inputs))
}

#[allow(clippy::large_futures)]
async fn run_kit_loop_async<Runtime>(
    inputs: KitLoopInputs<'_, Runtime>,
) -> Result<KitLoopOutput, OptimizeRunHostError>
where
    Runtime: AgentRuntime + Clone + 'static,
{
    let KitLoopInputs {
        lowered,
        kit_files,
        dispatch,
        runtime: agent_runtime,
        run_dir,
    } = inputs;

    let seed = seed_kit_repo(&run_dir, &kit_files)?;

    let report_slot: Arc<Mutex<Option<GepaReport>>> = Arc::new(Mutex::new(None));
    let report_sink = report_slot.clone();
    let artifacts = KitArtifacts::new();
    let reflector = build_reflector(&seed, agent_runtime);

    let mut gepa = Gepa::builder()
        .surface(GitProgramPathSurface)
        .reflector(reflector)
        .with_profile(GepaProfile::OptimizeAnything);
    if let Some(minibatch) = lowered.train_minibatch_size {
        gepa = gepa.train_minibatch_size(minibatch);
    }
    let gepa = gepa
        .reflective_dataset(leaven_gepa::GepaReflectiveDataset::with_case_input(
            super::super::project_case_input,
        ))
        .on_report(move |report| {
            *report_sink
                .lock()
                .expect("optimize.run kit report slot lock poisoned") = Some(report.clone());
        });
    let gepa = match lowered.max_candidates {
        Some(cap) => gepa.max_candidates(cap),
        None => gepa,
    };

    let budget = super::super::build_budget(lowered.max_metric_calls, lowered.max_cost_usd_micro)?;

    let stores_for_runner = seed.stores().clone();
    let repo_for_runner = seed.repo().clone();
    let runner_dispatch = dispatch.clone();
    let scorer_dispatch = dispatch;

    let optimized = leaven_run::optimize(seed.artifact().clone())
        .train(lowered.train.clone())
        .validation(lowered.validation.clone())
        .test(lowered.test.clone())
        .runner(
            move |artifact: GitProgramArtifact, case: leaven_run::RunCase<Value>| {
                let dispatch = runner_dispatch.clone();
                let stores = stores_for_runner.clone();
                let repo = repo_for_runner.clone();
                // Project the current candidate revision into a flat
                // `candidate_agent_kit` wire artifact the worker reads.
                let candidate_payload = candidate_kit_payload(&artifact, &stores, &repo);
                async move {
                    let candidate_payload = candidate_payload
                        .map_err(|error| leaven_run::RunError::new(error.to_string()))?;
                    run_runner_stage(dispatch, candidate_payload, case).await
                }
            },
        )
        .score(
            move |ctx: leaven_run::ScoreContext<GitProgramArtifact, Value, Value, Value>| {
                let dispatch = scorer_dispatch.clone();
                async move { run_scorer_stage(dispatch, ctx).await }
            },
        )
        .runner_fingerprint(super::super::worker_runtime_fingerprint(
            "optimize_run.kit.runner",
        ))
        .scorer_fingerprint(super::super::worker_runtime_fingerprint(
            "optimize_run.kit.scorer",
        ))
        .evaluation_parallelism(super::super::sequential())
        .on_event(KitArtifactSnapshot::new(artifacts.clone()))
        .using(gepa)
        .budget(budget)
        .run_id(RunId::new())
        .run_dir(run_dir)
        .run()
        .await
        .map_err(|error| OptimizeRunHostError::optimization(super::super::error_chain(&error)))?;

    let report = resolve_report(&report_slot, &optimized)?;

    Ok(KitLoopOutput {
        optimized,
        report,
        seed,
        artifacts,
    })
}

/// Builds the Git-program agentic reflector over the seed's stores.
///
/// The reflector materializes the parent kit revision into a disposable agent
/// workspace, runs the supplied agent runtime, and reads a typed
/// `GitProgramChange` back; the part id is the program `RepoKey` because the
/// surface exposes one part per repo.
fn build_reflector<Runtime>(
    seed: &GitProgramSeed,
    agent_runtime: Runtime,
) -> GepaGitProgramAgenticReflector<LocalWorkspaceFactory, Runtime, RepoKey>
where
    Runtime: AgentRuntime,
{
    GepaGitProgramAgenticReflector::new(
        AgenticProposerConfig::new(ProposerId::from("gepa/agent-kit-agentic")),
        LocalWorkspaceFactory::temp(),
        agent_runtime,
        GitProgramMaterializer::new(seed.stores().clone()),
        GitProgramReadback::new(seed.stores().clone()),
    )
}

/// Resolves the GEPA report from the sink, falling back to the result facade.
fn resolve_report(
    report_slot: &Arc<Mutex<Option<GepaReport>>>,
    optimized: &Optimized<GitProgramArtifact>,
) -> Result<GepaReport, OptimizeRunHostError> {
    report_slot
        .lock()
        .expect("optimize.run kit report slot lock poisoned")
        .clone()
        .or_else(|| optimized.optimizer_report::<GepaReport>().cloned())
        .ok_or_else(|| {
            OptimizeRunHostError::projection("GEPA kit loop produced no optimizer report")
        })
}

/// Seeds a run-scoped Git repository from the lowered kit file map.
///
/// The store lives under the run dir so it is cleaned up with the run, and the
/// seed commit is deterministic in the kit content.
fn seed_kit_repo(
    run_dir: &std::path::Path,
    kit_files: &std::collections::BTreeMap<GitPath, Vec<u8>>,
) -> Result<GitProgramSeed, OptimizeRunHostError> {
    let store_root = run_dir.join("kit-stores");
    std::fs::create_dir_all(&store_root).map_err(|error| {
        OptimizeRunHostError::lowering(format!("failed to create kit store root: {error}"))
    })?;
    let layout = GitPath::new(KIT_LAYOUT)
        .map_err(|error| OptimizeRunHostError::lowering(format!("invalid kit layout: {error}")))?;
    build_program_seed(kit_repo_key(), layout, &store_root, kit_files)
        .map_err(|error| OptimizeRunHostError::lowering(error.to_string()))
}

/// Projects a candidate `GitProgramArtifact` revision into the worker payload's
/// `candidate_agent_kit` flat wire parts.
fn candidate_kit_payload(
    artifact: &GitProgramArtifact,
    stores: &GitProgramStores,
    repo: &RepoKey,
) -> Result<Value, OptimizeRunHostError> {
    let revision = artifact
        .repo(repo)
        .ok_or_else(|| {
            OptimizeRunHostError::optimization(format!(
                "kit candidate is missing repo `{repo}` for projection"
            ))
        })?
        .revision()
        .clone();
    let files = read_revision_files(stores, repo, &revision)
        .map_err(|error| OptimizeRunHostError::optimization(error.to_string()))?;
    let parts = kit_parts_from_files(&files)?;
    Ok(json!({ "candidate_agent_kit": kit_wire_artifact(&parts) }))
}
