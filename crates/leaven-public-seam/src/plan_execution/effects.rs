use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use leaven_agent::{
    AgentInstructions, AgentLimits, AgentRunRequest, AgentToolPolicy, OutputContract,
};
use leaven_kernel::AgentRuntimeId;
use leaven_workspace::{Command, CommandOutput, WorkspacePath};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::PublicSeamError;

mod agent;
mod blob_ref;
mod lm;

pub use lm::{PlanLmCompleteOutcome, PlanLmCompleteRequest};

/// Lowered `workspace_materialize` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanWorkspaceMaterializeRequest<'a> {
    pub(super) name: &'a str,
    pub(super) call: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
}

impl<'a> PlanWorkspaceMaterializeRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `workspace_materialize` call body from the Plan IR.
    pub const fn call(&self) -> &'a Value {
        self.call
    }

    /// Resolved dependency bindings visible to this call.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Candidate being materialized.
    pub fn candidate(&self) -> Result<&'a str, PublicSeamError> {
        self.call
            .get("candidate")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_call("workspace_materialize must carry candidate"))
    }

    /// Optional surface selector.
    #[must_use]
    pub fn surface(&self) -> Option<&'a str> {
        self.call.get("surface").and_then(Value::as_str)
    }

    /// Workspace materialization mode.
    pub fn mode(&self) -> Result<&'a str, PublicSeamError> {
        self.call
            .get("mode")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_call("workspace_materialize must carry mode"))
    }

    /// Requested workspace lifetime.
    pub fn lifetime(&self) -> Result<&'a str, PublicSeamError> {
        self.call
            .get("lifetime")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_call("workspace_materialize must carry lifetime"))
    }
}

/// Lowered `workspace_release` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanWorkspaceReleaseRequest<'a> {
    pub(super) name: &'a str,
    pub(super) call: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
    pub(super) live_workspaces: &'a BTreeMap<String, LiveWorkspaceHandle>,
}

impl<'a> PlanWorkspaceReleaseRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `workspace_release` call body from the Plan IR.
    pub const fn call(&self) -> &'a Value {
        self.call
    }

    /// Resolved dependency bindings visible to this call.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Workspace handle requested for release.
    pub fn workspace(&self) -> Result<&'a str, PublicSeamError> {
        workspace_ref_id(
            self.call.get("workspace"),
            "workspace_release must carry workspace",
        )
    }

    pub(super) fn workspace_ref(&self) -> Result<WorkspaceRefFacts, PublicSeamError> {
        workspace_ref_facts(
            self.call.get("workspace"),
            "workspace_release must carry workspace",
        )
    }

    /// Workspace handle requested for release, proven against live dependency handles.
    pub fn live_workspace(&self) -> Result<&'a str, PublicSeamError> {
        let workspace = self.workspace_ref()?;
        Ok(require_live_workspace_ref(
            &workspace,
            self.deps,
            self.live_workspaces,
            "workspace_release",
        )?
        .workspace())
    }

    /// Lifetime attached to the live dependency handle being released.
    pub(super) fn live_workspace_lifetime(&self) -> Result<&'a str, PublicSeamError> {
        let workspace = self.workspace_ref()?;
        Ok(require_live_workspace_ref(
            &workspace,
            self.deps,
            self.live_workspaces,
            "workspace_release",
        )?
        .lifetime())
    }

    /// Whether release may force cleanup.
    #[must_use]
    pub fn force(&self) -> bool {
        self.call
            .get("force")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
}

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
    pub(super) fn new(
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
    pub(super) fn new(
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

fn required_object<'a>(
    value: &'a Value,
    key: &str,
    message: impl Into<String>,
) -> Result<&'a serde_json::Map<String, Value>, PublicSeamError> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_call(message))
}

fn workspace_path(path: &str, context: &str) -> Result<WorkspacePath, PublicSeamError> {
    if path == "." {
        return Ok(WorkspacePath::root());
    }
    WorkspacePath::new(path).map_err(|error| invalid_call(format!("{context}: {error}")))
}

pub(super) fn workspace_ref_id(
    value: Option<&Value>,
    context: impl Into<String>,
) -> Result<&str, PublicSeamError> {
    let context = context.into();
    let value = value.ok_or_else(|| invalid_call(context.clone()))?;
    if let Some(workspace) = value.as_str() {
        return Ok(workspace);
    }
    let object = value.as_object().ok_or_else(|| {
        invalid_call(format!("{context}: workspace ref must be string or object"))
    })?;
    if object.get("kind").and_then(Value::as_str) != Some("workspace") {
        return Err(invalid_call(format!(
            "{context}: workspace ref object must have kind `workspace`"
        )));
    }
    object
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_call(format!("{context}: workspace ref object must carry id")))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WorkspaceRefFacts {
    id: String,
    run: Option<String>,
    snapshot_fingerprint: Option<String>,
}

impl WorkspaceRefFacts {
    fn from_id(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            run: None,
            snapshot_fingerprint: None,
        }
    }

    fn from_value(
        value: Option<&Value>,
        context: impl Into<String>,
    ) -> Result<Self, PublicSeamError> {
        let context = context.into();
        let value = value.ok_or_else(|| invalid_call(context.clone()))?;
        if let Some(workspace) = value.as_str() {
            return Ok(Self::from_id(workspace));
        }
        let object = value.as_object().ok_or_else(|| {
            invalid_call(format!("{context}: workspace ref must be string or object"))
        })?;
        if object.get("kind").and_then(Value::as_str) != Some("workspace") {
            return Err(invalid_call(format!(
                "{context}: workspace ref object must have kind `workspace`"
            )));
        }
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_call(format!("{context}: workspace ref object must carry id")))?
            .to_owned();
        let run = optional_string(object.get("run"), "workspace ref run")?;
        let snapshot_fingerprint = optional_string(
            object.get("snapshot_fingerprint"),
            "workspace ref snapshot_fingerprint",
        )?;
        Ok(Self {
            id,
            run,
            snapshot_fingerprint,
        })
    }

    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn to_value(&self) -> Value {
        if self.run.is_none() && self.snapshot_fingerprint.is_none() {
            return Value::String(self.id.clone());
        }
        let mut value = json!({
            "kind": "workspace",
            "id": self.id
        });
        if let Some(run) = &self.run {
            value["run"] = json!(run);
        }
        if let Some(snapshot_fingerprint) = &self.snapshot_fingerprint {
            value["snapshot_fingerprint"] = json!(snapshot_fingerprint);
        }
        value
    }

    pub(super) fn satisfies_request(&self, requested: &Self) -> bool {
        self.id == requested.id
            && self.run == requested.run
            && self.snapshot_fingerprint == requested.snapshot_fingerprint
    }
}

pub(super) fn workspace_ref_facts(
    value: Option<&Value>,
    context: impl Into<String>,
) -> Result<WorkspaceRefFacts, PublicSeamError> {
    WorkspaceRefFacts::from_value(value, context)
}

fn workspace_ref_object(
    workspace: &str,
    run: Option<String>,
    snapshot_fingerprint: Option<String>,
) -> Value {
    WorkspaceRefFacts {
        id: workspace.to_owned(),
        run,
        snapshot_fingerprint,
    }
    .to_value()
}

fn optional_string(value: Option<&Value>, field: &str) -> Result<Option<String>, PublicSeamError> {
    value
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid_call(format!("{field} must be a string")))
        })
        .transpose()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LiveWorkspaceHandle {
    workspace: WorkspaceRefFacts,
    lifetime: String,
    released: bool,
}

impl LiveWorkspaceHandle {
    pub(super) fn live_ref(workspace: WorkspaceRefFacts, lifetime: impl Into<String>) -> Self {
        Self {
            workspace,
            lifetime: lifetime.into(),
            released: false,
        }
    }

    pub(super) fn released_ref(workspace: WorkspaceRefFacts, lifetime: impl Into<String>) -> Self {
        Self {
            workspace,
            lifetime: lifetime.into(),
            released: true,
        }
    }

    pub(super) fn release(&mut self) {
        self.released = true;
    }

    pub(super) fn satisfies_workspace(&self, requested: &WorkspaceRefFacts) -> bool {
        self.workspace.satisfies_request(requested)
    }

    pub(super) fn workspace(&self) -> &str {
        self.workspace.id()
    }

    pub(super) fn lifetime(&self) -> &str {
        &self.lifetime
    }
}

pub(super) fn require_live_workspace_ref<'a>(
    requested: &WorkspaceRefFacts,
    deps: &'a BTreeMap<String, Value>,
    live_workspaces: &'a BTreeMap<String, LiveWorkspaceHandle>,
    context: &str,
) -> Result<&'a LiveWorkspaceHandle, PublicSeamError> {
    let Some((dep_name, handle_value)) = deps.iter().find(|(_, value)| {
        value.get("kind").and_then(Value::as_str) == Some("workspace_handle")
            && value
                .get("workspace")
                .and_then(|value| workspace_ref_facts(Some(value), "workspace handle").ok())
                .is_some_and(|available| available.satisfies_request(requested))
    }) else {
        return Err(invalid_call(format!(
            "{context} refused unmaterialized workspace `{}`",
            requested.id()
        )));
    };
    let Some(handle) = live_workspaces.get(dep_name) else {
        return Err(invalid_call(format!(
            "{context} refused unmaterialized workspace `{}`",
            requested.id()
        )));
    };
    if !handle.workspace.satisfies_request(requested) {
        return Err(invalid_call(format!(
            "{context} refused unmaterialized workspace `{}`",
            requested.id()
        )));
    }
    if handle.released
        || handle_value
            .get("released")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return Err(invalid_call(format!(
            "{context} refused already released workspace `{}`",
            requested.id()
        )));
    }
    Ok(handle)
}

fn invalid_call(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}

fn cost_value(cost: &leaven_kernel::Cost) -> Value {
    let mut object = serde_json::Map::new();
    if cost.prompt_tokens > 0 {
        object.insert("input_tokens".to_owned(), json!(cost.prompt_tokens));
    }
    if cost.completion_tokens > 0 {
        object.insert("output_tokens".to_owned(), json!(cost.completion_tokens));
    }
    if cost.llm_calls > 0 {
        object.insert("lm_calls".to_owned(), json!(cost.llm_calls));
    }
    if cost.metric_calls > 0 {
        object.insert("metric_calls".to_owned(), json!(cost.metric_calls));
    }
    insert_count_cost_axis(&mut object, cost, "agent_calls");
    insert_count_cost_axis(&mut object, cost, "sandbox_calls");
    insert_count_cost_axis(&mut object, cost, "usd_micro");
    insert_count_cost_axis(&mut object, cost, "human_review_usd_micro");
    insert_count_cost_axis(&mut object, cost, "wall_ms");
    Value::Object(object)
}

fn insert_count_cost_axis(
    object: &mut serde_json::Map<String, Value>,
    cost: &leaven_kernel::Cost,
    axis: &str,
) {
    let Some(amount) = cost.other.get(axis) else {
        return;
    };
    let amount = amount.as_f64();
    if amount > 0.0
        && amount.fract() == 0.0
        && let Ok(amount) = amount.to_string().parse::<u64>()
    {
        object.insert(axis.to_owned(), json!(amount));
    }
}

fn fingerprint_hex(fingerprint: leaven_kernel::Fingerprint) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in fingerprint.0 {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    output
}

fn extend_data_classes_from_blob_ref(data_classes: &mut Vec<String>, blob_ref: &Value) {
    let Some(blob_data_classes) = blob_ref
        .as_object()
        .and_then(|object| object.get("data_classes"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for data_class in blob_data_classes.iter().filter_map(Value::as_str) {
        blob_ref::push_unique_data_class(data_classes, data_class);
    }
}

include!("effects/outcomes.rs");
