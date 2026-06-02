use std::collections::BTreeSet;

use serde_json::Value;

use crate::PublicSeamError;

mod extension_result;
mod lifecycle;
mod methods;
mod permission;
mod session;
mod stage_run_rpc;

pub use extension_result::AcpExtensionResultDocument;
pub use lifecycle::{
    AcpProgressDisposition, AcpProgressPriority, AcpSessionCancellation, AcpSessionLifecycle,
    AcpSessionState, AcpSessionUpdate,
};
pub use permission::{
    AcpPermissionDecision, AcpPermissionRequest, authenticate, authorize_permission,
};
pub use session::{
    AcpAuthenticateRequest, AcpAuthenticatedSession, AcpStdioWorkerLaunch, AcpWorkerSession,
};
pub use stage_run_rpc::{AcpStageRunRequestDocument, AcpStageRunResponseDocument};

/// Schema-valid Leaven ACP profile document with V1 semantic checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpProfileDocument {
    pinned_acp_version: String,
    transports: Vec<String>,
    token_env: String,
    endpoint_env: String,
    fingerprint_env: String,
    extension_methods: Vec<AcpExtensionMethod>,
    default_max_inflight_updates: u64,
    backpressure: AcpBackpressure,
}

impl AcpProfileDocument {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_acp("ACP profile must be an object"))?;
        let pinned_acp_version =
            required_string(object.get("pinned_acp_version"), "pinned_acp_version")?.to_owned();
        if pinned_acp_version.trim().is_empty() || pinned_acp_version == "latest" {
            return Err(invalid_acp("ACP profile must pin a concrete ACP version"));
        }
        let transports = string_array(object.get("transports"), "transports")?;
        if transports.first().map(String::as_str) != Some("stdio_jsonrpc") {
            return Err(invalid_acp(
                "Leaven V1 ACP profile must prefer stdio_jsonrpc first",
            ));
        }
        let auth = object
            .get("auth")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_acp("ACP profile must declare auth"))?;
        require_const(
            auth.get("token_env"),
            "LEAVEN_CAPABILITY_TOKEN",
            "auth.token_env",
        )?;
        require_const(
            auth.get("endpoint_env"),
            "LEAVEN_ENDPOINT",
            "auth.endpoint_env",
        )?;
        require_const(
            auth.get("fingerprint_env"),
            "LEAVEN_CAPABILITY_FINGERPRINT",
            "auth.fingerprint_env",
        )?;
        require_const(
            auth.get("authenticate_maps_to"),
            "leaven.capability.v1",
            "auth.authenticate_maps_to",
        )?;
        let permission = object
            .get("permission_model")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_acp("ACP profile must declare permission_model"))?;
        require_const(
            permission.get("answer"),
            "programmatic capability grant check",
            "permission_model.answer",
        )?;
        require_const(
            permission.get("denial"),
            "PlanError + Redaction",
            "permission_model.denial",
        )?;
        let extension_methods = extension_methods(object.get("extension_methods"))?;
        let advertised = extension_methods
            .iter()
            .map(|method| method.method.as_str())
            .collect::<BTreeSet<_>>();
        let locked = locked_extension_methods()
            .into_iter()
            .collect::<BTreeSet<_>>();
        if advertised != locked {
            return Err(invalid_acp(
                "ACP profile must advertise exactly the locked Leaven V1 extension methods",
            ));
        }
        let flow_control = object
            .get("flow_control")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_acp("ACP profile must declare flow_control"))?;
        if flow_control.get("bounded_channel_required") != Some(&Value::Bool(true)) {
            return Err(invalid_acp("ACP update queues must be bounded"));
        }
        let default_max_inflight_updates = flow_control
            .get("default_max_inflight_updates")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid_acp("flow_control.default_max_inflight_updates is required"))?;
        let backpressure = AcpBackpressure::from_wire(required_string(
            flow_control.get("backpressure"),
            "flow_control.backpressure",
        )?)?;

        Ok(Self {
            pinned_acp_version,
            transports,
            token_env: "LEAVEN_CAPABILITY_TOKEN".to_owned(),
            endpoint_env: "LEAVEN_ENDPOINT".to_owned(),
            fingerprint_env: "LEAVEN_CAPABILITY_FINGERPRINT".to_owned(),
            extension_methods,
            default_max_inflight_updates,
            backpressure,
        })
    }

    /// Pinned ACP protocol version.
    pub fn pinned_acp_version(&self) -> &str {
        &self.pinned_acp_version
    }

    /// Transport bindings in preference order.
    pub fn transports(&self) -> &[String] {
        &self.transports
    }

    /// Environment variable that carries the opaque capability bearer token.
    pub fn token_env(&self) -> &str {
        &self.token_env
    }

    /// Environment variable that carries the ACP endpoint.
    pub fn endpoint_env(&self) -> &str {
        &self.endpoint_env
    }

    /// Environment variable that carries the expected capability fingerprint.
    pub fn fingerprint_env(&self) -> &str {
        &self.fingerprint_env
    }

    /// Leaven ACP extension methods.
    pub fn extension_methods(&self) -> &[AcpExtensionMethod] {
        &self.extension_methods
    }

    /// Bounded update queue capacity advertised by the profile.
    pub const fn default_max_inflight_updates(&self) -> u64 {
        self.default_max_inflight_updates
    }

    /// Backpressure strategy required by the locked ACP profile.
    pub const fn backpressure(&self) -> AcpBackpressure {
        self.backpressure
    }

    /// Looks up one extension method by name.
    pub fn method(&self, method: &str) -> Option<&AcpExtensionMethod> {
        self.extension_methods
            .iter()
            .find(|entry| entry.method == method)
    }
}

/// One Leaven ACP extension method declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpExtensionMethod {
    method: String,
    params_schema: String,
    result_schema: String,
    required_action: String,
    produces_receipt: bool,
}

impl AcpExtensionMethod {
    /// ACP extension method name.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Required capability action.
    pub fn required_action(&self) -> &str {
        &self.required_action
    }

    /// JSON Schema used for method params.
    pub fn params_schema(&self) -> &str {
        &self.params_schema
    }

    /// JSON Schema used for method results.
    pub fn result_schema(&self) -> &str {
        &self.result_schema
    }

    /// Whether the method declares receipt-producing results.
    pub const fn produces_receipt(&self) -> bool {
        self.produces_receipt
    }
}

/// Locked ACP progress-update backpressure strategy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcpBackpressure {
    /// Pause worker progress until the engine acknowledges queued updates.
    PauseWorker,
    /// Drop noncritical progress updates while preserving critical updates.
    DropNoncriticalUpdates,
    /// Disconnect the session when the bounded update queue is overproduced.
    Disconnect,
}

impl AcpBackpressure {
    fn from_wire(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "pause_worker" => Ok(Self::PauseWorker),
            "drop_noncritical_updates" => Ok(Self::DropNoncriticalUpdates),
            "disconnect" => Ok(Self::Disconnect),
            other => Err(invalid_acp(format!(
                "unknown ACP backpressure strategy `{other}`"
            ))),
        }
    }

    /// Wire spelling from the locked ACP profile.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PauseWorker => "pause_worker",
            Self::DropNoncriticalUpdates => "drop_noncritical_updates",
            Self::Disconnect => "disconnect",
        }
    }
}

/// ACP JSON-RPC request envelope for one Leaven extension method.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpJsonRpcRequestDocument {
    id: String,
    method: String,
}

impl AcpJsonRpcRequestDocument {
    pub(crate) fn from_plan_valid_value(
        profile: &AcpProfileDocument,
        value: &Value,
    ) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_acp("ACP JSON-RPC request must be an object"))?;
        require_jsonrpc_2(object)?;
        if object.contains_key("result") || object.contains_key("error") {
            return Err(invalid_acp(
                "ACP JSON-RPC request must not carry response result or error fields",
            ));
        }
        require_jsonrpc_members(
            object,
            &["jsonrpc", "id", "method", "params"],
            "ACP JSON-RPC request",
        )?;
        let id = jsonrpc_id(object.get("id"))?;
        let method = required_string(object.get("method"), "method")?.to_owned();
        if profile.method(&method).is_none() {
            return Err(invalid_acp(format!(
                "ACP JSON-RPC request method `{method}` is not in the locked Leaven profile"
            )));
        }
        object
            .get("params")
            .ok_or_else(|| invalid_acp("ACP JSON-RPC request must carry Plan IR params"))?;
        Ok(Self { id, method })
    }

    /// JSON-RPC request id, normalized to a string for request/response binding.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Leaven ACP extension method.
    pub fn method(&self) -> &str {
        &self.method
    }
}

/// ACP JSON-RPC response envelope for one Leaven extension method result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpJsonRpcResponseDocument {
    id: String,
    method: String,
    primary_kind: String,
    result: Value,
}

impl AcpJsonRpcResponseDocument {
    pub(crate) fn from_extension_result_value(
        request: &AcpJsonRpcRequestDocument,
        extension: &AcpExtensionResultDocument,
        value: &Value,
    ) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_acp("ACP JSON-RPC response must be an object"))?;
        require_jsonrpc_2(object)?;
        if object.contains_key("method") || object.contains_key("params") {
            return Err(invalid_acp(
                "ACP JSON-RPC response must not carry request method or params fields",
            ));
        }
        require_jsonrpc_members(
            object,
            &["jsonrpc", "id", "result"],
            "ACP JSON-RPC response",
        )?;
        let id = jsonrpc_id(object.get("id"))?;
        if id != request.id() {
            return Err(invalid_acp(
                "ACP JSON-RPC response id must match the extension request id",
            ));
        }
        if object.contains_key("error") {
            return Err(invalid_acp(
                "ACP JSON-RPC extension success response must carry result, not error",
            ));
        }
        object
            .get("result")
            .ok_or_else(|| invalid_acp("ACP JSON-RPC response must carry extension result"))?;
        if extension.method() != request.method() {
            return Err(invalid_acp(
                "ACP JSON-RPC extension result method must match the request method",
            ));
        }
        Ok(Self {
            id,
            method: extension.method().to_owned(),
            primary_kind: extension.primary_kind().to_owned(),
            result: object
                .get("result")
                .expect("result was required above")
                .clone(),
        })
    }

    /// JSON-RPC response id, normalized to a string for request/response binding.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Leaven ACP extension method answered by this response.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Primary result kind returned by the extension result.
    pub fn primary_kind(&self) -> &str {
        &self.primary_kind
    }

    /// Validated ACP extension result payload carried by the JSON-RPC response.
    pub fn result(&self) -> &Value {
        &self.result
    }
}

fn extension_methods(value: Option<&Value>) -> Result<Vec<AcpExtensionMethod>, PublicSeamError> {
    let methods = required_array(value, "extension_methods")?;
    let mut seen = BTreeSet::new();
    let mut output = Vec::with_capacity(methods.len());
    for entry in methods {
        let entry = entry
            .as_object()
            .ok_or_else(|| invalid_acp("extension method entries must be objects"))?;
        let method = required_string(entry.get("method"), "extension_methods.method")?.to_owned();
        if !method.starts_with("leaven/") || method.to_ascii_lowercase().contains("mcp") {
            return Err(invalid_acp(format!(
                "extension method `{method}` is not a Leaven ACP method"
            )));
        }
        if !seen.insert(method.clone()) {
            return Err(invalid_acp(format!(
                "duplicate ACP extension method `{method}`"
            )));
        }
        let required_action =
            required_string(entry.get("required_action"), "required_action")?.to_owned();
        let Some(expected_action) = required_action_for_method(&method) else {
            return Err(invalid_acp(format!(
                "extension method `{method}` is not in the locked ACP profile"
            )));
        };
        if expected_action != required_action {
            return Err(invalid_acp(format!(
                "extension method `{method}` required_action does not match Leaven profile"
            )));
        }
        let produces_receipt = entry
            .get("produces_receipt")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid_acp("extension method must declare produces_receipt"))?;
        if !produces_receipt {
            return Err(invalid_acp(format!(
                "extension method `{method}` must produce receipts"
            )));
        }
        let (expected_params, expected_result) =
            schema_binding_for_method(&method).ok_or_else(|| {
                invalid_acp(format!(
                    "extension method `{method}` is not in the locked ACP profile"
                ))
            })?;
        let params_schema =
            required_string(entry.get("params_schema"), "params_schema")?.to_owned();
        if params_schema != expected_params.schema_file() {
            return Err(invalid_acp(format!(
                "extension method `{method}` params_schema must bind `{}`",
                expected_params.schema_file()
            )));
        }
        let result_schema =
            required_string(entry.get("result_schema"), "result_schema")?.to_owned();
        if result_schema != expected_result.schema_file() {
            return Err(invalid_acp(format!(
                "extension method `{method}` result_schema must bind `{}`",
                expected_result.schema_file()
            )));
        }
        output.push(AcpExtensionMethod {
            method,
            params_schema,
            result_schema,
            required_action,
            produces_receipt,
        });
    }
    Ok(output)
}

fn locked_extension_methods() -> [&'static str; 26] {
    methods::locked_extension_methods()
}

fn required_action_for_method(method: &str) -> Option<&'static str> {
    methods::required_action_for_method(method)
}

fn schema_binding_for_method(
    method: &str,
) -> Option<(methods::MethodSchema, methods::MethodSchema)> {
    methods::schema_binding_for_method(method)
}

fn require_const(
    value: Option<&Value>,
    expected: &str,
    field: &str,
) -> Result<(), PublicSeamError> {
    if value.and_then(Value::as_str) == Some(expected) {
        Ok(())
    } else {
        Err(invalid_acp(format!(
            "ACP profile `{field}` must be `{expected}`"
        )))
    }
}

fn required_string<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_acp(format!("ACP profile field `{field}` must be a string")))
}

fn require_jsonrpc_2(object: &serde_json::Map<String, Value>) -> Result<(), PublicSeamError> {
    match object.get("jsonrpc").and_then(Value::as_str) {
        Some("2.0") => Ok(()),
        _ => Err(invalid_acp(
            "ACP JSON-RPC envelope must declare jsonrpc 2.0",
        )),
    }
}

fn require_jsonrpc_members(
    object: &serde_json::Map<String, Value>,
    allowed: &[&str],
    context: &str,
) -> Result<(), PublicSeamError> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(invalid_acp(format!(
                "{context} must not carry extra top-level field `{key}`"
            )));
        }
    }
    Ok(())
}

fn jsonrpc_id(value: Option<&Value>) -> Result<String, PublicSeamError> {
    match value {
        Some(Value::String(id)) if !id.trim().is_empty() => Ok(id.clone()),
        Some(Value::Number(number)) => Ok(number.to_string()),
        Some(Value::Null) | None => Err(invalid_acp(
            "ACP JSON-RPC extension calls must carry a non-null id",
        )),
        Some(_) => Err(invalid_acp(
            "ACP JSON-RPC id must be a string or number for request/response binding",
        )),
    }
}

fn required_array<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<&'a Vec<Value>, PublicSeamError> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_acp(format!("ACP profile field `{field}` must be an array")))
}

fn string_array(value: Option<&Value>, field: &str) -> Result<Vec<String>, PublicSeamError> {
    required_array(value, field)?
        .iter()
        .map(|item| {
            item.as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid_acp(format!("ACP profile field `{field}` must be strings")))
        })
        .collect()
}

pub fn invalid_acp(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidScope {
        message: message.into(),
    }
}

fn prefixed_jcs_hash(prefix: &str, value: &Value) -> Result<String, PublicSeamError> {
    Ok(format!(
        "{prefix}{}",
        jcs_canonicalize::sha256_jcs_hex(value)
            .map_err(|error| invalid_acp(format!("failed to hash ACP result value: {error}")))?
    ))
}
