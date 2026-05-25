use std::collections::BTreeMap;
use std::time::Duration;

use leaven_agent::{
    AgentInstructions, AgentLimits, AgentRunRequest, AgentToolPolicy, OutputContract,
};
use leaven_kernel::AgentRuntimeId;
use leaven_workspace::WorkspacePath;
use serde_json::{Value, json};

use super::{
    LiveWorkspaceHandle, WorkspaceRefFacts, invalid_call, require_live_workspace_ref,
    required_object, workspace_path, workspace_ref_facts,
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
