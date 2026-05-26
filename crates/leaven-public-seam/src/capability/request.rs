use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::{CapabilityError, CapabilityLimitUsage};

/// Requested operation dimensions checked against a capability grant.
#[derive(Clone, Debug, Default)]
pub struct CapabilityGrantRequest {
    pub(super) action: String,
    pub(super) resource: BTreeMap<String, Value>,
    pub(super) case_fields: BTreeSet<String>,
    pub(super) partition: Option<String>,
    pub(super) input_classes: BTreeSet<String>,
    pub(super) purposes: BTreeSet<String>,
    pub(super) models: BTreeSet<String>,
    pub(super) model_roles: BTreeSet<String>,
    pub(super) workspace_ops: BTreeSet<String>,
    pub(super) commands: BTreeSet<String>,
    pub(super) schemas: BTreeSet<String>,
    pub(super) surface: Option<String>,
    pub(super) limits: CapabilityLimitUsage,
}

impl CapabilityGrantRequest {
    /// Starts a request for a capability action.
    pub fn for_action(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            ..Self::default()
        }
    }

    /// Adds a resource selector value.
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

    /// Sets a requested data partition.
    #[must_use]
    pub fn with_partition(mut self, partition: impl Into<String>) -> Self {
        self.partition = Some(partition.into());
        self
    }

    /// Adds an input data class.
    #[must_use]
    pub fn with_input_class(mut self, data_class: impl Into<String>) -> Self {
        self.input_classes.insert(data_class.into());
        self
    }

    /// Adds a purpose constraint.
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

    /// Adds a model role constraint.
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
        self.commands.insert(command.into());
        self
    }

    /// Adds a schema fingerprint used by the operation.
    #[must_use]
    pub fn with_schema(mut self, schema: impl Into<String>) -> Self {
        self.schemas.insert(schema.into());
        self
    }

    /// Sets the surface fingerprint used by the operation.
    #[must_use]
    pub fn with_surface(mut self, surface: impl Into<String>) -> Self {
        self.surface = Some(surface.into());
        self
    }

    /// Sets the requested limit usage for this operation.
    #[must_use]
    pub fn with_limits(mut self, limits: CapabilityLimitUsage) -> Self {
        self.limits = limits;
        self
    }
}

/// Capability denial category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityDenialKind {
    /// No grant allows the action.
    Action,
    /// Resource selectors do not fit the grant.
    Resource,
    /// Requested partition is outside the grant.
    Partition,
    /// Requested case field is outside the grant or explicitly forbidden.
    CaseField,
    /// Requested schema fingerprint is outside the grant.
    Schema,
    /// Requested surface fingerprint is outside the grant.
    Surface,
    /// Data class is outside the grant or explicitly forbidden.
    DataClass,
    /// Requested usage exceeds grant limits.
    Limit,
    /// Delegated capability widens parent authority.
    Delegation,
}

/// Typed capability denial with redaction facts.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("capability denied by {kind:?}: {message}")]
pub struct CapabilityDenial {
    kind: CapabilityDenialKind,
    message: String,
    redactions: Vec<String>,
}

impl CapabilityDenial {
    pub(super) fn new(kind: CapabilityDenialKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            redactions: Vec::new(),
        }
    }

    pub(super) fn with_redactions(
        kind: CapabilityDenialKind,
        message: impl Into<String>,
        redactions: Vec<String>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            redactions,
        }
    }

    pub(super) fn from_invalid_document(error: &CapabilityError) -> Self {
        Self::new(CapabilityDenialKind::Delegation, error.to_string())
    }

    /// Denial category.
    pub fn kind(&self) -> CapabilityDenialKind {
        self.kind
    }

    /// Data classes redacted by the denial.
    pub fn redactions(&self) -> &[String] {
        &self.redactions
    }
}
