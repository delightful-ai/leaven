use std::collections::BTreeMap;
use std::time::Duration;

use leaven_workspace::Command;
use serde_json::Value;

use super::{
    LiveWorkspaceHandle, WorkspaceRefFacts, invalid_call, require_live_workspace_ref,
    required_object, workspace_path, workspace_ref_facts,
};
use crate::PublicSeamError;

/// Lowered `sandbox_exec` request passed to a plan execution host.
#[derive(Clone, Debug)]
pub struct PlanSandboxExecRequest<'a> {
    pub(super) name: &'a str,
    pub(super) deps: &'a BTreeMap<String, Value>,
    pub(super) live_workspaces: &'a BTreeMap<String, LiveWorkspaceHandle>,
    workspace: WorkspaceRefFacts,
    stream_policy: String,
    command: Command,
}

impl<'a> PlanSandboxExecRequest<'a> {
    pub(in crate::plan_execution) fn new(
        name: &'a str,
        call: &'a Value,
        deps: &'a BTreeMap<String, Value>,
        live_workspaces: &'a BTreeMap<String, LiveWorkspaceHandle>,
    ) -> Result<Self, PublicSeamError> {
        Ok(Self {
            name,
            deps,
            live_workspaces,
            workspace: workspace_ref_facts(
                call.get("workspace"),
                "sandbox_exec must carry workspace",
            )?,
            stream_policy: call
                .get("stream_policy")
                .and_then(Value::as_str)
                .unwrap_or("buffer")
                .to_owned(),
            command: lower_sandbox_exec_call(call)?,
        })
    }

    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Resolved dependency bindings visible to this call.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Requested stream policy, defaulting to buffered output.
    #[must_use]
    pub fn stream_policy(&self) -> &str {
        &self.stream_policy
    }

    /// Workspace handle requested for sandbox execution.
    pub fn workspace(&self) -> &str {
        self.workspace.id()
    }

    /// Workspace handle requested for sandbox execution, proven against live
    /// dependency handles.
    pub fn live_workspace(&self) -> Result<&'a str, PublicSeamError> {
        Ok(require_live_workspace_ref(
            &self.workspace,
            self.deps,
            self.live_workspaces,
            "sandbox_exec",
        )?
        .workspace())
    }

    /// Backend-neutral workspace command already lowered by the public seam
    /// before host execution.
    pub const fn workspace_command(&self) -> &Command {
        &self.command
    }

    /// Consumes the request wrapper and returns the backend-neutral workspace
    /// command already lowered by the public seam.
    pub fn into_workspace_command(self) -> Command {
        self.command
    }
}

/// Lowers the locked Plan IR `sandbox_exec` call into backend-neutral workspace
/// command vocabulary before any host can execute it.
fn lower_sandbox_exec_call(call: &Value) -> Result<Command, PublicSeamError> {
    let argv = call
        .get("argv")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_call("sandbox_exec must carry argv"))?;
    let program = argv
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_call("sandbox_exec argv must start with program"))?;
    let mut command = Command::new(program);
    command.args = argv
        .iter()
        .skip(1)
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid_call("sandbox_exec argv entries must be strings"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(cwd) = call.get("cwd").and_then(Value::as_str) {
        command.cwd = Some(workspace_path(cwd, "sandbox_exec cwd")?);
    }
    if let Some(env) = call.get("env").and_then(Value::as_object) {
        command.env = env
            .iter()
            .map(|(key, value)| {
                Ok((
                    key.clone(),
                    value
                        .as_str()
                        .ok_or_else(|| invalid_call("sandbox_exec env values must be strings"))?
                        .to_owned(),
                ))
            })
            .collect::<Result<BTreeMap<_, _>, PublicSeamError>>()?;
    }
    lower_sandbox_output_contract(
        required_object(call, "output", "sandbox_exec must carry output contract")?,
        &mut command,
    )?;
    command.limits.timeout = Some(Duration::from_secs(
        call.get("timeout_s")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid_call("sandbox_exec must carry timeout_s"))?,
    ));
    Ok(command)
}

fn lower_sandbox_output_contract(
    object: &serde_json::Map<String, Value>,
    command: &mut Command,
) -> Result<(), PublicSeamError> {
    match object.get("kind").and_then(Value::as_str) {
        Some("files") => {
            command.output_files = object
                .get("paths")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid_call("files output contract must carry paths"))?
                .iter()
                .map(|path| {
                    let path = path
                        .as_str()
                        .ok_or_else(|| invalid_call("files output paths must be strings"))?;
                    workspace_path(path, "files output path")
                })
                .collect::<Result<Vec<_>, _>>()?;
            command.limits.max_output_file_bytes = object.get("max_bytes").and_then(Value::as_u64);
            Ok(())
        }
        Some("final_message" | "json_schema" | "workspace_diff") => Ok(()),
        Some(other) => Err(invalid_call(format!(
            "unsupported sandbox_exec output contract `{other}`"
        ))),
        None => Err(invalid_call("sandbox_exec output contract must carry kind")),
    }
}
