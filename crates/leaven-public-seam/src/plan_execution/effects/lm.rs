use std::collections::BTreeMap;

use leaven_lm::{
    JsonSchemaOutput, LmRequest, LmTool, Message, Messages, ModelName, OutputMode, ProviderHints,
    Role, SamplingOptions,
};
use serde_json::{Value, json};

use super::{cost_value, fingerprint_hex};
use crate::PublicSeamError;

pub struct PlanLmCompleteRequest<'a> {
    pub(in crate::plan_execution) name: &'a str,
    pub(in crate::plan_execution) call: &'a Value,
    pub(in crate::plan_execution) deps: &'a BTreeMap<String, Value>,
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
        let message_tool_call_id = object.get("tool_call_id").and_then(Value::as_str);
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
                let part_tool_call_id = part
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        invalid_lm_call("tool_result lm content part must carry tool_call_id")
                    })?;
                if let Some(message_tool_call_id) = message_tool_call_id
                    && message_tool_call_id != part_tool_call_id
                {
                    return Err(invalid_lm_call(
                        "tool message tool_call_id must match tool_result part tool_call_id",
                    ));
                }
                Message::tool_result(
                    part_tool_call_id,
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
        if let Some(tool_call_id) = message_tool_call_id {
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
    let mut hints = ProviderHints::default();
    for (key, value) in object {
        match key.as_str() {
            "prompt_cache_key" => {
                hints.prompt_cache_key = Some(
                    value
                        .as_str()
                        .ok_or_else(|| {
                            invalid_lm_call("provider_hints.prompt_cache_key must be a string")
                        })?
                        .to_owned(),
                );
            }
            "store" => {
                hints.store =
                    Some(value.as_bool().ok_or_else(|| {
                        invalid_lm_call("provider_hints.store must be a boolean")
                    })?);
            }
            "metadata" => {
                let metadata = value
                    .as_object()
                    .ok_or_else(|| invalid_lm_call("provider_hints.metadata must be an object"))?;
                hints.metadata = metadata
                    .iter()
                    .map(|(metadata_key, metadata_value)| {
                        metadata_value
                            .as_str()
                            .map(|value| (metadata_key.clone(), value.to_owned()))
                            .ok_or_else(|| {
                                invalid_lm_call("provider_hints.metadata values must be strings")
                            })
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?;
            }
            _ => {
                hints.values.insert(key.clone(), value.clone());
            }
        }
    }
    Ok(hints)
}

fn invalid_lm_call(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}

/// Host outcome for a typed `lm_complete` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanLmCompleteOutcome {
    pub(in crate::plan_execution) message: Value,
    pub(in crate::plan_execution) parsed: Option<Value>,
    pub(in crate::plan_execution) data_classes: Vec<String>,
    pub(in crate::plan_execution) replayability: String,
    pub(in crate::plan_execution) runtime_fingerprint: String,
    pub(in crate::plan_execution) error: Option<Value>,
    pub(in crate::plan_execution) cost: Option<Value>,
}

impl PlanLmCompleteOutcome {
    /// Creates an LM response outcome.
    pub fn new(message: Value, runtime_fingerprint: impl Into<String>) -> Self {
        Self {
            message,
            parsed: None,
            data_classes: vec!["public".to_owned()],
            replayability: "fully_managed".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
            error: None,
            cost: None,
        }
    }

    /// Creates an LM response outcome from the provider-neutral LM trait result.
    #[must_use]
    pub fn from_lm_response(
        response: leaven_kernel::Metered<leaven_lm::LmResponse>,
        runtime_fingerprint: leaven_kernel::Fingerprint,
    ) -> Self {
        let leaven_kernel::Metered { value, cost } = response;
        Self::new(
            lm_message_value(&value.assistant),
            format!("fp_runtime_sha256_{}", fingerprint_hex(runtime_fingerprint)),
        )
        .with_cost(cost_value(&cost))
    }

    /// Creates a failed paid LM outcome that still emits audit and charge receipts.
    pub fn failed_provider_error(
        message: impl Into<String>,
        runtime_fingerprint: impl Into<String>,
        usd_micro: u64,
    ) -> Self {
        Self {
            message: Value::Null,
            parsed: None,
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

    /// Attaches the parsed payload required by JSON-schema output contracts.
    #[must_use]
    pub fn with_parsed(mut self, parsed: Value) -> Self {
        self.parsed = Some(parsed);
        self
    }

    /// Attaches a cost object.
    #[must_use]
    pub fn with_cost(mut self, cost: Value) -> Self {
        self.cost = Some(cost);
        self
    }
}

fn lm_message_value(message: &leaven_lm::Message) -> Value {
    let mut object = serde_json::Map::new();
    object.insert("role".to_owned(), json!(lm_role_value(message.role())));
    object.insert(
        "content".to_owned(),
        Value::Array(
            message
                .content_parts()
                .iter()
                .map(lm_content_part_value)
                .collect(),
        ),
    );
    if let Some(tool_call_id) = message.tool_call_id() {
        object.insert("tool_call_id".to_owned(), json!(tool_call_id));
    }
    if let Some(name) = message.name() {
        object.insert("name".to_owned(), json!(name));
    }
    Value::Object(object)
}

fn lm_role_value(role: leaven_lm::Role) -> &'static str {
    match role {
        leaven_lm::Role::System => "system",
        leaven_lm::Role::Developer => "developer",
        leaven_lm::Role::User => "user",
        leaven_lm::Role::Assistant => "assistant",
        leaven_lm::Role::Tool => "tool",
    }
}

fn lm_content_part_value(part: &leaven_lm::MessageContentPart) -> Value {
    match part {
        leaven_lm::MessageContentPart::Text { text } => {
            json!({
                "kind": "text",
                "text": text
            })
        }
        leaven_lm::MessageContentPart::ToolResult {
            tool_call_id,
            content,
        } => {
            json!({
                "kind": "tool_result",
                "tool_call_id": tool_call_id,
                "content": content
            })
        }
    }
}
