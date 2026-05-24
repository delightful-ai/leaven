use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::{
    CapabilityDocument, CapabilityGrantRequest, CapabilityLimitUsage, CapabilityRegistry,
    PublicSeamError,
};

mod extension_result;
mod lifecycle;
mod methods;

pub use extension_result::AcpExtensionResultDocument;
pub use lifecycle::{
    AcpProgressDisposition, AcpProgressPriority, AcpSessionCancellation, AcpSessionLifecycle,
    AcpSessionState, AcpSessionUpdate,
};

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

/// ACP permission request projected to a capability grant check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpPermissionRequest {
    method: String,
    resource: BTreeMap<String, Value>,
    case_fields: BTreeSet<String>,
    partition: Option<String>,
    input_classes: BTreeSet<String>,
    purposes: BTreeSet<String>,
    models: BTreeSet<String>,
    model_roles: BTreeSet<String>,
    workspace_ops: BTreeSet<String>,
    command: Option<String>,
    schemas: BTreeSet<String>,
    surface: Option<String>,
    limits: CapabilityLimitUsage,
}

/// ACP authenticate request that resolves a bearer token into a capability document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpAuthenticateRequest {
    token_id: String,
    now: String,
    expected_capability_fingerprint: String,
}

impl AcpAuthenticateRequest {
    /// Creates an authenticate request from an opaque public-seam token handle.
    pub fn opaque(
        token_id: impl Into<String>,
        now: impl Into<String>,
        expected_capability_fingerprint: impl Into<String>,
    ) -> Self {
        Self {
            token_id: token_id.into(),
            now: now.into(),
            expected_capability_fingerprint: expected_capability_fingerprint.into(),
        }
    }
}

/// Resolved ACP authentication state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpAuthenticatedSession {
    capability_fingerprint: String,
    policy_fingerprint: String,
    subject_fingerprint: String,
    jti: String,
}

impl AcpAuthenticatedSession {
    /// Capability fingerprint resolved by `authenticate`.
    pub fn capability_fingerprint(&self) -> &str {
        &self.capability_fingerprint
    }

    /// Policy fingerprint carried by the resolved capability.
    pub fn policy_fingerprint(&self) -> &str {
        &self.policy_fingerprint
    }

    /// Subject fingerprint carried by the resolved capability.
    pub fn subject_fingerprint(&self) -> &str {
        &self.subject_fingerprint
    }

    /// JWT id of the resolved capability document.
    pub fn jti(&self) -> &str {
        &self.jti
    }
}

/// Profile-derived ACP worker session facts for lifecycle/backpressure validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpWorkerSession {
    pinned_acp_version: String,
    transport: String,
    engine_role: String,
    worker_role: String,
    lifecycle: AcpSessionLifecycle,
}

/// Stdio ACP worker launch environment with a redacted artifact projection.
#[derive(Clone, Eq, PartialEq)]
pub struct AcpStdioWorkerLaunch {
    transport: String,
    engine_role: String,
    worker_role: String,
    token_env: String,
    endpoint_env: String,
    fingerprint_env: String,
    bearer_token: String,
    endpoint: String,
    capability_fingerprint: String,
    worker_env: BTreeMap<String, String>,
}

impl AcpStdioWorkerLaunch {
    /// Builds the stdio launch environment for a validated ACP worker session.
    pub fn new(
        profile: &AcpProfileDocument,
        session: &AcpWorkerSession,
        bearer_token: impl Into<String>,
        endpoint: impl Into<String>,
        capability_fingerprint: impl Into<String>,
    ) -> Result<Self, PublicSeamError> {
        if session.transport() != "stdio_jsonrpc" {
            return Err(invalid_acp(
                "ACP stdio worker launch requires stdio_jsonrpc transport",
            ));
        }
        let bearer_token = bearer_token.into();
        let endpoint = endpoint.into();
        let capability_fingerprint = capability_fingerprint.into();
        if bearer_token.trim().is_empty() {
            return Err(invalid_acp(
                "ACP stdio worker launch requires a non-empty bearer token",
            ));
        }
        if endpoint.trim().is_empty() {
            return Err(invalid_acp(
                "ACP stdio worker launch requires a non-empty endpoint",
            ));
        }
        if capability_fingerprint.trim().is_empty() {
            return Err(invalid_acp(
                "ACP stdio worker launch requires a capability fingerprint",
            ));
        }
        let mut worker_env = BTreeMap::new();
        worker_env.insert(profile.token_env().to_owned(), bearer_token.clone());
        worker_env.insert(profile.endpoint_env().to_owned(), endpoint.clone());
        worker_env.insert(
            profile.fingerprint_env().to_owned(),
            capability_fingerprint.clone(),
        );
        Ok(Self {
            transport: session.transport().to_owned(),
            engine_role: session.engine_role().to_owned(),
            worker_role: session.worker_role().to_owned(),
            token_env: profile.token_env().to_owned(),
            endpoint_env: profile.endpoint_env().to_owned(),
            fingerprint_env: profile.fingerprint_env().to_owned(),
            bearer_token,
            endpoint,
            capability_fingerprint,
            worker_env,
        })
    }

    /// Transport used by the worker launch.
    pub fn transport(&self) -> &str {
        &self.transport
    }

    /// ACP role of the Leaven engine.
    pub fn engine_role(&self) -> &str {
        &self.engine_role
    }

    /// ACP role of the external worker.
    pub fn worker_role(&self) -> &str {
        &self.worker_role
    }

    /// Environment passed to the worker process.
    pub fn worker_env(&self) -> &BTreeMap<String, String> {
        &self.worker_env
    }

    /// Artifact-safe launch facts. The bearer token is intentionally omitted.
    pub fn artifact_env(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            (self.endpoint_env.clone(), self.endpoint.clone()),
            (
                self.fingerprint_env.clone(),
                self.capability_fingerprint.clone(),
            ),
        ])
    }

    /// Rejects persisted launch facts that still contain the bearer token.
    pub fn validate_artifact_env(
        &self,
        artifact_env: &BTreeMap<String, String>,
    ) -> Result<(), PublicSeamError> {
        if artifact_env.contains_key(&self.token_env) {
            Err(invalid_acp(
                "ACP worker launch artifacts must not persist LEAVEN_CAPABILITY_TOKEN",
            ))
        } else if artifact_env
            .values()
            .any(|value| value.contains(&self.bearer_token))
        {
            Err(invalid_acp(
                "ACP worker launch artifacts must not persist the bearer secret value",
            ))
        } else {
            Ok(())
        }
    }
}

impl std::fmt::Debug for AcpStdioWorkerLaunch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut worker_env = self.worker_env.clone();
        if let Some(token) = worker_env.get_mut(&self.token_env) {
            "<redacted>".clone_into(token);
        }
        formatter
            .debug_struct("AcpStdioWorkerLaunch")
            .field("transport", &self.transport)
            .field("engine_role", &self.engine_role)
            .field("worker_role", &self.worker_role)
            .field("token_env", &self.token_env)
            .field("endpoint_env", &self.endpoint_env)
            .field("fingerprint_env", &self.fingerprint_env)
            .field("bearer_token", &"<redacted>")
            .field("endpoint", &self.endpoint)
            .field("capability_fingerprint", &self.capability_fingerprint)
            .field("worker_env", &worker_env)
            .finish()
    }
}

impl AcpWorkerSession {
    /// Starts a public-seam ACP worker session model from a validated profile.
    pub fn start(profile: &AcpProfileDocument) -> Result<Self, PublicSeamError> {
        let transport = profile
            .transports()
            .first()
            .filter(|transport| transport.as_str() == "stdio_jsonrpc")
            .ok_or_else(|| invalid_acp("ACP worker session must start on stdio_jsonrpc transport"))?
            .clone();
        Ok(Self {
            pinned_acp_version: profile.pinned_acp_version().to_owned(),
            transport,
            engine_role: "engine_client".to_owned(),
            worker_role: "worker_agent".to_owned(),
            lifecycle: AcpSessionLifecycle::from_profile(profile)?,
        })
    }

    /// Pinned ACP version used for this session.
    pub fn pinned_acp_version(&self) -> &str {
        &self.pinned_acp_version
    }

    /// Transport binding used to start this session.
    pub fn transport(&self) -> &str {
        &self.transport
    }

    /// ACP role of the Leaven engine.
    pub fn engine_role(&self) -> &str {
        &self.engine_role
    }

    /// ACP role of the external worker.
    pub fn worker_role(&self) -> &str {
        &self.worker_role
    }

    /// Lifecycle and progress-update state for this session.
    pub const fn lifecycle(&self) -> &AcpSessionLifecycle {
        &self.lifecycle
    }

    /// Mutable lifecycle and progress-update state for this session.
    pub fn lifecycle_mut(&mut self) -> &mut AcpSessionLifecycle {
        &mut self.lifecycle
    }
}

impl AcpPermissionRequest {
    /// Creates a permission request for a Leaven ACP extension method.
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            resource: BTreeMap::new(),
            case_fields: BTreeSet::new(),
            partition: None,
            input_classes: BTreeSet::new(),
            purposes: BTreeSet::new(),
            models: BTreeSet::new(),
            model_roles: BTreeSet::new(),
            workspace_ops: BTreeSet::new(),
            command: None,
            schemas: BTreeSet::new(),
            surface: None,
            limits: CapabilityLimitUsage::default(),
        }
    }

    /// Adds a resource selector to the permission request.
    #[must_use]
    pub fn with_resource(mut self, key: impl Into<String>, value: Value) -> Self {
        self.resource.insert(key.into(), value);
        self
    }

    /// Adds a requested case field.
    #[must_use]
    pub fn with_case_field(mut self, field: impl Into<String>) -> Self {
        self.case_fields.insert(field.into());
        self
    }

    /// Adds a requested case partition.
    #[must_use]
    pub fn with_partition(mut self, partition: impl Into<String>) -> Self {
        self.partition = Some(partition.into());
        self
    }

    /// Adds an input data class to the permission request.
    #[must_use]
    pub fn with_input_class(mut self, data_class: impl Into<String>) -> Self {
        self.input_classes.insert(data_class.into());
        self
    }

    /// Adds a requested operation purpose.
    #[must_use]
    pub fn with_purpose(mut self, purpose: impl Into<String>) -> Self {
        self.purposes.insert(purpose.into());
        self
    }

    /// Adds a requested model id.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.models.insert(model.into());
        self
    }

    /// Adds a requested model role.
    #[must_use]
    pub fn with_model_role(mut self, role: impl Into<String>) -> Self {
        self.model_roles.insert(role.into());
        self
    }

    /// Adds a requested workspace operation.
    #[must_use]
    pub fn with_workspace_op(mut self, operation: impl Into<String>) -> Self {
        self.workspace_ops.insert(operation.into());
        self
    }

    /// Sets the command requested by an agent or sandbox operation.
    #[must_use]
    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    /// Adds a schema fingerprint to the permission request.
    #[must_use]
    pub fn with_schema(mut self, schema: impl Into<String>) -> Self {
        self.schemas.insert(schema.into());
        self
    }

    /// Adds a surface fingerprint to the permission request.
    #[must_use]
    pub fn with_surface(mut self, surface: impl Into<String>) -> Self {
        self.surface = Some(surface.into());
        self
    }

    /// Adds per-operation usage checked against grant limits.
    #[must_use]
    pub fn with_limits(mut self, limits: CapabilityLimitUsage) -> Self {
        self.limits = limits;
        self
    }
}

/// Programmatic ACP permission decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpPermissionDecision {
    allowed: bool,
    capability_fingerprint: String,
    error: Option<Value>,
    redactions: Vec<Value>,
}

impl AcpPermissionDecision {
    /// Whether the extension call is authorized.
    pub const fn allowed(&self) -> bool {
        self.allowed
    }

    /// Capability fingerprint used for the decision.
    pub fn capability_fingerprint(&self) -> &str {
        &self.capability_fingerprint
    }

    /// Closed `PlanError` value returned for denials.
    pub fn error(&self) -> Option<&Value> {
        self.error.as_ref()
    }

    /// Redaction values returned for denials.
    pub fn redactions(&self) -> &[Value] {
        &self.redactions
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
}

pub fn authorize_permission(
    profile: &AcpProfileDocument,
    capability: &CapabilityDocument,
    session: &AcpAuthenticatedSession,
    request: AcpPermissionRequest,
) -> AcpPermissionDecision {
    if session.capability_fingerprint() != capability.capability_fingerprint() {
        return denied(
            session.capability_fingerprint(),
            "capability_denied",
            "ACP permission capability does not match authenticated session",
            Vec::new(),
        );
    }
    let Some(method) = profile.method(&request.method) else {
        return denied(
            capability.capability_fingerprint(),
            "extension_error",
            format!("unknown ACP extension method `{}`", request.method),
            Vec::new(),
        );
    };
    let mut grant = CapabilityGrantRequest::for_action(method.required_action.clone());
    for (key, value) in request.resource {
        grant = grant.with_resource(key, value);
    }
    for field in request.case_fields {
        grant = grant.with_case_field(field);
    }
    if let Some(partition) = request.partition {
        grant = grant.with_partition(partition);
    }
    for data_class in request.input_classes {
        grant = grant.with_input_class(data_class);
    }
    for purpose in request.purposes {
        grant = grant.with_purpose(purpose);
    }
    for model in request.models {
        grant = grant.with_model(model);
    }
    for model_role in request.model_roles {
        grant = grant.with_model_role(model_role);
    }
    for workspace_op in request.workspace_ops {
        grant = grant.with_workspace_op(workspace_op);
    }
    if let Some(command) = request.command {
        grant = grant.with_command(command);
    }
    for schema in request.schemas {
        grant = grant.with_schema(schema);
    }
    if let Some(surface) = request.surface {
        grant = grant.with_surface(surface);
    }
    grant = grant.with_limits(request.limits);
    match capability.authorize_grant(grant) {
        Ok(authorized) => AcpPermissionDecision {
            allowed: true,
            capability_fingerprint: authorized.capability_fingerprint().to_owned(),
            error: None,
            redactions: Vec::new(),
        },
        Err(denial) => denied(
            capability.capability_fingerprint(),
            "capability_denied",
            denial.to_string(),
            denial.redactions().to_vec(),
        ),
    }
}

pub fn authenticate(
    profile: &AcpProfileDocument,
    registry: &CapabilityRegistry,
    request: AcpAuthenticateRequest,
) -> Result<AcpAuthenticatedSession, PublicSeamError> {
    let AcpAuthenticateRequest {
        token_id,
        now,
        expected_capability_fingerprint,
    } = request;
    if profile.pinned_acp_version().trim().is_empty() {
        return Err(invalid_acp(
            "ACP profile must be validated before authenticate",
        ));
    }
    let document = registry
        .resolve_opaque_for_new_operation(&token_id, &now)
        .map_err(|error| invalid_acp(format!("ACP authenticate failed: {error}")))?;
    if expected_capability_fingerprint != document.capability_fingerprint() {
        return Err(invalid_acp(
            "ACP authenticate capability fingerprint binding mismatch",
        ));
    }
    Ok(AcpAuthenticatedSession {
        capability_fingerprint: document.capability_fingerprint().to_owned(),
        policy_fingerprint: document.policy_fingerprint().to_owned(),
        subject_fingerprint: document.subject_fingerprint().to_owned(),
        jti: document.jti().to_owned(),
    })
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
        let params_schema =
            required_string(entry.get("params_schema"), "params_schema")?.to_owned();
        if params_schema != "leaven.plan.v1.schema.json" {
            return Err(invalid_acp(format!(
                "extension method `{method}` params_schema must bind the locked Plan IR schema"
            )));
        }
        let result_schema =
            required_string(entry.get("result_schema"), "result_schema")?.to_owned();
        if result_schema != "leaven.plan_result.v1.schema.json" {
            return Err(invalid_acp(format!(
                "extension method `{method}` result_schema must bind the locked Plan Result schema"
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

fn locked_extension_methods() -> [&'static str; 25] {
    methods::locked_extension_methods()
}

fn required_action_for_method(method: &str) -> Option<&'static str> {
    methods::required_action_for_method(method)
}

fn denied(
    capability_fingerprint: &str,
    code: &str,
    message: impl Into<String>,
    redactions: Vec<String>,
) -> AcpPermissionDecision {
    let redactions = redactions
        .into_iter()
        .map(|data_class| {
            json!({
                "path": "",
                "reason": "data_class_forbidden",
                "public_reason": format!("data class `{data_class}` is not visible")
            })
        })
        .collect::<Vec<_>>();
    AcpPermissionDecision {
        allowed: false,
        capability_fingerprint: capability_fingerprint.to_owned(),
        error: Some(json!({
            "code": code,
            "message": message.into(),
            "retryable": false
        })),
        redactions,
    }
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

fn invalid_acp(message: impl Into<String>) -> PublicSeamError {
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
