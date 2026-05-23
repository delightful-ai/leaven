use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::{
    CapabilityDocument, CapabilityGrantRequest, CapabilityLimitUsage, CapabilityRegistry,
    PublicSeamError,
};

/// Schema-valid Leaven ACP profile document with V1 semantic checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpProfileDocument {
    pinned_acp_version: String,
    transports: Vec<String>,
    extension_methods: Vec<AcpExtensionMethod>,
    default_max_inflight_updates: u64,
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

        Ok(Self {
            pinned_acp_version,
            transports,
            extension_methods,
            default_max_inflight_updates,
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

    /// Leaven ACP extension methods.
    pub fn extension_methods(&self) -> &[AcpExtensionMethod] {
        &self.extension_methods
    }

    /// Bounded update queue capacity advertised by the profile.
    pub const fn default_max_inflight_updates(&self) -> u64 {
        self.default_max_inflight_updates
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
        let primary = object
            .get("primary")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_acp("ACP extension result must carry primary object"))?;
        let primary_kind = required_string(primary.get("kind"), "primary.kind")?.to_owned();
        validate_primary_kind(&method, &primary_kind)?;
        let primary_data_classes =
            string_array(primary.get("data_classes"), "primary.data_classes")?;
        for data_class in &primary_data_classes {
            if !data_classes.contains(data_class) {
                return Err(invalid_acp(format!(
                    "ACP extension result data_classes must cover primary data class `{data_class}`"
                )));
            }
        }
        validate_receipts_for_method(&method, receipts)?;
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
        output.push(AcpExtensionMethod {
            method,
            params_schema: required_string(entry.get("params_schema"), "params_schema")?.to_owned(),
            result_schema: required_string(entry.get("result_schema"), "result_schema")?.to_owned(),
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
