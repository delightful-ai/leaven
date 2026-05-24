use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::{Value, json};

use crate::{
    CapabilityDocument, CapabilityGrantRequest, CapabilityLimitUsage, CapabilityRegistry,
    PublicSeamError,
    plan_error::{is_closed_plan_error_code, receipt_ref_id},
    plan_execution::{validate_agent_session_value, validate_sandbox_exec_value},
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

/// Bounded ACP progress-update queue plus session cancellation state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpSessionLifecycle {
    max_inflight_updates: usize,
    backpressure: AcpBackpressure,
    next_sequence: u64,
    updates: VecDeque<AcpSessionUpdate>,
    cancellation: Option<AcpSessionCancellation>,
    state: AcpSessionState,
}

impl AcpSessionLifecycle {
    /// Builds a bounded ACP update lifecycle from the validated profile.
    pub fn from_profile(profile: &AcpProfileDocument) -> Result<Self, PublicSeamError> {
        let max_inflight_updates = usize::try_from(profile.default_max_inflight_updates())
            .map_err(|_| invalid_acp("ACP max inflight updates does not fit this platform"))?;
        Self::bounded(max_inflight_updates, profile.backpressure())
    }

    fn bounded(
        max_inflight_updates: usize,
        backpressure: AcpBackpressure,
    ) -> Result<Self, PublicSeamError> {
        if max_inflight_updates == 0 {
            return Err(invalid_acp("ACP update queue bound must be non-zero"));
        }
        Ok(Self {
            max_inflight_updates,
            backpressure,
            next_sequence: 0,
            updates: VecDeque::new(),
            cancellation: None,
            state: AcpSessionState::Running,
        })
    }

    /// Maximum in-flight progress updates allowed before backpressure is applied.
    pub const fn max_inflight_updates(&self) -> usize {
        self.max_inflight_updates
    }

    /// Backpressure strategy governing the bounded update queue.
    pub const fn backpressure(&self) -> AcpBackpressure {
        self.backpressure
    }

    /// Current number of queued progress updates.
    pub fn inflight_updates(&self) -> usize {
        self.updates.len()
    }

    /// Whether ACP session cancellation has been requested.
    pub const fn is_cancelled(&self) -> bool {
        matches!(self.state, AcpSessionState::Cancelled)
    }

    /// Current session lifecycle state.
    pub const fn state(&self) -> AcpSessionState {
        self.state
    }

    /// Cancellation facts, when the session has been cancelled.
    pub const fn cancellation(&self) -> Option<&AcpSessionCancellation> {
        self.cancellation.as_ref()
    }

    /// Enqueues one ACP progress update or returns bounded-queue backpressure.
    pub fn enqueue_progress(
        &mut self,
        message: impl Into<String>,
    ) -> Result<&AcpSessionUpdate, PublicSeamError> {
        match self.offer_progress(message, AcpProgressPriority::Critical)? {
            AcpProgressDisposition::Enqueued(_) => Ok(self
                .updates
                .back()
                .expect("enqueued critical update must be observable")),
            AcpProgressDisposition::DroppedNoncritical => Err(invalid_acp(
                "ACP critical progress update cannot be dropped as noncritical",
            )),
            AcpProgressDisposition::Disconnected(reason) => Err(invalid_acp(reason)),
        }
    }

    /// Offers one progress update with explicit priority.
    pub fn offer_progress(
        &mut self,
        message: impl Into<String>,
        priority: AcpProgressPriority,
    ) -> Result<AcpProgressDisposition, PublicSeamError> {
        if self.is_cancelled() {
            return Err(invalid_acp(
                "ACP session updates are refused after session cancellation",
            ));
        }
        if self.updates.len() >= self.max_inflight_updates {
            return match self.backpressure {
                AcpBackpressure::PauseWorker => Err(invalid_acp(
                    "ACP session update queue is full; worker must pause",
                )),
                AcpBackpressure::DropNoncriticalUpdates
                    if priority == AcpProgressPriority::Noncritical =>
                {
                    Ok(AcpProgressDisposition::DroppedNoncritical)
                }
                AcpBackpressure::DropNoncriticalUpdates => Err(invalid_acp(
                    "ACP session update queue is full; worker must pause critical updates",
                )),
                AcpBackpressure::Disconnect => {
                    let reason = "ACP session disconnected after update overflow";
                    let receipt = format!("acprec_disconnect_{}", self.next_sequence);
                    let error = cancellation_plan_error(&receipt, "cancelled", reason);
                    let cancellation = self.cancel_with_error(reason, receipt, error)?;
                    Ok(AcpProgressDisposition::Disconnected(
                        cancellation.reason().to_owned(),
                    ))
                }
            };
        }
        let update = AcpSessionUpdate {
            sequence: self.next_sequence,
            message: message.into(),
        };
        self.next_sequence += 1;
        self.updates.push_back(update);
        Ok(AcpProgressDisposition::Enqueued(
            self.updates
                .back()
                .expect("pushed update must be observable")
                .clone(),
        ))
    }

    /// Acknowledges the oldest in-flight progress update.
    pub fn acknowledge_oldest_update(&mut self) -> Option<AcpSessionUpdate> {
        self.updates.pop_front()
    }

    /// Cancels the ACP session with an auditable receipt and closed `PlanError`.
    pub fn cancel_with_error(
        &mut self,
        reason: impl Into<String>,
        receipt: impl Into<String>,
        error: Value,
    ) -> Result<&AcpSessionCancellation, PublicSeamError> {
        if self.cancellation.is_none() {
            let receipt = receipt.into();
            validate_cancellation_error(&receipt, &error)?;
            self.cancellation = Some(AcpSessionCancellation {
                reason: reason.into(),
                receipt,
                error,
            });
            self.state = AcpSessionState::Cancelled;
        }
        Ok(self
            .cancellation
            .as_ref()
            .expect("cancellation set before return"))
    }
}

/// Priority of one ACP progress update under backpressure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcpProgressPriority {
    /// Critical updates must be delivered or force producer backpressure.
    Critical,
    /// Noncritical updates may be dropped when the profile allows it.
    Noncritical,
}

/// Result of offering one ACP progress update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcpProgressDisposition {
    /// The update entered the bounded queue.
    Enqueued(AcpSessionUpdate),
    /// The profile dropped a noncritical update at the queue boundary.
    DroppedNoncritical,
    /// The profile disconnected the session at the queue boundary.
    Disconnected(String),
}

/// Profile-level ACP session lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcpSessionState {
    /// The worker session accepts progress updates.
    Running,
    /// ACP cancellation has been requested for the session.
    Cancelled,
}

/// One ACP session progress update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpSessionUpdate {
    sequence: u64,
    message: String,
}

impl AcpSessionUpdate {
    /// Monotone sequence number within one session.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Human-readable progress update.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// ACP session cancellation facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpSessionCancellation {
    reason: String,
    receipt: String,
    error: Value,
}

impl AcpSessionCancellation {
    /// Cancellation reason.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Receipt that audits the ACP session cancellation.
    pub fn receipt(&self) -> &str {
        &self.receipt
    }

    /// Closed `PlanError` associated with the ACP session cancellation.
    pub const fn error(&self) -> &Value {
        &self.error
    }
}

fn cancellation_plan_error(receipt: &str, code: &str, message: &str) -> Value {
    json!({
        "code": code,
        "message": message,
        "receipt": receipt
    })
}

fn validate_cancellation_error(receipt: &str, error: &Value) -> Result<(), PublicSeamError> {
    if receipt.trim().is_empty() {
        return Err(invalid_acp("ACP cancellation receipt must be non-empty"));
    }
    let object = error
        .as_object()
        .ok_or_else(|| invalid_acp("ACP cancellation error must be a PlanError object"))?;
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "code" | "message" | "op" | "path" | "receipt" | "retryable" | "details"
        ) {
            return Err(invalid_acp(format!(
                "ACP cancellation error carries unknown PlanError field `{key}`"
            )));
        }
    }
    let code = required_string(object.get("code"), "cancellation error code")?;
    if !is_closed_plan_error_code(code) {
        return Err(invalid_acp(
            "ACP cancellation error code must be a closed PlanError code",
        ));
    }
    let message = object
        .get("message")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_acp("ACP cancellation error must carry message"))?;
    if message.trim().is_empty() {
        return Err(invalid_acp(
            "ACP cancellation error must carry non-empty message",
        ));
    }
    let error_receipt = object
        .get("receipt")
        .ok_or_else(|| invalid_acp("ACP cancellation error receipt must be present"))
        .and_then(|receipt| {
            receipt_ref_id(receipt, "ACP cancellation error receipt").map_err(invalid_acp)
        })?;
    if error_receipt != receipt {
        return Err(invalid_acp(
            "ACP cancellation error receipt must match cancellation receipt",
        ));
    }
    Ok(())
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

/// Leaven ACP extension result envelope with public-seam receipt facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpExtensionResultDocument {
    method: String,
    primary_kind: String,
    capability_fingerprint: String,
    receipt_count: usize,
    redaction_count: usize,
    data_classes: Vec<String>,
}

impl AcpExtensionResultDocument {
    pub(crate) fn from_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_acp("ACP extension result must be an object"))?;
        let method = required_string(object.get("method"), "method")?.to_owned();
        if !method.starts_with("leaven/") || method.contains("mcp") {
            return Err(invalid_acp(
                "ACP extension result method must be Leaven-only",
            ));
        }
        let receipts = required_array(object.get("receipts"), "receipts")?;
        if receipts.is_empty() {
            return Err(invalid_acp("ACP extension result must carry receipts"));
        }
        let redactions = required_array(object.get("redactions"), "redactions")?;
        let data_classes = string_array(object.get("data_classes"), "data_classes")?;
        if data_classes.is_empty() {
            return Err(invalid_acp("ACP extension result must carry data classes"));
        }
        let primary_value = object
            .get("primary")
            .ok_or_else(|| invalid_acp("ACP extension result must carry primary object"))?;
        let primary = primary_value
            .as_object()
            .ok_or_else(|| invalid_acp("ACP extension result must carry primary object"))?;
        let primary_kind = required_string(primary.get("kind"), "primary.kind")?.to_owned();
        validate_primary_kind(&method, &primary_kind)?;
        if let Some(primary_data_classes) = primary.get("data_classes") {
            for data_class in string_array(Some(primary_data_classes), "primary.data_classes")? {
                if !data_classes.contains(&data_class) {
                    return Err(invalid_acp(format!(
                        "ACP extension result data_classes must cover primary data class `{data_class}`"
                    )));
                }
            }
        }
        validate_receipts_for_method(&method, receipts)?;
        validate_primary_result_hash(&method, primary_value, receipts)?;
        let expected_receipt = expected_receipt_for_method(&method, receipts)?;
        validate_effect_primary_audit(&method, primary, expected_receipt)?;
        if let Some(primary_receipt) = primary.get("receipt").and_then(Value::as_str) {
            ensure_primary_receipt_is_carried(primary_receipt, receipts)?;
        }
        Ok(Self {
            method,
            primary_kind,
            capability_fingerprint: required_string(
                object.get("capability_fingerprint"),
                "capability_fingerprint",
            )?
            .to_owned(),
            receipt_count: receipts.len(),
            redaction_count: redactions.len(),
            data_classes,
        })
    }

    pub(crate) fn synthetic_plan_result(value: &Value) -> Result<Value, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_acp("ACP extension result must be an object"))?;
        let primary = object
            .get("primary")
            .ok_or_else(|| invalid_acp("ACP extension result must carry primary value"))?;
        let primary_object = primary
            .as_object()
            .ok_or_else(|| invalid_acp("ACP extension primary must be an object"))?;
        let graph_revision = primary_object
            .get("graph_revision")
            .and_then(Value::as_str)
            .unwrap_or("rev_acp_extension_result");
        let replayability = primary_object
            .get("replayability")
            .and_then(Value::as_str)
            .unwrap_or("fully_managed");
        Ok(json!({
            "schema_version": "leaven.plan_result.v1",
            "plan_id": "acp_extension_result",
            "capability_fingerprint": required_string(
                object.get("capability_fingerprint"),
                "capability_fingerprint",
            )?,
            "policy_fingerprint": object
                .get("policy_fingerprint")
                .and_then(Value::as_str)
                .unwrap_or("fp_policy_sha256_acp_extension"),
            "base_revision": graph_revision,
            "final_revision": graph_revision,
            "replayability_summary": replayability,
            "values": {
                "primary": primary
            },
            "receipts": object
                .get("receipts")
                .ok_or_else(|| invalid_acp("ACP extension result must carry receipts"))?,
            "redactions": object
                .get("redactions")
                .ok_or_else(|| invalid_acp("ACP extension result must carry redactions"))?,
            "charges": [],
            "errors": []
        }))
    }

    /// Extension method name.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Primary result value kind.
    pub fn primary_kind(&self) -> &str {
        &self.primary_kind
    }

    /// Capability fingerprint attached to the result.
    pub fn capability_fingerprint(&self) -> &str {
        &self.capability_fingerprint
    }

    /// Number of receipts carried by the result.
    pub const fn receipt_count(&self) -> usize {
        self.receipt_count
    }

    /// Number of redactions carried by the result.
    pub const fn redaction_count(&self) -> usize {
        self.redaction_count
    }

    /// Data classes carried by the result.
    pub fn data_classes(&self) -> &[String] {
        &self.data_classes
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReceiptExpectation {
    Query,
    Call(&'static str),
    Write(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExtensionResultContract {
    primary_kinds: &'static [&'static str],
    receipt: ReceiptExpectation,
}

fn extension_result_contract(method: &str) -> Result<ExtensionResultContract, PublicSeamError> {
    const EXTENSION: &[&str] = &["extension"];
    const WORKSPACE_READ: &[&str] = &[
        "workspace_file",
        "workspace_diff",
        "workspace_listing",
        "workspace_snapshot",
        "extension",
    ];
    match method {
        "leaven/graph.query"
        | "leaven/case.load"
        | "leaven/case.input"
        | "leaven/case.target"
        | "leaven/case.metadata" => Ok(ExtensionResultContract {
            primary_kinds: EXTENSION,
            receipt: ReceiptExpectation::Query,
        }),
        "leaven/workspace.materialize" => Ok(ExtensionResultContract {
            primary_kinds: &["workspace_handle"],
            receipt: ReceiptExpectation::Call("workspace_materialize"),
        }),
        "leaven/workspace.snapshot" => Ok(ExtensionResultContract {
            primary_kinds: &["workspace_snapshot"],
            receipt: ReceiptExpectation::Query,
        }),
        "leaven/workspace.list"
        | "leaven/workspace.read_file"
        | "leaven/workspace.stat"
        | "leaven/workspace.digest"
        | "leaven/workspace.git_log"
        | "leaven/workspace.git_diff"
        | "leaven/workspace.git_status"
        | "leaven/workspace.capture_artifacts" => Ok(ExtensionResultContract {
            primary_kinds: WORKSPACE_READ,
            receipt: ReceiptExpectation::Query,
        }),
        "leaven/workspace.release" => Ok(ExtensionResultContract {
            primary_kinds: EXTENSION,
            receipt: ReceiptExpectation::Call("workspace_release"),
        }),
        "leaven/lm.complete" => Ok(ExtensionResultContract {
            primary_kinds: &["lm_response"],
            receipt: ReceiptExpectation::Call("lm_complete"),
        }),
        "leaven/agent.run" => Ok(ExtensionResultContract {
            primary_kinds: &["agent_session"],
            receipt: ReceiptExpectation::Call("agent_run"),
        }),
        "leaven/sandbox.exec" => Ok(ExtensionResultContract {
            primary_kinds: &["sandbox_exec"],
            receipt: ReceiptExpectation::Call("sandbox_exec"),
        }),
        "leaven/human.review" => Ok(ExtensionResultContract {
            primary_kinds: EXTENSION,
            receipt: ReceiptExpectation::Call("human_review"),
        }),
        "leaven/proposal.submit_batch" => Ok(ExtensionResultContract {
            primary_kinds: &["proposal_batch_receipt"],
            receipt: ReceiptExpectation::Write("submit_proposal_batch"),
        }),
        "leaven/proposal.apply" => Ok(ExtensionResultContract {
            primary_kinds: &["apply_receipt"],
            receipt: ReceiptExpectation::Write("apply_proposal_batch"),
        }),
        "leaven/assessment.submit" => Ok(ExtensionResultContract {
            primary_kinds: &["assessment_batch_receipt"],
            receipt: ReceiptExpectation::Write("submit_assessments"),
        }),
        "leaven/evaluation.request" => Ok(ExtensionResultContract {
            primary_kinds: &["evaluation_request_receipt"],
            receipt: ReceiptExpectation::Write("request_evaluation"),
        }),
        "leaven/event.emit" => Ok(ExtensionResultContract {
            primary_kinds: EXTENSION,
            receipt: ReceiptExpectation::Write("emit_run_event"),
        }),
        _ => Err(invalid_acp(format!(
            "ACP extension result method `{method}` is not in the locked profile"
        ))),
    }
}

fn validate_primary_kind(method: &str, primary_kind: &str) -> Result<(), PublicSeamError> {
    let contract = extension_result_contract(method)?;
    if contract.primary_kinds.contains(&primary_kind) {
        Ok(())
    } else {
        Err(invalid_acp(format!(
            "ACP extension result method `{method}` cannot return primary kind `{primary_kind}`"
        )))
    }
}

fn validate_receipts_for_method(method: &str, receipts: &[Value]) -> Result<(), PublicSeamError> {
    let contract = extension_result_contract(method)?;
    if receipts
        .iter()
        .any(|receipt| receipt_matches(receipt, contract.receipt))
    {
        Ok(())
    } else {
        Err(invalid_acp(format!(
            "ACP extension result method `{method}` is missing its expected receipt"
        )))
    }
}

fn validate_primary_result_hash(
    method: &str,
    primary: &Value,
    receipts: &[Value],
) -> Result<(), PublicSeamError> {
    let receipt = expected_receipt_for_method(method, receipts)?;
    let schema_version = match required_string(receipt.get("kind"), "receipt.kind")? {
        "query" => "leaven.plan_query_result.v1",
        "call" => "leaven.plan_call_result.v1",
        "write" => "leaven.plan_write_result.v1",
        other => {
            return Err(invalid_acp(format!(
                "ACP extension result receipt kind `{other}` cannot bind primary value"
            )));
        }
    };
    let op_name = receipt
        .get("op_var")
        .and_then(Value::as_str)
        .unwrap_or("primary");
    let expected = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": schema_version,
            "name": op_name,
            "value": primary
        }),
    )?;
    let actual = required_string(receipt.get("result_hash"), "receipt.result_hash")?;
    if actual != expected {
        let receipt_id = required_string(receipt.get("receipt"), "receipt.receipt")?;
        return Err(invalid_acp(format!(
            "ACP extension result receipt `{receipt_id}` result_hash does not bind primary value"
        )));
    }
    Ok(())
}

fn validate_effect_primary_audit(
    method: &str,
    primary: &serde_json::Map<String, Value>,
    expected_receipt: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let expected_receipt_id = required_string(expected_receipt.get("receipt"), "receipt.receipt")?;
    match method {
        "leaven/lm.complete" => {
            validate_effect_primary_receipt(primary, expected_receipt_id)?;
            validate_effect_primary_cost("lm_complete", primary, expected_receipt)
        }
        "leaven/agent.run" => {
            validate_effect_primary_receipt(primary, expected_receipt_id)?;
            validate_agent_session_value("agent_run", primary, expected_receipt_id)?;
            validate_effect_primary_cost("agent_run", primary, expected_receipt)
        }
        "leaven/sandbox.exec" => {
            validate_effect_primary_receipt(primary, expected_receipt_id)?;
            validate_sandbox_exec_value("sandbox_exec", primary)?;
            validate_effect_primary_cost("sandbox_exec", primary, expected_receipt)
        }
        _ => Ok(()),
    }
}

fn validate_effect_primary_receipt(
    primary: &serde_json::Map<String, Value>,
    expected_receipt_id: &str,
) -> Result<(), PublicSeamError> {
    let primary_receipt = required_string(primary.get("receipt"), "primary.receipt")?;
    if primary_receipt == expected_receipt_id {
        Ok(())
    } else {
        Err(invalid_acp(format!(
            "ACP extension result primary receipt `{primary_receipt}` does not match expected receipt `{expected_receipt_id}`"
        )))
    }
}

fn validate_effect_primary_cost(
    call_kind: &str,
    primary: &serde_json::Map<String, Value>,
    receipt: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    match (primary.get("cost"), receipt.get("cost")) {
        (Some(primary_cost), Some(receipt_cost)) if primary_cost == receipt_cost => Ok(()),
        (Some(_), _) => Err(invalid_acp(format!(
            "ACP extension result {call_kind} primary cost must match call receipt cost"
        ))),
        (None, Some(_)) => Err(invalid_acp(format!(
            "ACP extension result {call_kind} call receipt cost must have a matching primary cost"
        ))),
        (None, None) => Err(invalid_acp(format!(
            "ACP extension result {call_kind} primary must carry cost"
        ))),
    }
}

fn expected_receipt_for_method<'a>(
    method: &str,
    receipts: &'a [Value],
) -> Result<&'a serde_json::Map<String, Value>, PublicSeamError> {
    let expectation = extension_result_contract(method)?.receipt;
    receipts
        .iter()
        .find(|receipt| receipt_matches(receipt, expectation))
        .and_then(Value::as_object)
        .ok_or_else(|| {
            invalid_acp(format!(
                "ACP extension result method `{method}` is missing its expected receipt"
            ))
        })
}

fn receipt_matches(receipt: &Value, expectation: ReceiptExpectation) -> bool {
    let Some(object) = receipt.as_object() else {
        return false;
    };
    match expectation {
        ReceiptExpectation::Query => object.get("kind").and_then(Value::as_str) == Some("query"),
        ReceiptExpectation::Call(call_kind) => {
            object.get("kind").and_then(Value::as_str) == Some("call")
                && object.get("call_kind").and_then(Value::as_str) == Some(call_kind)
        }
        ReceiptExpectation::Write(write_kind) => {
            object.get("kind").and_then(Value::as_str) == Some("write")
                && object.get("write_kind").and_then(Value::as_str) == Some(write_kind)
        }
    }
}

fn ensure_primary_receipt_is_carried(
    primary_receipt: &str,
    receipts: &[Value],
) -> Result<(), PublicSeamError> {
    if receipts.iter().any(|receipt| {
        receipt
            .as_object()
            .and_then(|object| object.get("receipt"))
            .and_then(Value::as_str)
            == Some(primary_receipt)
    }) {
        Ok(())
    } else {
        Err(invalid_acp(format!(
            "ACP extension result primary receipt `{primary_receipt}` is not carried"
        )))
    }
}

fn locked_extension_methods() -> [&'static str; 25] {
    [
        "leaven/graph.query",
        "leaven/case.load",
        "leaven/case.input",
        "leaven/case.target",
        "leaven/case.metadata",
        "leaven/workspace.materialize",
        "leaven/workspace.snapshot",
        "leaven/workspace.list",
        "leaven/workspace.read_file",
        "leaven/workspace.stat",
        "leaven/workspace.digest",
        "leaven/workspace.git_log",
        "leaven/workspace.git_diff",
        "leaven/workspace.git_status",
        "leaven/workspace.capture_artifacts",
        "leaven/workspace.release",
        "leaven/lm.complete",
        "leaven/agent.run",
        "leaven/sandbox.exec",
        "leaven/human.review",
        "leaven/proposal.submit_batch",
        "leaven/proposal.apply",
        "leaven/assessment.submit",
        "leaven/evaluation.request",
        "leaven/event.emit",
    ]
}

fn required_action_for_method(method: &str) -> Option<&'static str> {
    match method {
        "leaven/graph.query" => Some("graph.query"),
        "leaven/case.load"
        | "leaven/case.input"
        | "leaven/case.target"
        | "leaven/case.metadata" => Some("case.read"),
        "leaven/workspace.materialize" => Some("workspace.materialize"),
        "leaven/workspace.snapshot"
        | "leaven/workspace.list"
        | "leaven/workspace.read_file"
        | "leaven/workspace.stat"
        | "leaven/workspace.digest"
        | "leaven/workspace.git_log"
        | "leaven/workspace.git_diff"
        | "leaven/workspace.git_status"
        | "leaven/workspace.capture_artifacts" => Some("workspace.read"),
        "leaven/workspace.release" => Some("workspace.release"),
        "leaven/lm.complete" => Some("lm.complete"),
        "leaven/agent.run" => Some("agent.run"),
        "leaven/sandbox.exec" => Some("sandbox.exec"),
        "leaven/human.review" => Some("human.review"),
        "leaven/proposal.submit_batch" => Some("proposal.submit_batch"),
        "leaven/proposal.apply" => Some("proposal.apply_batch"),
        "leaven/assessment.submit" => Some("assessment.submit"),
        "leaven/evaluation.request" => Some("evaluation.request"),
        "leaven/event.emit" => Some("event.emit"),
        _ => None,
    }
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
