use std::collections::BTreeMap;
use std::time::Duration;

use leaven_agent::{
    AgentInstructions, AgentLimits, AgentRunRequest, AgentToolPolicy, OutputContract,
};
use leaven_workspace::{Command, CommandLimits, CommandOutput, WorkspacePath};
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
#[derive(Clone, Copy, Debug)]
pub struct PlanAgentRunRequest<'a> {
    pub(super) name: &'a str,
    pub(super) call: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
    pub(super) live_workspaces: &'a BTreeMap<String, LiveWorkspaceHandle>,
}

impl<'a> PlanAgentRunRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `agent_run` call body from the Plan IR.
    pub const fn call(&self) -> &'a Value {
        self.call
    }

    /// Resolved dependency bindings visible to this call.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Workspace handle requested for agent execution.
    pub fn workspace(&self) -> Result<&'a str, PublicSeamError> {
        workspace_ref_id(self.call.get("workspace"), "agent_run must carry workspace")
    }

    pub(super) fn workspace_ref(&self) -> Result<WorkspaceRefFacts, PublicSeamError> {
        workspace_ref_facts(self.call.get("workspace"), "agent_run must carry workspace")
    }

    /// Workspace handle requested for agent execution, proven against live
    /// dependency handles.
    pub fn live_workspace(&self) -> Result<&'a str, PublicSeamError> {
        let workspace = self.workspace_ref()?;
        Ok(
            require_live_workspace_ref(&workspace, self.deps, self.live_workspaces, "agent_run")?
                .workspace(),
        )
    }

    /// Lowers the locked Plan IR `agent_run` call into provider-neutral agent
    /// runtime vocabulary.
    ///
    /// This preserves only the output contracts currently owned by
    /// `leaven-agent`; unsupported schema-valid contracts return an explicit
    /// error instead of being treated as an unstructured final message.
    pub fn to_agent_run_request(&self) -> Result<AgentRunRequest, PublicSeamError> {
        let instructions = lower_agent_instructions(required_object(
            self.call,
            "instructions",
            "agent_run must carry instructions",
        )?)?;
        let output_contract = lower_agent_output_contract(required_object(
            self.call,
            "output",
            "agent_run must carry output contract",
        )?)?;
        let mut request = AgentRunRequest::new(instructions, output_contract);
        if let Some(policy) = self.call.get("tool_policy").and_then(Value::as_object) {
            request.tool_policy = lower_agent_tool_policy(policy)?;
        }
        if let Some(limits) = self.call.get("limits").and_then(Value::as_object) {
            request.limits = lower_agent_limits(limits)?;
        }
        Ok(request)
    }
}

/// Lowered `sandbox_exec` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanSandboxExecRequest<'a> {
    pub(super) name: &'a str,
    pub(super) call: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
    pub(super) live_workspaces: &'a BTreeMap<String, LiveWorkspaceHandle>,
}

impl<'a> PlanSandboxExecRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `sandbox_exec` call body from the Plan IR.
    pub const fn call(&self) -> &'a Value {
        self.call
    }

    /// Resolved dependency bindings visible to this call.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Requested stream policy, defaulting to buffered output.
    #[must_use]
    pub fn stream_policy(&self) -> &str {
        self.call
            .get("stream_policy")
            .and_then(Value::as_str)
            .unwrap_or("buffer")
    }

    /// Workspace handle requested for sandbox execution.
    pub fn workspace(&self) -> Result<&'a str, PublicSeamError> {
        workspace_ref_id(
            self.call.get("workspace"),
            "sandbox_exec must carry workspace",
        )
    }

    pub(super) fn workspace_ref(&self) -> Result<WorkspaceRefFacts, PublicSeamError> {
        workspace_ref_facts(
            self.call.get("workspace"),
            "sandbox_exec must carry workspace",
        )
    }

    /// Workspace handle requested for sandbox execution, proven against live
    /// dependency handles.
    pub fn live_workspace(&self) -> Result<&'a str, PublicSeamError> {
        let workspace = self.workspace_ref()?;
        Ok(
            require_live_workspace_ref(
                &workspace,
                self.deps,
                self.live_workspaces,
                "sandbox_exec",
            )?
            .workspace(),
        )
    }

    /// Lowers the locked Plan IR `sandbox_exec` call into backend-neutral
    /// workspace command vocabulary.
    pub fn to_workspace_command(&self) -> Result<Command, PublicSeamError> {
        let argv = self
            .call
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
        if let Some(cwd) = self.call.get("cwd").and_then(Value::as_str) {
            command.cwd = Some(workspace_path(cwd, "sandbox_exec cwd")?);
        }
        if let Some(env) = self.call.get("env").and_then(Value::as_object) {
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
        command.limits = CommandLimits {
            timeout: Some(Duration::from_secs(
                self.call
                    .get("timeout_s")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| invalid_call("sandbox_exec must carry timeout_s"))?,
            )),
            ..CommandLimits::default()
        };
        Ok(command)
    }
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

    fn to_value(&self) -> Value {
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

/// Host outcome for a typed `agent_run` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanAgentRunOutcome {
    pub(super) status: String,
    pub(super) parsed: Option<Value>,
    pub(super) transcript_ref: Option<Value>,
    pub(super) commands: Vec<Value>,
    pub(super) data_classes: Vec<String>,
    pub(super) replayability: String,
    pub(super) runtime_fingerprint: String,
    pub(super) cost: Option<Value>,
}

impl PlanAgentRunOutcome {
    /// Creates a completed agent session outcome.
    #[must_use]
    pub fn completed(runtime_fingerprint: impl Into<String>) -> Self {
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
    #[must_use]
    pub fn from_agent_session(
        session: leaven_kernel::Metered<leaven_agent::AgentSession>,
        runtime_fingerprint: leaven_kernel::Fingerprint,
        transcript_ref: Value,
        session_receipt: impl Into<String>,
    ) -> Self {
        let leaven_kernel::Metered { value, cost } = session;
        let session_receipt = session_receipt.into();
        Self::completed(format!(
            "fp_runtime_sha256_{}",
            fingerprint_hex(runtime_fingerprint)
        ))
        .with_status(agent::agent_status_value(&value.status))
        .with_transcript_ref(transcript_ref)
        .with_commands(
            value
                .commands
                .iter()
                .map(|command| agent::agent_command_value(command, &session_receipt)),
        )
        .with_cost(cost_value(&cost))
    }

    /// Attaches a transcript blob reference.
    #[must_use]
    pub fn with_transcript_ref(mut self, transcript_ref: Value) -> Self {
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
    pub fn with_commands(mut self, commands: impl IntoIterator<Item = Value>) -> Self {
        self.commands = commands.into_iter().collect();
        self
    }

    /// Attaches a cost object.
    #[must_use]
    pub fn with_cost(mut self, cost: Value) -> Self {
        self.cost = Some(cost);
        self
    }

    #[must_use]
    fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }
}

/// Host outcome for a typed `sandbox_exec` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanSandboxExecOutcome {
    pub(super) status: String,
    pub(super) exit_code: Option<i64>,
    pub(super) stdout_ref: Option<Value>,
    pub(super) stderr_ref: Option<Value>,
    pub(super) files: BTreeMap<String, Value>,
    pub(super) data_classes: Vec<String>,
    pub(super) replayability: String,
    pub(super) runtime_fingerprint: String,
    pub(super) cost: Option<Value>,
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
        output: leaven_kernel::Metered<CommandOutput>,
        runtime_fingerprint: leaven_kernel::Fingerprint,
        stdout_ref: Value,
        stderr_ref: Value,
    ) -> Result<Self, PublicSeamError> {
        let leaven_kernel::Metered { value, cost } = output;
        validate_stream_blob_ref(&stdout_ref, &value.stdout.bytes, "sandbox stdout")?;
        validate_stream_blob_ref(&stderr_ref, &value.stderr.bytes, "sandbox stderr")?;
        let mut outcome = Self::completed(format!(
            "fp_runtime_sha256_{}",
            fingerprint_hex(runtime_fingerprint)
        ));
        outcome.exit_code = value.status.code.map(i64::from);
        Ok(outcome
            .with_stream_refs(stdout_ref, stderr_ref)
            .with_cost(cost_value(&cost)))
    }

    /// Attaches stdout and stderr blob references.
    #[must_use]
    pub fn with_stream_refs(mut self, stdout_ref: Value, stderr_ref: Value) -> Self {
        extend_data_classes_from_blob_ref(&mut self.data_classes, &stdout_ref);
        extend_data_classes_from_blob_ref(&mut self.data_classes, &stderr_ref);
        self.stdout_ref = Some(stdout_ref);
        self.stderr_ref = Some(stderr_ref);
        self
    }

    /// Attaches a captured output file blob reference.
    #[must_use]
    pub fn with_file_ref(mut self, path: impl Into<String>, blob_ref: Value) -> Self {
        extend_data_classes_from_blob_ref(&mut self.data_classes, &blob_ref);
        self.files.insert(path.into(), blob_ref);
        self
    }

    /// Attaches a cost object.
    #[must_use]
    pub fn with_cost(mut self, cost: Value) -> Self {
        self.cost = Some(cost);
        self
    }
}

fn validate_stream_blob_ref(
    blob_ref: &Value,
    bytes: &[u8],
    stream: &str,
) -> Result<(), PublicSeamError> {
    let object = blob_ref
        .as_object()
        .ok_or_else(|| invalid_call(format!("{stream} blob ref must be an object")))?;
    let declared_bytes = object
        .get("bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_call(format!("{stream} blob ref must carry bytes")))?;
    let actual_bytes = u64::try_from(bytes.len()).map_err(|_| {
        invalid_call(format!(
            "{stream} captured output is too large for public byte audit"
        ))
    })?;
    if declared_bytes != actual_bytes {
        return Err(invalid_call(format!(
            "{stream} blob ref bytes `{declared_bytes}` do not match captured output bytes `{actual_bytes}`"
        )));
    }
    let declared_sha = object
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_call(format!("{stream} blob ref must carry sha256")))?;
    let actual_sha = lower_hex_sha256(bytes);
    if declared_sha != actual_sha {
        return Err(invalid_call(format!(
            "{stream} blob ref sha256 does not match captured output"
        )));
    }
    Ok(())
}

fn lower_hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Host outcome for a typed `workspace_materialize` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanWorkspaceMaterializeOutcome {
    pub(super) workspace: String,
    pub(super) workspace_ref: Value,
    pub(super) lifetime: String,
    pub(super) data_classes: Vec<String>,
    pub(super) replayability: String,
    pub(super) runtime_fingerprint: String,
}

impl PlanWorkspaceMaterializeOutcome {
    /// Creates a live workspace handle outcome.
    #[must_use]
    pub fn new(
        workspace: impl Into<String>,
        lifetime: impl Into<String>,
        runtime_fingerprint: impl Into<String>,
    ) -> Self {
        let workspace = workspace.into();
        Self {
            workspace_ref: Value::String(workspace.clone()),
            workspace,
            lifetime: lifetime.into(),
            data_classes: vec!["public".to_owned()],
            replayability: "boundary_managed".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
        }
    }

    #[must_use]
    pub fn with_workspace_object_ref(
        mut self,
        run: Option<impl Into<String>>,
        snapshot_fingerprint: Option<impl Into<String>>,
    ) -> Self {
        self.workspace_ref = workspace_ref_object(
            &self.workspace,
            run.map(Into::into),
            snapshot_fingerprint.map(Into::into),
        );
        self
    }
}

/// Host outcome for a typed `workspace_release` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanWorkspaceReleaseOutcome {
    pub(super) workspace: String,
    pub(super) workspace_ref: Value,
    pub(super) lifetime: String,
    pub(super) runtime_fingerprint: String,
}

impl PlanWorkspaceReleaseOutcome {
    /// Creates a workspace release outcome.
    #[must_use]
    pub fn new(
        workspace: impl Into<String>,
        lifetime: impl Into<String>,
        runtime_fingerprint: impl Into<String>,
    ) -> Self {
        let workspace = workspace.into();
        Self {
            workspace_ref: Value::String(workspace.clone()),
            workspace,
            lifetime: lifetime.into(),
            runtime_fingerprint: runtime_fingerprint.into(),
        }
    }

    #[must_use]
    pub fn with_workspace_object_ref(
        mut self,
        run: Option<impl Into<String>>,
        snapshot_fingerprint: Option<impl Into<String>>,
    ) -> Self {
        self.workspace_ref = workspace_ref_object(
            &self.workspace,
            run.map(Into::into),
            snapshot_fingerprint.map(Into::into),
        );
        self
    }
}

/// Lowered `emit_run_event` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanEmitRunEventRequest<'a> {
    pub(super) name: &'a str,
    pub(super) write: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
    pub(super) base_revision: &'a str,
}

impl<'a> PlanEmitRunEventRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `emit_run_event` write body from the Plan IR.
    pub const fn write(&self) -> &'a Value {
        self.write
    }

    /// Resolved dependency bindings visible to this write.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Base graph revision supplied by the public-seam execution context.
    pub const fn base_revision(&self) -> &'a str {
        self.base_revision
    }
}

/// Host outcome for a typed `emit_run_event` write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanEmitRunEventOutcome {
    pub(super) event_id: String,
    pub(super) committed_revision: String,
}

impl PlanEmitRunEventOutcome {
    /// Creates an emitted event outcome.
    pub fn new(event_id: impl Into<String>, committed_revision: impl Into<String>) -> Self {
        Self {
            event_id: event_id.into(),
            committed_revision: committed_revision.into(),
        }
    }
}
