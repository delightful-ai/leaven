//! GEPA-over-seam host: executes `leaven/optimize.run` through the real
//! `leaven-gepa` optimizer loop.
//!
//! This module is configured composition, not optimizer strategy. It lowers the
//! locked `leaven/optimize.run` request into the `leaven-run` builder, drives
//! the real GEPA loop (frontier, reflection, admission gate), dispatches every
//! per-case rollout (runner stage) and scoring (scorer stage with typed reward
//! vectors) to the configured worker subprocess over `leaven/stage.run`, and
//! projects the durable `Optimized` result plus GEPA frontier into the locked
//! `leaven.optimize_run.v1` result document. GEPA search policy stays in
//! `leaven-gepa`; wire law stays in `leaven-public-seam`; graph mutation stays
//! behind `RunContext`. This module owns lowering, worker composition, and
//! projection only.

mod agent_kit;
mod error;
mod instrumentation;
mod lowering;
mod problem;
mod projection;
mod sanitize;
mod worker;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use leaven_gepa::{Gepa, GepaProfile, GepaReport};
use leaven_kernel::{Budget, RunId};
use leaven_public_seam::LockedMethod;
use leaven_run::Optimized;
use leaven_seam_runtime::{SeamOptimizeRunRequest, SeamServiceError};
use serde_json::Value;

use self::error::OptimizeRunHostError;
use self::instrumentation::{
    CandidateArtifactSnapshot, CandidateArtifacts, GepaEventLog, write_run_instrumentation,
};
use self::lowering::{LoweredRequest, lower_request};
use self::problem::{SeamPromptArtifact, SeamPromptSurface};
use self::projection::{ProjectionInputs, project_result};
use self::worker::{LmHandler, WorkerDispatch, run_runner_stage, run_scorer_stage};
use crate::service::ConfiguredSeamService;

/// Executes a validated `leaven/optimize.run` dispatch through the GEPA host.
pub fn handle_optimize_run(
    service: &ConfiguredSeamService,
    request: &SeamOptimizeRunRequest<'_>,
) -> Result<Value, SeamServiceError> {
    execute(service, request).map_err(SeamServiceError::from)
}

fn execute(
    service: &ConfiguredSeamService,
    request: &SeamOptimizeRunRequest<'_>,
) -> Result<Value, OptimizeRunHostError> {
    let argv = service
        .stage_worker_argv()
        .ok_or_else(|| {
            OptimizeRunHostError::worker_unavailable(
                "configure SeamStageConfig::CommandRunner argv to dispatch optimize.run rollouts",
            )
        })?
        .clone();

    let lowered = lower_request(request.document())?;
    let run_dir_root = service.runs_root_for(&lowered.run_id);

    // The GEPA loop runs under a tokio runtime (so OpenAI-backed reflection has
    // a reactor), but a worker's nested `leaven/lm.complete` callback executes
    // through `execute_plan_method`, which builds its own current-thread tokio
    // runtime per call. Building a runtime inside an active runtime on the same
    // thread panics ("Cannot start a runtime from within a runtime"). The worker
    // dispatch is synchronous and polled inline on the loop thread, so the
    // callback runs the provider call on a scoped helper thread with no tokio
    // context, keeping the nested provider runtime off the loop's runtime thread.
    let lm_service = service.clone();
    let lm_handler: LmHandler = Arc::new(move |params: &Value| {
        std::thread::scope(|scope| {
            scope
                .spawn(|| lm_service.execute_plan_method(LockedMethod::LmComplete, params))
                .join()
                .unwrap_or_else(|_| {
                    Err(leaven_public_seam::PublicSeamError::InvalidPlan {
                        message: "optimize.run LM callback worker thread panicked".to_owned(),
                    })
                })
        })
    });

    let dispatch = WorkerDispatch::new(
        argv,
        &lowered.run_id,
        lowered.capability_fingerprint.clone(),
        lowered.cases_by_id.clone(),
        lm_handler,
    );

    match &lowered.objective {
        lowering::LoweredObjective::Prompt { .. } => {
            execute_prompt(service, &lowered, dispatch, &run_dir_root)
        }
        lowering::LoweredObjective::AgentKit { .. } => {
            agent_kit::execute_agent_kit(service, &lowered, dispatch, &run_dir_root)
        }
    }
}

/// Runs the prompt-template LM-reflection optimization path and projects its
/// result.
fn execute_prompt(
    service: &ConfiguredSeamService,
    lowered: &LoweredRequest,
    dispatch: WorkerDispatch,
    run_dir_root: &std::path::Path,
) -> Result<Value, OptimizeRunHostError> {
    let lowering::LoweredObjective::Prompt {
        seed,
        reflection_model,
    } = &lowered.objective
    else {
        return Err(OptimizeRunHostError::lowering(
            "execute_prompt requires a prompt objective",
        ));
    };

    let reflection_lm = service
        .configured_lm_runtime()
        .map_err(|error| OptimizeRunHostError::lowering(error.to_string()))?;

    let events = GepaEventLog::new();
    let artifacts = CandidateArtifacts::new();
    let report_slot: Arc<std::sync::Mutex<Option<GepaReport>>> =
        Arc::new(std::sync::Mutex::new(None));

    let optimized = run_gepa_loop(GepaLoopInputs {
        lowered,
        seed: seed.clone(),
        reflection_model: reflection_model.clone(),
        dispatch,
        reflection_lm,
        events: events.clone(),
        artifacts: artifacts.clone(),
        report_slot: report_slot.clone(),
        run_dir: run_dir_root.to_path_buf(),
    })?;

    let report = report_slot
        .lock()
        .expect("optimize.run report slot lock poisoned")
        .clone()
        .or_else(|| optimized.optimizer_report::<GepaReport>().cloned())
        .ok_or_else(|| {
            OptimizeRunHostError::projection("GEPA loop produced no optimizer report")
        })?;

    let event_summaries = events.snapshot();
    write_run_instrumentation(run_dir_root, &event_summaries, Some(&report));

    let revision = revision_label(&optimized, &lowered.run_id);
    let result = project_result(&ProjectionInputs {
        run_id: &lowered.run_id,
        seed_schema: &lowered.seed_schema,
        optimized: &optimized,
        report: &report,
        artifacts: &artifacts,
        revision: &revision,
    })?;

    Ok(result)
}

struct GepaLoopInputs<'a> {
    lowered: &'a LoweredRequest,
    seed: SeamPromptArtifact,
    reflection_model: String,
    dispatch: WorkerDispatch,
    reflection_lm: crate::lm::ConfiguredLmRuntime,
    events: GepaEventLog,
    artifacts: CandidateArtifacts,
    report_slot: Arc<std::sync::Mutex<Option<GepaReport>>>,
    run_dir: std::path::PathBuf,
}

fn run_gepa_loop(
    inputs: GepaLoopInputs<'_>,
) -> Result<Optimized<SeamPromptArtifact>, OptimizeRunHostError> {
    // GEPA's reflector awaits `Lm::complete` directly inside this future. A live
    // provider (`SeamLmConfig::OpenAi`) uses reqwest plus tokio timers and
    // semaphores, so the loop needs a real tokio reactor on the polling thread;
    // `futures::executor::block_on` would panic ("there is no reactor running").
    // A current-thread runtime is sufficient: the loop is sequential
    // (`evaluation_parallelism(1)`) and worker callbacks run their own provider
    // runtime on a scoped helper thread (see `lm_handler`), so they never nest
    // on this runtime's thread.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            OptimizeRunHostError::optimization(format!(
                "optimize.run loop runtime build failed: {error}"
            ))
        })?;
    runtime.block_on(run_gepa_loop_async(inputs))
}

// The optimize builder future is large because it owns the whole run
// configuration, but this is a single top-level orchestration future driven by
// one `block_on`, not a future stored or polled in a hot loop, so heap-boxing it
// is sufficient.
#[allow(clippy::large_futures)]
async fn run_gepa_loop_async(
    inputs: GepaLoopInputs<'_>,
) -> Result<Optimized<SeamPromptArtifact>, OptimizeRunHostError> {
    let GepaLoopInputs {
        lowered,
        seed,
        reflection_model,
        dispatch,
        reflection_lm,
        events,
        artifacts,
        report_slot,
        run_dir,
    } = inputs;

    let train = lowered.train.clone();
    let validation = lowered.validation.clone();
    let test = lowered.test.clone();
    let max_metric_calls = lowered.max_metric_calls;
    let max_candidates = lowered.max_candidates;
    let train_minibatch_size = lowered.train_minibatch_size;
    let max_cost_usd_micro = lowered.max_cost_usd_micro;

    let scorer_dispatch = dispatch.clone();
    let runner_dispatch = dispatch;

    let event_sink = events;
    let report_sink = report_slot;

    // The OptimizeAnything profile fixes the reference per-case Pareto frontier
    // and screening minibatch. `population_size` lowers into the candidate-pool
    // cap (a stop condition over the seed plus loop-authored children) and
    // `minibatch_size` lowers into the train screening minibatch override, which
    // survives the profile because the builder records it explicitly.
    let mut gepa = Gepa::reflect_with_lm(reflection_lm, reflection_model)
        .surface(SeamPromptSurface)
        .build()
        .with_profile(GepaProfile::OptimizeAnything);
    if let Some(minibatch) = train_minibatch_size {
        gepa = gepa.train_minibatch_size(minibatch);
    }
    let gepa = gepa
        // Reflection sees a target-free string projection of each case input;
        // scorer feedback flows from the stored assessment evidence, so
        // reflection quality keeps the per-case feedback text the scorer
        // produced.
        .reflective_dataset(leaven_gepa::GepaReflectiveDataset::with_case_input(
            project_case_input,
        ))
        .on_event(move |event| event_sink.record(event))
        .on_report(move |report| {
            *report_sink
                .lock()
                .expect("optimize.run report slot lock poisoned") = Some(report.clone());
        });
    let gepa = match max_candidates {
        Some(cap) => gepa.max_candidates(cap),
        None => gepa,
    };

    // The metric-call cap always bounds the loop. An optional `usd_micro` cost
    // ceiling adds a second budget axis: the worker reports metered provider
    // spend on the `usd_micro` cost axis, and the engine budget ledger refuses a
    // charge that would exceed this axis limit, stopping the loop on real spend.
    let budget = build_budget(max_metric_calls, max_cost_usd_micro)?;

    let optimized = leaven_run::optimize(seed)
        .train(train)
        .validation(validation)
        .test(test)
        .runner(
            move |artifact: SeamPromptArtifact, case: leaven_run::RunCase<Value>| {
                let dispatch = runner_dispatch.clone();
                // The prompt path projects its candidate material as the single
                // `candidate_template` key the worker reads.
                let candidate_payload = serde_json::json!({
                    "candidate_template": artifact.template(),
                });
                async move { run_runner_stage(dispatch, candidate_payload, case).await }
            },
        )
        .score(
            move |ctx: leaven_run::ScoreContext<SeamPromptArtifact, Value, Value, Value>| {
                let dispatch = scorer_dispatch.clone();
                async move { run_scorer_stage(dispatch, ctx).await }
            },
        )
        .runner_fingerprint(worker_runtime_fingerprint("optimize_run.runner"))
        .scorer_fingerprint(worker_runtime_fingerprint("optimize_run.scorer"))
        .evaluation_parallelism(sequential())
        .on_event(CandidateArtifactSnapshot::new(artifacts))
        .using(gepa)
        .budget(budget)
        .run_id(RunId::new())
        .run_dir(run_dir)
        .run()
        .await
        .map_err(|error| OptimizeRunHostError::optimization(error_chain(&error)))?;

    Ok(optimized)
}

pub(in crate::optimize_run_service) fn error_chain(
    error: &(dyn std::error::Error + 'static),
) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

pub(in crate::optimize_run_service) fn sequential() -> std::num::NonZeroUsize {
    std::num::NonZeroUsize::new(1).expect("1 is non-zero")
}

/// Builds the run budget shared by both optimization paths.
///
/// The metric-call cap always bounds the loop. An optional `usd_micro` cost
/// ceiling adds a second budget axis: the worker reports metered provider spend
/// on the `usd_micro` cost axis, and the engine budget ledger refuses a charge
/// that would exceed this axis limit, stopping the loop on real spend.
pub(in crate::optimize_run_service) fn build_budget(
    max_metric_calls: u64,
    max_cost_usd_micro: Option<u64>,
) -> Result<Budget, OptimizeRunHostError> {
    let mut budget = Budget::metric_calls(max_metric_calls);
    if let Some(usd_micro) = max_cost_usd_micro {
        budget = budget
            .with_axis_limit("usd_micro", u64_to_f64(usd_micro))
            .map_err(|error| {
                OptimizeRunHostError::lowering(format!(
                    "optimizer.max_cost_usd_micro is not a valid budget amount: {error}"
                ))
            })?;
    }
    Ok(budget)
}

#[allow(clippy::cast_precision_loss)]
pub(in crate::optimize_run_service) fn u64_to_f64(value: u64) -> f64 {
    // A `usd_micro` ceiling is a non-negative integer counter; the f64 budget
    // axis tolerates rounding well beyond any realistic micro-dollar ceiling.
    value as f64
}

/// Stable durable behavior fingerprint for the worker-backed runner/scorer
/// closures. Worker closures are not introspectable, so durable runs require an
/// explicit declared fingerprint per role.
pub(in crate::optimize_run_service) fn worker_runtime_fingerprint(
    role: &str,
) -> leaven_kernel::Fingerprint {
    let mut builder = leaven_kernel::FingerprintBuilder::new();
    builder.update(b"leaven-seam-service.optimize_run.worker.v1");
    builder.update(role.as_bytes());
    builder.finish()
}

/// Target-free reflective projection of a case input.
///
/// GEPA's reflective dataset only reads this case input string; the target is
/// never projected here. Object inputs render each field on its own line so the
/// reflection prompt sees structured input without raw JSON noise.
pub(in crate::optimize_run_service) fn project_case_input(
    case: &leaven_eval::Case<Value, Value>,
) -> String {
    match &case.input {
        Value::String(text) => text.clone(),
        Value::Object(fields) => fields
            .iter()
            .map(|(key, value)| match value {
                Value::String(text) => format!("{key}: {text}"),
                other => format!("{key}: {other}"),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        other => other.to_string(),
    }
}

fn revision_label(optimized: &Optimized<SeamPromptArtifact>, run_id: &str) -> String {
    revision_label_for(optimized, run_id)
}

/// Kit-path revision label over a `GitProgramArtifact` result.
pub(in crate::optimize_run_service) fn revision_label_kit(
    optimized: &Optimized<leaven_artifact_git::GitProgramArtifact>,
    run_id: &str,
) -> String {
    revision_label_for(optimized, run_id)
}

fn revision_label_for<A>(optimized: &Optimized<A>, run_id: &str) -> String
where
    A: leaven_core::Artifact,
{
    if let leaven_run::RunStorage::Stored {
        latest_checkpoint: Some(checkpoint),
        ..
    } = &optimized.summary().storage
    {
        return format!("rev_{}", sanitize::sanitize_token(&checkpoint.to_string()));
    }
    format!("rev_{}_final", sanitize::sanitize_token(run_id))
}
