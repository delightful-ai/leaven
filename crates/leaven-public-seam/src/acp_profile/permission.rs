use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::{AcpAuthenticatedSession, AcpProfileDocument, invalid_acp};
use crate::{
    CapabilityDocument, CapabilityGrantRequest, CapabilityLimitUsage, CapabilityRegistry,
    PublicSeamError,
};

/// ACP permission request projected to a capability grant check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpPermissionRequest {
    pub(super) method: String,
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
    let mut grant = CapabilityGrantRequest::for_action(method.required_action().to_owned());
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
    request: super::AcpAuthenticateRequest,
) -> Result<AcpAuthenticatedSession, PublicSeamError> {
    let (token_id, now, expected_capability_fingerprint) = request.into_parts();
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
    Ok(AcpAuthenticatedSession::new(
        document.capability_fingerprint(),
        document.policy_fingerprint(),
        document.subject_fingerprint(),
        document.jti(),
    ))
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
