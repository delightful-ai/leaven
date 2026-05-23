use std::collections::BTreeMap;
use std::time::Duration;

use leaven_agent::{
    AgentInstructions, AgentLimits, AgentRunRequest, AgentToolPolicy, OutputContract,
};
use leaven_lm::{
    JsonSchemaOutput, LmRequest, LmTool, Message, Messages, ModelName, OutputMode, ProviderHints,
    Role, SamplingOptions,
};
use leaven_workspace::{Command, CommandLimits, WorkspacePath};
use serde_json::{Value, json};

use crate::PublicSeamError;

/// Lowered `agent_run` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanAgentRunRequest<'a> {
    pub(super) name: &'a str,
    pub(super) call: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
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
        }),
        Some("json_schema") => Err(invalid_call(
            "agent_run json_schema output needs an owned leaven-agent structured-output primitive",
        )),
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
            .unwrap_or(true),
        allowed_tools: Vec::new(),
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

fn invalid_call(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}

/// Lowered `lm_complete` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanLmCompleteRequest<'a> {
    pub(super) name: &'a str,
    pub(super) call: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
}

impl<'a> PlanLmCompleteRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `lm_complete` call body from the Plan IR.
    pub const fn call(&self) -> &'a Value {
        self.call
    }

    /// Resolved dependency bindings visible to this call.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Lowers the locked Plan IR `lm_complete` call into provider-neutral LM
    /// vocabulary.
    ///
    /// This rejects V1-deferred or extension-only LM content instead of
    /// silently downgrading it to text.
    pub fn to_lm_request(&self) -> Result<LmRequest, PublicSeamError> {
        let model = self
            .call
            .get("model")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_lm_call("lm_complete lowering requires explicit model"))?;
        let messages = lower_lm_messages(
            self.call
                .get("messages")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid_lm_call("lm_complete must carry messages"))?,
        )?;
        let mut request = LmRequest::new(ModelName::new(model), messages).with_output(
            lower_lm_output(self.call.get("output").ok_or_else(|| {
                invalid_lm_call("lm_complete lowering requires output contract")
            })?)?,
        );
        if let Some(model_role) = self.call.get("model_role").and_then(Value::as_str) {
            request = request.with_model_role(model_role);
        }
        if let Some(sampling) = self.call.get("sampling") {
            request = request.with_sampling(lower_lm_sampling(sampling)?);
        }
        if let Some(tools) = self.call.get("tools").and_then(Value::as_array) {
            request = request.with_tools(lower_lm_tools(tools)?);
        }
        if let Some(provider_hints) = self.call.get("provider_hints") {
            request = request.with_provider_hints(lower_provider_hints(provider_hints)?);
        }
        Ok(request)
    }
}

fn lower_lm_messages(messages: &[Value]) -> Result<Messages, PublicSeamError> {
    let mut lowered = Messages::new();
    for message in messages {
        let object = message
            .as_object()
            .ok_or_else(|| invalid_lm_call("lm message must be an object"))?;
        let role = match object.get("role").and_then(Value::as_str) {
            Some("system") => Role::System,
            Some("developer") => Role::Developer,
            Some("user") => Role::User,
            Some("assistant") => Role::Assistant,
            Some("tool") => Role::Tool,
            Some(other) => {
                return Err(invalid_lm_call(format!(
                    "unsupported lm message role `{other}`"
                )));
            }
            None => return Err(invalid_lm_call("lm message must carry role")),
        };
        let content = object
            .get("content")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_lm_call("lm message must carry content parts"))?;
        let mut message = match (role, content.as_slice()) {
            (_, [part]) if part.get("kind").and_then(Value::as_str) == Some("text") => {
                Message::new(
                    role,
                    part.get("text")
                        .and_then(Value::as_str)
                        .ok_or_else(|| invalid_lm_call("text lm content part must carry text"))?,
                )
            }
            (Role::Tool, [part])
                if part.get("kind").and_then(Value::as_str) == Some("tool_result") =>
            {
                Message::tool_result(
                    part.get("tool_call_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            invalid_lm_call("tool_result lm content part must carry tool_call_id")
                        })?,
                    part.get("content").and_then(Value::as_str).ok_or_else(|| {
                        invalid_lm_call("tool_result lm content part must carry content")
                    })?,
                )
            }
            (_, _) => {
                return Err(invalid_lm_call(
                    "lm_complete V1 lowering supports text parts and tool_result tool messages only",
                ));
            }
        };
        if let Some(tool_call_id) = object.get("tool_call_id").and_then(Value::as_str) {
            message = message.with_tool_call_id(tool_call_id);
        }
        if let Some(name) = object.get("name").and_then(Value::as_str) {
            message = message.with_name(name);
        }
        lowered.push(message);
    }
    Ok(lowered)
}

fn lower_lm_tools(tools: &[Value]) -> Result<Vec<LmTool>, PublicSeamError> {
    tools
        .iter()
        .map(|tool| {
            let object = tool
                .as_object()
                .ok_or_else(|| invalid_lm_call("lm tool must be an object"))?;
            Ok(LmTool {
                name: object
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid_lm_call("lm tool must carry name"))?
                    .to_owned(),
                description: object
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                input_schema: object
                    .get("input_schema")
                    .cloned()
                    .ok_or_else(|| invalid_lm_call("lm tool must carry input_schema"))?,
                requires_capability_action: object
                    .get("requires_capability_action")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            })
        })
        .collect()
}

fn lower_lm_sampling(value: &Value) -> Result<SamplingOptions, PublicSeamError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_lm_call("lm sampling must be an object"))?;
    let mut sampling = SamplingOptions::default();
    if let Some(value) = object.get("temperature").and_then(Value::as_f64) {
        sampling.temperature = Some(
            leaven_kernel::FiniteF64::new(value)
                .map_err(|error| invalid_lm_call(format!("invalid temperature: {error}")))?,
        );
    }
    if let Some(value) = object.get("top_p").and_then(Value::as_f64) {
        sampling.top_p = Some(
            leaven_kernel::FiniteF64::new(value)
                .map_err(|error| invalid_lm_call(format!("invalid top_p: {error}")))?,
        );
    }
    sampling.max_output_tokens = object
        .get("max_output_tokens")
        .and_then(Value::as_u64)
        .map(u32::try_from)
        .transpose()
        .map_err(|_| invalid_lm_call("max_output_tokens exceeds u32"))?;
    sampling.seed = object.get("seed").and_then(Value::as_u64);
    if let Some(stop) = object.get("stop").and_then(Value::as_array) {
        sampling.stop = stop
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| invalid_lm_call("lm stop sequence must be a string"))
            })
            .collect::<Result<Vec<_>, _>>()?;
    }
    Ok(sampling)
}

fn lower_lm_output(value: &Value) -> Result<OutputMode, PublicSeamError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_lm_call("lm output contract must be an object"))?;
    match object.get("kind").and_then(Value::as_str) {
        Some("final_message") => Ok(OutputMode::FinalMessage {
            max_bytes: object.get("max_bytes").and_then(Value::as_u64),
        }),
        Some("json_schema") => Ok(OutputMode::JsonSchema(JsonSchemaOutput {
            name: object
                .get("schema_fingerprint")
                .and_then(Value::as_str)
                .unwrap_or("schema")
                .to_owned(),
            schema: object.get("schema").cloned().unwrap_or(Value::Null),
            strict: true,
        })),
        Some(other) => Err(invalid_lm_call(format!(
            "unsupported lm output contract `{other}`"
        ))),
        None => Err(invalid_lm_call("lm output contract must carry kind")),
    }
}

fn lower_provider_hints(value: &Value) -> Result<ProviderHints, PublicSeamError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_lm_call("provider_hints must be an object"))?;
    Ok(ProviderHints {
        values: object.clone().into_iter().collect(),
        ..ProviderHints::default()
    })
}

fn invalid_lm_call(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}

/// Host outcome for a typed `lm_complete` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanLmCompleteOutcome {
    pub(super) message: Value,
    pub(super) data_classes: Vec<String>,
    pub(super) replayability: String,
    pub(super) runtime_fingerprint: String,
    pub(super) error: Option<Value>,
    pub(super) cost: Option<Value>,
}

impl PlanLmCompleteOutcome {
    /// Creates an LM response outcome.
    pub fn new(message: Value, runtime_fingerprint: impl Into<String>) -> Self {
        Self {
            message,
            data_classes: vec!["public".to_owned()],
            replayability: "fully_managed".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
            error: None,
            cost: None,
        }
    }

    /// Creates a failed paid LM outcome that still emits audit and charge receipts.
    pub fn failed_provider_error(
        message: impl Into<String>,
        runtime_fingerprint: impl Into<String>,
        usd_micro: u64,
    ) -> Self {
        Self {
            message: Value::Null,
            data_classes: Vec::new(),
            replayability: "has_declared_external_effects".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
            error: Some(json!({
                "code": "provider_error",
                "message": message.into(),
                "retryable": true
            })),
            cost: Some(json!({
                "usd_micro": usd_micro
            })),
        }
    }

    /// Overrides the data classes carried by the LM response value.
    #[must_use]
    pub fn with_data_classes(mut self, data_classes: impl IntoIterator<Item = String>) -> Self {
        self.data_classes = data_classes.into_iter().collect();
        self
    }

    /// Overrides the replayability classification carried by the LM response value.
    #[must_use]
    pub fn with_replayability(mut self, replayability: impl Into<String>) -> Self {
        self.replayability = replayability.into();
        self
    }
}

/// Host outcome for a typed `agent_run` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanAgentRunOutcome {
    pub(super) status: String,
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
            transcript_ref: None,
            commands: Vec::new(),
            data_classes: vec!["public".to_owned()],
            replayability: "has_declared_external_effects".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
            cost: None,
        }
    }

    /// Attaches a transcript blob reference.
    #[must_use]
    pub fn with_transcript_ref(mut self, transcript_ref: Value) -> Self {
        self.transcript_ref = Some(transcript_ref);
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

    /// Attaches stdout and stderr blob references.
    #[must_use]
    pub fn with_stream_refs(mut self, stdout_ref: Value, stderr_ref: Value) -> Self {
        self.stdout_ref = Some(stdout_ref);
        self.stderr_ref = Some(stderr_ref);
        self
    }

    /// Attaches a cost object.
    #[must_use]
    pub fn with_cost(mut self, cost: Value) -> Self {
        self.cost = Some(cost);
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
