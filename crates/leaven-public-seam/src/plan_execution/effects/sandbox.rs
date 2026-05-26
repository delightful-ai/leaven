use std::collections::BTreeMap;
use std::time::Duration;

use leaven_workspace::Command;
use serde_json::Value;

use super::{
    LiveWorkspaceHandle, WorkspaceRefFacts, blob_ref, cost_value,
    extend_data_classes_from_blob_ref, invalid_call, require_live_workspace_ref, required_object,
    workspace_path, workspace_ref_facts,
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

/// Host outcome for a typed `sandbox_exec` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanSandboxExecOutcome {
    pub(in crate::plan_execution) status: String,
    pub(in crate::plan_execution) exit_code: Option<i64>,
    pub(in crate::plan_execution) stdout_ref: Option<Value>,
    pub(in crate::plan_execution) stderr_ref: Option<Value>,
    pub(in crate::plan_execution) files: BTreeMap<String, Value>,
    pub(in crate::plan_execution) data_classes: Vec<String>,
    pub(in crate::plan_execution) replayability: String,
    pub(in crate::plan_execution) runtime_fingerprint: String,
    pub(in crate::plan_execution) cost: Option<Value>,
}

impl PlanSandboxExecOutcome {
    /// Creates a completed sandbox execution outcome.
    #[must_use]
    pub fn completed(runtime_fingerprint: impl Into<String>) -> Self {
        Self {
            status: "completed".to_owned(),
            exit_code: Some(0),
            stdout_ref: None,
            stderr_ref: None,
            files: BTreeMap::new(),
            data_classes: vec!["public".to_owned()],
            replayability: "boundary_managed".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
            cost: None,
        }
    }

    /// Creates a sandbox outcome from provider-neutral workspace command output.
    pub fn from_command_output(
        output: leaven_kernel::Metered<leaven_workspace::CommandOutput>,
        runtime_fingerprint: leaven_kernel::Fingerprint,
        stdout_ref: Value,
        stderr_ref: Value,
    ) -> Result<Self, PublicSeamError> {
        Self::from_command_output_with_file_refs(
            output,
            runtime_fingerprint,
            stdout_ref,
            stderr_ref,
            std::iter::empty::<(leaven_workspace::WorkspacePath, Value)>(),
        )
    }

    /// Creates a sandbox outcome from command output plus blob refs for captured files.
    ///
    /// Every file captured by the backend-neutral command output must have a
    /// matching blob ref, and every supplied file blob ref must correspond to a
    /// captured workspace file. This keeps file artifacts bound to the command
    /// result instead of letting hosts attach unrelated blobs after execution.
    pub fn from_command_output_with_file_refs(
        output: leaven_kernel::Metered<leaven_workspace::CommandOutput>,
        runtime_fingerprint: leaven_kernel::Fingerprint,
        stdout_ref: Value,
        stderr_ref: Value,
        file_refs: impl IntoIterator<Item = (leaven_workspace::WorkspacePath, Value)>,
    ) -> Result<Self, PublicSeamError> {
        let leaven_kernel::Metered { value, cost } = output;
        blob_ref::validate_stream_blob_ref(&stdout_ref, &value.stdout.bytes, "sandbox stdout")?;
        blob_ref::validate_stream_blob_ref(&stderr_ref, &value.stderr.bytes, "sandbox stderr")?;
        let mut outcome = Self::completed(format!(
            "fp_runtime_sha256_{}",
            runtime_fingerprint.to_hex()
        ));
        outcome.exit_code = value.status.code.map(i64::from);
        outcome = outcome.with_stream_refs(stdout_ref, stderr_ref);

        let mut file_refs_by_path = BTreeMap::new();
        for (path, blob_ref) in file_refs {
            if file_refs_by_path.insert(path.clone(), blob_ref).is_some() {
                return Err(invalid_call(format!(
                    "sandbox output file `{}` has duplicate blob refs",
                    path.as_str()
                )));
            }
        }
        for path in file_refs_by_path.keys() {
            if !value.output_files.contains_key(path) {
                return Err(invalid_call(format!(
                    "sandbox output file `{}` blob ref does not match a captured command output file",
                    path.as_str()
                )));
            }
        }
        for (path, captured) in &value.output_files {
            if captured.truncated {
                return Err(invalid_call(format!(
                    "sandbox output file `{}` capture is truncated and cannot be bound to a blob ref",
                    path.as_str()
                )));
            }
            let blob_ref = file_refs_by_path.get(path).ok_or_else(|| {
                invalid_call(format!(
                    "sandbox output file `{}` is missing a blob ref",
                    path.as_str()
                ))
            })?;
            outcome = outcome.with_file_ref(path.as_str(), blob_ref.clone(), &captured.bytes)?;
        }

        Ok(outcome.with_cost(cost_value(&cost)))
    }

    /// Attaches stdout and stderr blob references.
    #[must_use]
    fn with_stream_refs(mut self, stdout_ref: Value, stderr_ref: Value) -> Self {
        extend_data_classes_from_blob_ref(&mut self.data_classes, &stdout_ref);
        extend_data_classes_from_blob_ref(&mut self.data_classes, &stderr_ref);
        self.stdout_ref = Some(stdout_ref);
        self.stderr_ref = Some(stderr_ref);
        self
    }

    /// Attaches a captured output file blob reference after binding its byte audit.
    fn with_file_ref(
        mut self,
        path: impl Into<String>,
        blob_ref: Value,
        bytes: impl AsRef<[u8]>,
    ) -> Result<Self, PublicSeamError> {
        let path = path.into();
        leaven_workspace::WorkspacePath::new(&path).map_err(|error| {
            invalid_call(format!(
                "sandbox output file path must be relative workspace path: {error}"
            ))
        })?;
        blob_ref::validate_stream_blob_ref(
            &blob_ref,
            bytes.as_ref(),
            &format!("sandbox output file `{path}`"),
        )?;
        extend_data_classes_from_blob_ref(&mut self.data_classes, &blob_ref);
        self.files.insert(path, blob_ref);
        Ok(self)
    }

    /// Attaches a cost object.
    #[must_use]
    pub fn with_cost(mut self, cost: Value) -> Self {
        self.cost = Some(cost);
        self
    }
}
