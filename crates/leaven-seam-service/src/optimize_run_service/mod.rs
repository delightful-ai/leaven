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

    let reflection_lm = service
        .configured_lm_runtime()
        .map_err(|error| OptimizeRunHostError::lowering(error.to_string()))?;

    let events = GepaEventLog::new();
    let artifacts = CandidateArtifacts::new();
    let report_slot: Arc<std::sync::Mutex<Option<GepaReport>>> =
        Arc::new(std::sync::Mutex::new(None));

    let optimized = run_gepa_loop(GepaLoopInputs {
        lowered: &lowered,
        dispatch,
        reflection_lm,
        events: events.clone(),
        artifacts: artifacts.clone(),
        report_slot: report_slot.clone(),
        run_dir: run_dir_root.clone(),
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
    write_run_instrumentation(&run_dir_root, &event_summaries, Some(&report));

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
        dispatch,
        reflection_lm,
        events,
        artifacts,
        report_slot,
        run_dir,
    } = inputs;

    let seed = lowered.seed.clone();
    let train = lowered.train.clone();
    let validation = lowered.validation.clone();
    let test = lowered.test.clone();
    let reflection_model = lowered.reflection_model.clone();
    let max_metric_calls = lowered.max_metric_calls;

    let scorer_dispatch = dispatch.clone();
    let runner_dispatch = dispatch;

    let event_sink = events;
    let report_sink = report_slot;

    let optimized = leaven_run::optimize(seed)
        .train(train)
        .validation(validation)
        .test(test)
        .runner(
            move |artifact: SeamPromptArtifact, case: leaven_run::RunCase<Value>| {
                let dispatch = runner_dispatch.clone();
                async move { run_runner_stage(dispatch, artifact, case).await }
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
        .using(
            Gepa::reflect_with_lm(reflection_lm, reflection_model)
                .surface(SeamPromptSurface)
                .build()
                .with_profile(GepaProfile::OptimizeAnything)
                // Reflection sees a target-free string projection of each
                // case input; scorer feedback flows from the stored
                // assessment evidence, so reflection quality keeps the
                // per-case feedback text the scorer produced.
                .reflective_dataset(leaven_gepa::GepaReflectiveDataset::with_case_input(
                    project_case_input,
                ))
                .on_event(move |event| event_sink.record(event))
                .on_report(move |report| {
                    *report_sink
                        .lock()
                        .expect("optimize.run report slot lock poisoned") = Some(report.clone());
                }),
        )
        .budget(Budget::metric_calls(max_metric_calls))
        .run_id(RunId::new())
        .run_dir(run_dir)
        .run()
        .await
        .map_err(|error| OptimizeRunHostError::optimization(error_chain(&error)))?;

    Ok(optimized)
}

fn error_chain(error: &(dyn std::error::Error + 'static)) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

fn sequential() -> std::num::NonZeroUsize {
    std::num::NonZeroUsize::new(1).expect("1 is non-zero")
}

/// Stable durable behavior fingerprint for the worker-backed runner/scorer
/// closures. Worker closures are not introspectable, so durable runs require an
/// explicit declared fingerprint per role.
fn worker_runtime_fingerprint(role: &str) -> leaven_kernel::Fingerprint {
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
fn project_case_input(case: &leaven_eval::Case<Value, Value>) -> String {
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
    if let leaven_run::RunStorage::Stored {
        latest_checkpoint: Some(checkpoint),
        ..
    } = &optimized.summary().storage
    {
        return format!("rev_{}", sanitize::sanitize_token(&checkpoint.to_string()));
    }
    format!("rev_{}_final", sanitize::sanitize_token(run_id))
}
