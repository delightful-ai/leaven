use std::collections::BTreeMap;
use std::time::Duration;

use leaven_agent::{
    AgentInstructions, AgentLimits, AgentRunRequest, AgentToolPolicy, OutputContract,
};
use leaven_kernel::AgentRuntimeId;
use leaven_workspace::WorkspacePath;
use serde_json::{Value, json};

use super::{
    LiveWorkspaceHandle, WorkspaceRefFacts, blob_ref, cost_value,
    extend_data_classes_from_blob_ref, invalid_call, require_live_workspace_ref, required_object,
    workspace_path, workspace_ref_facts,
};
use crate::PublicSeamError;

/// Lowered `agent_run` request passed to a plan execution host.
#[derive(Clone, Debug)]
pub struct PlanAgentRunRequest<'a> {
    pub(super) name: &'a str,
    pub(super) deps: &'a BTreeMap<String, Value>,
    pub(super) live_workspaces: &'a BTreeMap<String, LiveWorkspaceHandle>,
    workspace: WorkspaceRefFacts,
    agent_request: AgentRunRequest,
}

impl<'a> PlanAgentRunRequest<'a> {
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
                "agent_run must carry workspace",
            )?,
            agent_request: lower_agent_run_call(call)?,
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

    /// Workspace handle requested for agent execution.
    pub fn workspace(&self) -> &str {
        self.workspace.id()
    }

    /// Workspace handle requested for agent execution, proven against live
    /// dependency handles.
    pub fn live_workspace(&self) -> Result<&'a str, PublicSeamError> {
        Ok(require_live_workspace_ref(
            &self.workspace,
            self.deps,
            self.live_workspaces,
            "agent_run",
        )?
        .workspace())
    }

    /// Provider-neutral agent request already lowered by the public seam before
    /// host execution.
    pub const fn agent_run_request(&self) -> &AgentRunRequest {
        &self.agent_request
    }

    /// Runtime selector requested by the Plan IR.
    pub fn runtime(&self) -> Option<&AgentRuntimeId> {
        self.agent_request.runtime.as_ref()
    }

    /// Expected runtime fingerprint requested by the Plan IR, when supplied.
    pub fn runtime_fingerprint(&self) -> Option<&str> {
        self.agent_request.runtime_fingerprint.as_deref()
    }

    /// Consumes the request wrapper and returns the provider-neutral agent
    /// request already lowered by the public seam.
    pub fn into_agent_run_request(self) -> AgentRunRequest {
        self.agent_request
    }
}

/// Lowers the locked Plan IR `agent_run` call into provider-neutral agent
/// runtime vocabulary.
///
/// This preserves only the output contracts currently owned by `leaven-agent`;
/// unsupported schema-valid contracts return an explicit error before any host
/// can execute the call.
fn lower_agent_run_call(call: &Value) -> Result<AgentRunRequest, PublicSeamError> {
    let runtime = call
        .get("runtime")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_call("agent_run must carry runtime"))?;
    let instructions = lower_agent_instructions(required_object(
        call,
        "instructions",
        "agent_run must carry instructions",
    )?)?;
    let output_contract = lower_agent_output_contract(required_object(
        call,
        "output",
        "agent_run must carry output contract",
    )?)?;
    let mut request = AgentRunRequest::new(instructions, output_contract);
    request = request.with_runtime(runtime.to_owned());
    if let Some(runtime_fingerprint) = call.get("runtime_fingerprint").and_then(Value::as_str) {
        request = request.with_runtime_fingerprint(runtime_fingerprint.to_owned());
    }
    if let Some(policy) = call.get("tool_policy").and_then(Value::as_object) {
        request.tool_policy = lower_agent_tool_policy(policy)?;
    }
    if let Some(limits) = call.get("limits").and_then(Value::as_object) {
        request.limits = lower_agent_limits(limits)?;
    }
    Ok(request)
}

fn lower_agent_instructions(
    object: &serde_json::Map<String, Value>,
) -> Result<AgentInstructions, PublicSeamError> {
    let task = object
        .get("task")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_call("agent_run instructions must carry task"))?;
    let mut instructions = AgentInstructions::task(task);
    if let Some(system) = object.get("system").and_then(Value::as_str) {
        instructions.system = Some(system.to_owned());
    }
    Ok(instructions)
}

fn lower_agent_output_contract(
    object: &serde_json::Map<String, Value>,
) -> Result<OutputContract, PublicSeamError> {
    match object.get("kind").and_then(Value::as_str) {
        Some("final_message") => Ok(OutputContract::FinalMessage),
        Some("files") => {
            let paths = object
                .get("paths")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid_call("files output contract must carry paths"))?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .ok_or_else(|| invalid_call("files output paths must be strings"))
                        .and_then(|path| workspace_path(path, "files output path"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(OutputContract::Files { paths })
        }
        Some("workspace_diff") => Ok(OutputContract::WorkspaceDiff {
            roots: vec![WorkspacePath::root()],
            surface_fingerprint: Some(
                object
                    .get("surface_fingerprint")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        invalid_call("workspace_diff output must carry surface_fingerprint")
                    })?
                    .to_owned(),
            ),
        }),
        Some("json_schema") => Ok(OutputContract::JsonSchema {
            schema_fingerprint: object
                .get("schema_fingerprint")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_call("json_schema output must carry schema_fingerprint"))?
                .to_owned(),
            schema: object.get("schema").cloned().unwrap_or(Value::Null),
        }),
        Some(other) => Err(invalid_call(format!(
            "unsupported agent_run output contract `{other}`"
        ))),
        None => Err(invalid_call("agent_run output contract must carry kind")),
    }
}

fn lower_agent_tool_policy(
    object: &serde_json::Map<String, Value>,
) -> Result<AgentToolPolicy, PublicSeamError> {
    let mut policy = AgentToolPolicy {
        allow_shell: object
            .get("allow_shell")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        allowed_tools: Vec::new(),
        allowed_commands: Vec::new(),
    };
    if let Some(tools) = object.get("allowed_tools").and_then(Value::as_array) {
        policy.allowed_tools = tools
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| invalid_call("agent_run allowed_tools must be strings"))
            })
            .collect::<Result<Vec<_>, _>>()?;
    }
    if let Some(commands) = object.get("allowed_commands").and_then(Value::as_array) {
        policy.allowed_commands = commands
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| invalid_call("agent_run allowed_commands must be strings"))
            })
            .collect::<Result<Vec<_>, _>>()?;
    }
    Ok(policy)
}

fn lower_agent_limits(
    object: &serde_json::Map<String, Value>,
) -> Result<AgentLimits, PublicSeamError> {
    Ok(AgentLimits {
        timeout: object
            .get("timeout_s")
            .and_then(Value::as_u64)
            .map(Duration::from_secs),
        max_turns: object
            .get("max_turns")
            .and_then(Value::as_u64)
            .map(u32::try_from)
            .transpose()
            .map_err(|_| invalid_call("agent_run max_turns exceeds u32"))?,
        max_output_bytes: None,
    })
}

/// Host outcome for a typed `agent_run` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanAgentRunOutcome {
    pub(in crate::plan_execution) status: String,
    pub(in crate::plan_execution) parsed: Option<Value>,
    pub(in crate::plan_execution) transcript_ref: Option<Value>,
    pub(in crate::plan_execution) commands: Vec<Value>,
    pub(in crate::plan_execution) data_classes: Vec<String>,
    pub(in crate::plan_execution) replayability: String,
    pub(in crate::plan_execution) runtime_fingerprint: String,
    pub(in crate::plan_execution) cost: Option<Value>,
}

/// Blob refs for one observed command inside a provider-neutral agent session.
///
/// These refs are supplied by the host after it persists the observed command
/// streams/files. The seam verifies the refs against the captured bytes before
/// they can appear in a public `agent_session` value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentCommandOutputRefs {
    stdout_ref: Value,
    stderr_ref: Value,
    file_refs: BTreeMap<WorkspacePath, Value>,
}

impl AgentCommandOutputRefs {
    /// Creates refs for stdout and stderr captured by an agent command.
    #[must_use]
    pub fn new(stdout_ref: Value, stderr_ref: Value) -> Self {
        Self {
            stdout_ref,
            stderr_ref,
            file_refs: BTreeMap::new(),
        }
    }

    /// Attaches a persisted blob ref for one captured output file.
    #[must_use]
    pub fn with_output_file(mut self, path: WorkspacePath, blob_ref: Value) -> Self {
        self.file_refs.insert(path, blob_ref);
        self
    }
}

impl PlanAgentRunOutcome {
    /// Creates a completed agent session outcome.
    #[must_use]
    fn completed(runtime_fingerprint: impl Into<String>) -> Self {
        Self {
            status: "completed".to_owned(),
            parsed: None,
            transcript_ref: None,
            commands: Vec::new(),
            data_classes: vec!["public".to_owned()],
            replayability: "has_declared_external_effects".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
            cost: None,
        }
    }

    /// Creates an agent session outcome from provider-neutral agent evidence.
    ///
    /// Every command observed in the session must have stdout/stderr refs, and
    /// every captured command output file must have an exact path-matched blob
    /// ref. This prevents hosts from treating unbound stdout as a proposal or
    /// attaching unrelated blobs after the agent has run.
    pub fn from_agent_session_with_command_output_refs(
        session: leaven_kernel::Metered<leaven_agent::AgentSession>,
        runtime_fingerprint: leaven_kernel::Fingerprint,
        transcript_ref: Value,
        session_receipt: impl Into<String>,
        command_output_refs: impl IntoIterator<Item = AgentCommandOutputRefs>,
    ) -> Result<Self, PublicSeamError> {
        let leaven_kernel::Metered { value, cost } = session;
        let session_receipt = session_receipt.into();
        let command_output_refs = command_output_refs.into_iter().collect::<Vec<_>>();
        if command_output_refs.len() != value.commands.len() {
            return Err(invalid_call(format!(
                "agent session has {} commands but {} command output ref sets",
                value.commands.len(),
                command_output_refs.len()
            )));
        }
        let mut outcome = Self::completed(format!(
            "fp_runtime_sha256_{}",
            runtime_fingerprint.to_hex()
        ))
        .with_status(agent_status_value(&value.status))
        .with_transcript_ref(transcript_ref)
        .with_cost(cost_value(&cost));
        let commands = value
            .commands
            .iter()
            .zip(command_output_refs)
            .enumerate()
            .map(|(index, (command, refs))| {
                outcome.command_value_with_output_refs(index, command, &session_receipt, refs)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(outcome.with_commands(commands))
    }

    /// Attaches a transcript blob reference.
    #[must_use]
    fn with_transcript_ref(mut self, transcript_ref: Value) -> Self {
        extend_data_classes_from_blob_ref(&mut self.data_classes, &transcript_ref);
        self.transcript_ref = Some(transcript_ref);
        self
    }

    /// Attaches the parsed payload required by JSON-schema output contracts.
    #[must_use]
    pub fn with_parsed(mut self, parsed: Value) -> Self {
        self.parsed = Some(parsed);
        self
    }

    /// Attaches command audit records.
    #[must_use]
    fn with_commands(mut self, commands: impl IntoIterator<Item = Value>) -> Self {
        self.commands.clear();
        for command in commands {
            extend_data_classes_from_agent_command(&mut self.data_classes, &command);
            self.commands.push(command);
        }
        self
    }

    /// Attaches a cost object.
    #[must_use]
    fn with_cost(mut self, cost: Value) -> Self {
        self.cost = Some(cost);
        self
    }

    #[must_use]
    fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    fn command_value_with_output_refs(
        &mut self,
        index: usize,
        command: &leaven_agent::CommandRecord,
        receipt: &str,
        refs: AgentCommandOutputRefs,
    ) -> Result<Value, PublicSeamError> {
        if command.output.stdout.truncated {
            return Err(invalid_call(format!(
                "agent command {index} stdout capture is truncated and cannot be bound to a blob ref"
            )));
        }
        blob_ref::validate_stream_blob_ref(
            &refs.stdout_ref,
            &command.output.stdout.bytes,
            &format!("agent command {index} stdout"),
        )?;
        if command.output.stderr.truncated {
            return Err(invalid_call(format!(
                "agent command {index} stderr capture is truncated and cannot be bound to a blob ref"
            )));
        }
        blob_ref::validate_stream_blob_ref(
            &refs.stderr_ref,
            &command.output.stderr.bytes,
            &format!("agent command {index} stderr"),
        )?;
        extend_data_classes_from_blob_ref(&mut self.data_classes, &refs.stdout_ref);
        extend_data_classes_from_blob_ref(&mut self.data_classes, &refs.stderr_ref);

        let mut file_refs = refs.file_refs;
        for path in file_refs.keys() {
            if !command.output.output_files.contains_key(path) {
                return Err(invalid_call(format!(
                    "agent command {index} output file `{}` blob ref does not match a captured command output file",
                    path.as_str()
                )));
            }
        }

        let mut files = serde_json::Map::new();
        for (path, captured) in &command.output.output_files {
            if captured.truncated {
                return Err(invalid_call(format!(
                    "agent command {index} output file `{}` capture is truncated and cannot be bound to a blob ref",
                    path.as_str()
                )));
            }
            let blob_ref = file_refs.remove(path).ok_or_else(|| {
                invalid_call(format!(
                    "agent command {index} output file `{}` is missing a blob ref",
                    path.as_str()
                ))
            })?;
            blob_ref::validate_stream_blob_ref(
                &blob_ref,
                &captured.bytes,
                &format!("agent command {index} output file `{}`", path.as_str()),
            )?;
            extend_data_classes_from_blob_ref(&mut self.data_classes, &blob_ref);
            files.insert(path.as_str().to_owned(), blob_ref);
        }

        let mut value = agent_command_value(command, receipt);
        let object = value
            .as_object_mut()
            .expect("agent command values are JSON objects");
        object.insert("stdout_ref".to_owned(), refs.stdout_ref);
        object.insert("stderr_ref".to_owned(), refs.stderr_ref);
        if !files.is_empty() {
            object.insert("files".to_owned(), Value::Object(files));
        }
        Ok(value)
    }
}

fn extend_data_classes_from_agent_command(data_classes: &mut Vec<String>, command: &Value) {
    let Some(command) = command.as_object() else {
        return;
    };
    if let Some(stdout_ref) = command.get("stdout_ref") {
        extend_data_classes_from_blob_ref(data_classes, stdout_ref);
    }
    if let Some(stderr_ref) = command.get("stderr_ref") {
        extend_data_classes_from_blob_ref(data_classes, stderr_ref);
    }
    if let Some(files) = command.get("files").and_then(Value::as_object) {
        for blob_ref in files.values() {
            extend_data_classes_from_blob_ref(data_classes, blob_ref);
        }
    }
}

pub(super) fn agent_status_value(status: &leaven_agent::AgentStatus) -> &'static str {
    match status {
        leaven_agent::AgentStatus::Succeeded => "completed",
        leaven_agent::AgentStatus::Failed { .. }
        | leaven_agent::AgentStatus::OutputContractViolation { .. } => "failed",
        leaven_agent::AgentStatus::Cancelled => "cancelled",
        leaven_agent::AgentStatus::TimedOut => "timeout",
    }
}

pub(super) fn agent_command_value(command: &leaven_agent::CommandRecord, receipt: &str) -> Value {
    let mut argv = Vec::with_capacity(command.command.args.len() + 1);
    argv.push(command.command.program.clone());
    argv.extend(command.command.args.clone());
    json!({
        "argv": argv,
        "status": command_status_value(command.output.status),
        "receipt": receipt
    })
}

fn command_status_value(status: leaven_workspace::ExitStatus) -> &'static str {
    if status.code == Some(0) {
        "completed"
    } else {
        "failed"
    }
}
