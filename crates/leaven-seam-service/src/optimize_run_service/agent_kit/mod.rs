//! Git-backed `AgentKit` optimization path for `leaven/optimize.run`.
//!
//! This submodule owns everything specific to the `agent_kit` artifact type: the
//! wire-record-to-Git-file projection, the kit candidate snapshot, the
//! Git-backed GEPA loop with agentic reflection, and the kit result projection.
//! The parent module owns the prompt path and the shared worker transport,
//! budget axes, and profile/knob lowering both paths reuse.

pub(super) mod instrumentation;
pub(super) mod loop_run;
pub(super) mod projection;
pub(super) mod result;

use serde_json::Value;

use super::error::OptimizeRunHostError;
use super::lowering::{LoweredObjective, LoweredRequest};
use super::worker::WorkerDispatch;
use crate::service::ConfiguredSeamService;
use loop_run::{KitLoopInputs, KitLoopOutput, run_kit_loop};
use result::{KitProjectionInputs, project_kit_result};

/// Runs the Git-backed `AgentKit` optimization path and projects its result.
///
/// The agent runtime is resolved from the configured service: deterministic
/// tests inject a scripted `FakeAgentRuntime` through test-support
/// configuration, and the live path uses the configured `CodexCliRuntime`. When
/// neither is configured the host refuses with the method-unavailable style,
/// because agentic reflection over an `agent_kit` seed cannot run without an
/// agent runtime.
pub(super) fn execute_agent_kit(
    service: &ConfiguredSeamService,
    lowered: &LoweredRequest,
    dispatch: WorkerDispatch,
    run_dir_root: &std::path::Path,
) -> Result<Value, OptimizeRunHostError> {
    let LoweredObjective::AgentKit { kit_files } = &lowered.objective else {
        return Err(OptimizeRunHostError::lowering(
            "execute_agent_kit requires an agent_kit objective",
        ));
    };

    #[cfg(test)]
    if let Some(runtime) = service.test_agent_runtime() {
        let output = run_kit_loop(KitLoopInputs {
            lowered,
            kit_files: kit_files.clone(),
            dispatch,
            runtime,
            run_dir: run_dir_root.to_path_buf(),
        })?;
        return project(lowered, output, run_dir_root);
    }

    let runtime = service.configured_codex_runtime().ok_or_else(|| {
        OptimizeRunHostError::worker_unavailable(
            "agentic reflection over an agent_kit seed requires a configured agent runtime (SeamAgentConfig::CodexCli)",
        )
    })?;
    let output = run_kit_loop(KitLoopInputs {
        lowered,
        kit_files: kit_files.clone(),
        dispatch,
        runtime,
        run_dir: run_dir_root.to_path_buf(),
    })?;
    project(lowered, output, run_dir_root)
}

fn project(
    lowered: &LoweredRequest,
    output: KitLoopOutput,
    run_dir_root: &std::path::Path,
) -> Result<Value, OptimizeRunHostError> {
    let KitLoopOutput {
        optimized,
        report,
        seed,
        artifacts,
    } = output;

    super::instrumentation::write_run_instrumentation(run_dir_root, &[], Some(&report));

    let revision = super::revision_label_kit(&optimized, &lowered.run_id);
    project_kit_result(&KitProjectionInputs {
        run_id: &lowered.run_id,
        seed_schema: &lowered.seed_schema,
        optimized: &optimized,
        report: &report,
        artifacts: &artifacts,
        seed: &seed,
        revision: &revision,
    })
}
