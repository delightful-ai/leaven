use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use jsonschema::{Retrieve, Uri};
use serde::Deserialize;
use serde_json::Value;

mod budget;
mod delegation;
mod grant;
mod registry;

pub use budget::{
    CapabilityBudgetLedger, CapabilityBudgetProjectionError, CapabilityBudgetReservation,
    CapabilityBudgetUsage,
};
pub use delegation::CapabilityDelegation;
pub use grant::{AuthorizedGrant, CapabilityLimitUsage};
pub use registry::CapabilityRegistry;

const ACTIVE_PACKAGE_RELATIVE: &str = "docs/specs/public-seam-v1";
const CAPABILITY_SCHEMA: &str = "leaven.capability.v1.schema.json";

/// Structured authority document resolved from a public-seam bearer token.
#[derive(Clone, Debug, Deserialize)]
pub struct CapabilityDocument {
    schema_version: String,
    jti: String,
    capability_fingerprint: String,
    policy_fingerprint: String,
    subject_fingerprint: String,
    #[serde(default)]
    grant_fingerprint: Option<String>,
    #[serde(default)]
    parent_capability_fingerprint: Option<String>,
    issuer: Issuer,
    subject: Value,
    audience: Vec<String>,
    issued_at: String,
    token_binding: TokenBinding,
    expires_at: String,
    expiry_behavior: ExpiryBehavior,
    revocation: RevocationPolicy,
    renewal: RenewalPolicy,
    budgets: AggregateBudgets,
    execution_policy: ExecutionPolicy,
    grants: Vec<Grant>,
    delegation: DelegationPolicy,
}

impl CapabilityDocument {
    /// Parses and semantically checks a capability document value.
    pub fn from_value(value: Value) -> Result<Self, CapabilityError> {
        validate_capability_schema(&value)?;
        let document = serde_json::from_value::<Self>(value).map_err(|error| {
            CapabilityError::InvalidDocument {
                message: error.to_string(),
            }
        })?;
        document.validate()?;
        Ok(document)
    }

    /// JWT ID.
    pub fn jti(&self) -> &str {
        &self.jti
    }

    /// Subject fingerprint bound to the authority.
    pub fn subject_fingerprint(&self) -> &str {
        &self.subject_fingerprint
    }

    /// Capability fingerprint.
    pub fn capability_fingerprint(&self) -> &str {
        &self.capability_fingerprint
    }

    /// Policy fingerprint.
    pub fn policy_fingerprint(&self) -> &str {
        &self.policy_fingerprint
    }

    /// Opaque token id when this document uses opaque lookup binding.
    pub fn opaque_token_id(&self) -> Option<&str> {
        match &self.token_binding {
            TokenBinding::OpaqueLookup { token_id, .. } => Some(token_id),
            _ => None,
        }
    }

    /// Expiry timestamp.
    pub fn expires_at(&self) -> &str {
        &self.expires_at
    }

    /// Revocation mode.
    pub fn revocation_mode(&self) -> Option<&str> {
        Some(self.revocation.mode.as_str())
    }

    /// Renewal mode.
    pub fn renewal_mode(&self) -> Option<&str> {
        Some(self.renewal.mode.as_str())
    }

    /// Aggregate total budget in USD micro-units.
    pub fn max_total_usd_micro(&self) -> Option<u64> {
        self.budgets.total_usd_micro
    }

    /// Aggregate LM budget in USD micro-units.
    pub fn max_lm_usd_micro(&self) -> Option<u64> {
        self.budgets.lm_usd_micro
    }

    /// Aggregate agent budget in USD micro-units.
    pub fn max_agent_usd_micro(&self) -> Option<u64> {
        self.budgets.agent_usd_micro
    }

    /// Aggregate concurrent-call limit.
    pub fn max_concurrent_calls(&self) -> Option<u64> {
        self.budgets.concurrent_calls
    }

    /// Aggregate human-review budget in USD micro-units.
    pub fn max_human_usd_micro(&self) -> Option<u64> {
        self.budgets.human_usd_micro
    }

    /// Aggregate wall-clock budget in milliseconds.
    pub fn max_wall_ms(&self) -> Option<u64> {
        self.budgets.wall_ms
    }

    /// Aggregate plan-node budget.
    pub fn max_plan_nodes(&self) -> Option<u64> {
        self.budgets.plan_nodes
    }

    /// Aggregate materialized-byte budget.
    pub fn max_materialized_bytes(&self) -> Option<u64> {
        self.budgets.materialized_bytes
    }

    /// Issuer kind.
    pub fn issuer_kind(&self) -> &str {
        &self.issuer.kind
    }

    /// Audience strings.
    pub fn audience(&self) -> &[String] {
        &self.audience
    }

    pub(crate) fn subject_stage_role(&self) -> Option<&str> {
        self.subject
            .as_object()
            .filter(|subject| subject.get("kind").and_then(Value::as_str) == Some("stage_call"))
            .and_then(|subject| subject.get("role"))
            .and_then(Value::as_str)
    }

    /// Execution policy profile.
    pub fn execution_policy_profile(&self) -> &str {
        &self.execution_policy.profile
    }

    /// Delegation allowed action strings.
    pub fn delegation_allowed_actions(&self) -> &[String] {
        &self.delegation.allowed_actions
    }

    /// Returns the first grant with the requested action.
    pub fn grant(&self, action: &str) -> Option<&Grant> {
        self.grants.iter().find(|grant| grant.action == action)
    }

    /// Grant action strings.
    pub fn grant_actions(&self) -> impl Iterator<Item = &str> {
        self.grants.iter().map(|grant| grant.action.as_str())
    }

    /// Authorizes a requested operation against the document's grant envelope.
    pub fn authorize_grant(
        &self,
        request: CapabilityGrantRequest,
    ) -> Result<AuthorizedGrant, CapabilityDenial> {
        let Some(grant) = self
            .grants
            .iter()
            .find(|grant| grant.action == request.action)
        else {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Action,
                request.action,
            ));
        };

        grant.authorize(self, &request)
    }

    fn validate(&self) -> Result<(), CapabilityError> {
        if self.schema_version != "leaven.capability.v1" {
            return Err(CapabilityError::InvalidDocument {
                message: "schema_version must be leaven.capability.v1".to_owned(),
            });
        }
        require_prefix("jti", &self.jti, "jti_")?;
        require_prefix(
            "capability_fingerprint",
            &self.capability_fingerprint,
            "fp_cap_",
        )?;
        require_prefix("policy_fingerprint", &self.policy_fingerprint, "fp_policy_")?;
        require_prefix(
            "subject_fingerprint",
            &self.subject_fingerprint,
            "fp_subject_",
        )?;
        parse_timestamp(&self.expires_at)?;
        if self.grants.is_empty() {
            return Err(CapabilityError::InvalidDocument {
                message: "capability document must contain at least one grant".to_owned(),
            });
        }
        if let Some(fingerprint) = &self.grant_fingerprint {
            require_prefix("grant_fingerprint", fingerprint, "fp_grant_")?;
        }
        if let Some(fingerprint) = &self.parent_capability_fingerprint {
            require_prefix("parent_capability_fingerprint", fingerprint, "fp_cap_")?;
        }
        match &self.token_binding {
            TokenBinding::OpaqueLookup {
                token_id,
                lookup_audience,
            } => {
                require_prefix("token_binding.token_id", token_id, "ltok_")?;
                if lookup_audience.as_deref().is_some_and(str::is_empty) {
                    return Err(CapabilityError::InvalidDocument {
                        message: "token_binding.lookup_audience cannot be empty".to_owned(),
                    });
                }
            }
            TokenBinding::SignedJwt { alg, kid } => {
                if alg.is_empty() || kid.is_empty() {
                    return Err(CapabilityError::InvalidDocument {
                        message: "signed_jwt binding requires alg and kid".to_owned(),
                    });
                }
            }
            TokenBinding::MtlsBound {
                certificate_fingerprint,
            } => {
                require_prefix(
                    "token_binding.certificate_fingerprint",
                    certificate_fingerprint,
                    "fp_",
                )?;
            }
        }
        if self.revocation.mode.is_empty() || self.revocation.check.is_empty() {
            return Err(CapabilityError::InvalidDocument {
                message: "revocation mode and check must be explicit".to_owned(),
            });
        }
        if self.renewal.mode.is_empty() {
            return Err(CapabilityError::InvalidDocument {
                message: "renewal mode must be explicit".to_owned(),
            });
        }
        if self.issuer.kind.is_empty() || self.issuer.id.is_empty() {
            return Err(CapabilityError::InvalidDocument {
                message: "issuer kind and id must be explicit".to_owned(),
            });
        }
        if self.audience.is_empty() {
            return Err(CapabilityError::InvalidDocument {
                message: "audience must not be empty".to_owned(),
            });
        }
        parse_timestamp(&self.issued_at)?;
        self.execution_policy.validate()?;
        if !self.delegation.may_delegate && self.delegation.max_depth != 0 {
            return Err(CapabilityError::InvalidDocument {
                message: "non-delegable capability must have max_depth 0".to_owned(),
            });
        }
        let _ = &self.subject;
        let _ = self.expiry_behavior;
        let _ = self.delegation.must_attenuate;
        Ok(())
    }
}

/// Capability resolution and document validation failures.
#[derive(Debug, thiserror::Error)]
pub enum CapabilityError {
    /// The document is not structured authority.
    #[error("invalid capability document: {message}")]
    InvalidDocument {
        /// Human-readable reason.
        message: String,
    },

    /// The bearer token was only a bare handle with no document.
    #[error("unknown capability token `{token_id}`")]
    UnknownToken {
        /// Opaque token id.
        token_id: String,
    },

    /// The presented token does not match the document binding.
    #[error("capability token `{token_id}` does not match bound token `{bound_token_id:?}`")]
    BindingMismatch {
        /// Presented token id.
        token_id: String,
        /// Token id from the document binding, when present.
        bound_token_id: Option<String>,
    },

    /// The token cannot authorize new work after expiry.
    #[error("capability `{jti}` expired at `{expires_at}`")]
    Expired {
        /// Capability JTI.
        jti: String,
        /// Expiry timestamp.
        expires_at: String,
    },

    /// The token was revoked.
    #[error("capability `{jti}` is revoked")]
    Revoked {
        /// Capability JTI.
        jti: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TokenBinding {
    OpaqueLookup {
        token_id: String,
        #[serde(default)]
        lookup_audience: Option<String>,
    },
    SignedJwt {
        alg: String,
        kid: String,
    },
    MtlsBound {
        certificate_fingerprint: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExpiryBehavior {
    DrainInflightNoNewOps,
    CancelInflight,
    RenewRequiredBeforeLongCall,
}

#[derive(Clone, Debug, Deserialize)]
struct RevocationPolicy {
    mode: String,
    check: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RenewalPolicy {
    mode: String,
}

#[derive(Clone, Debug, Deserialize)]
struct AggregateBudgets {
    #[serde(default, rename = "max_total_usd_micro")]
    total_usd_micro: Option<u64>,
    #[serde(default, rename = "max_lm_usd_micro")]
    lm_usd_micro: Option<u64>,
    #[serde(default, rename = "max_agent_usd_micro")]
    agent_usd_micro: Option<u64>,
    #[serde(default, rename = "max_human_usd_micro")]
    human_usd_micro: Option<u64>,
    #[serde(default, rename = "max_wall_ms")]
    wall_ms: Option<u64>,
    #[serde(default, rename = "max_concurrent_calls")]
    concurrent_calls: Option<u64>,
    #[serde(default, rename = "max_plan_nodes")]
    plan_nodes: Option<u64>,
    #[serde(default, rename = "max_materialized_bytes")]
    materialized_bytes: Option<u64>,
}

/// One capability grant with policy-bearing details preserved.
#[derive(Clone, Debug, Deserialize)]
pub struct Grant {
    /// Action path.
    pub action: String,
    /// Resource selector object.
    pub resource: BTreeMap<String, Value>,
    /// Constraint object.
    pub constraints: BTreeMap<String, Value>,
    /// Optional per-grant limits.
    #[serde(default)]
    pub limits: Option<BTreeMap<String, Value>>,
}

impl Grant {
    fn authorize(
        &self,
        document: &CapabilityDocument,
        request: &CapabilityGrantRequest,
    ) -> Result<AuthorizedGrant, CapabilityDenial> {
        ensure_resource(self, request)?;
        ensure_constraints(self, request)?;
        ensure_limits(self, request)?;

        Ok(AuthorizedGrant {
            capability_fingerprint: document.capability_fingerprint.clone(),
            policy_fingerprint: document.policy_fingerprint.clone(),
            grant_action: self.action.clone(),
            max_usd_micro: self.limit_value("max_usd_micro"),
            max_calls: self.limit_value("max_calls"),
            max_concurrent: self.limit_value("max_concurrent"),
            timeout_s: self.limit_value("timeout_s"),
            max_rows: self.limit_value("max_rows"),
            max_materialized_bytes: self.limit_value("max_materialized_bytes"),
        })
    }

    fn limit_value(&self, key: &str) -> Option<u64> {
        self.limits
            .as_ref()
            .and_then(|limits| limits.get(key))
            .and_then(Value::as_u64)
    }
}

/// Requested operation dimensions checked against a capability grant.
#[derive(Clone, Debug, Default)]
pub struct CapabilityGrantRequest {
    action: String,
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
        self.command = Some(command.into());
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
    fn new(kind: CapabilityDenialKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            redactions: Vec::new(),
        }
    }

    fn with_redactions(
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

    fn from_invalid_document(error: &CapabilityError) -> Self {
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

fn ensure_resource(
    grant: &Grant,
    request: &CapabilityGrantRequest,
) -> Result<(), CapabilityDenial> {
    for key in grant.resource.keys() {
        if !request.resource.contains_key(key) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Resource,
                format!("resource `{key}` is required by grant"),
            ));
        }
    }
    for (key, requested) in &request.resource {
        let Some(allowed) = grant.resource.get(key) else {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Resource,
                format!("resource `{key}` is not granted"),
            ));
        };
        if !value_allows(allowed, requested) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Resource,
                format!("resource `{key}` does not match grant"),
            ));
        }
    }
    Ok(())
}

fn ensure_constraints(
    grant: &Grant,
    request: &CapabilityGrantRequest,
) -> Result<(), CapabilityDenial> {
    ensure_set_constraint(
        grant,
        "case_fields",
        "forbidden_case_fields",
        &request.case_fields,
        CapabilityDenialKind::CaseField,
    )?;
    ensure_optional_one(
        grant,
        "partitions",
        request.partition.as_deref(),
        CapabilityDenialKind::Partition,
    )?;
    ensure_set_constraint(
        grant,
        "allowed_input_classes",
        "forbidden_input_classes",
        &request.input_classes,
        CapabilityDenialKind::DataClass,
    )?;
    ensure_allowed_set(
        grant,
        "purposes",
        &request.purposes,
        CapabilityDenialKind::Resource,
    )?;
    ensure_allowed_set(
        grant,
        "models",
        &request.models,
        CapabilityDenialKind::Resource,
    )?;
    ensure_allowed_set(
        grant,
        "model_roles",
        &request.model_roles,
        CapabilityDenialKind::Resource,
    )?;
    ensure_allowed_set(
        grant,
        "workspace_ops",
        &request.workspace_ops,
        CapabilityDenialKind::Resource,
    )?;
    ensure_optional_one(
        grant,
        "allowed_commands",
        request.command.as_deref(),
        CapabilityDenialKind::Resource,
    )?;
    ensure_schema_constraint(grant, &request.schemas)?;
    ensure_optional_one(
        grant,
        "allowed_surfaces",
        request.surface.as_deref(),
        CapabilityDenialKind::Surface,
    )?;
    Ok(())
}

fn ensure_set_constraint(
    grant: &Grant,
    allowed_key: &str,
    forbidden_key: &str,
    requested: &BTreeSet<String>,
    kind: CapabilityDenialKind,
) -> Result<(), CapabilityDenial> {
    let forbidden = string_set(grant.constraints.get(forbidden_key));
    let redactions = requested
        .intersection(&forbidden)
        .cloned()
        .collect::<Vec<_>>();
    if !redactions.is_empty() {
        return Err(CapabilityDenial::with_redactions(
            kind,
            format!("request intersects `{forbidden_key}`"),
            redactions,
        ));
    }
    ensure_allowed_set(grant, allowed_key, requested, kind)
}

fn ensure_allowed_set(
    grant: &Grant,
    allowed_key: &str,
    requested: &BTreeSet<String>,
    kind: CapabilityDenialKind,
) -> Result<(), CapabilityDenial> {
    let allowed_value = grant.constraints.get(allowed_key);
    let allowed = string_set(allowed_value);
    if requested.is_empty() && allowed_value.is_some() && !allowed.is_empty() {
        return Err(CapabilityDenial::new(
            kind,
            format!("request must declare `{allowed_key}`"),
        ));
    }
    if requested.is_empty() {
        return Ok(());
    }
    if requested.is_subset(&allowed) {
        Ok(())
    } else {
        Err(CapabilityDenial::new(
            kind,
            format!("request is outside `{allowed_key}`"),
        ))
    }
}

fn ensure_allowed_one(
    grant: &Grant,
    allowed_key: &str,
    requested: &str,
    kind: CapabilityDenialKind,
) -> Result<(), CapabilityDenial> {
    let allowed = string_set(grant.constraints.get(allowed_key));
    if allowed.contains(requested) {
        Ok(())
    } else {
        Err(CapabilityDenial::new(
            kind,
            format!("`{requested}` is outside `{allowed_key}`"),
        ))
    }
}

fn ensure_optional_one(
    grant: &Grant,
    allowed_key: &str,
    requested: Option<&str>,
    kind: CapabilityDenialKind,
) -> Result<(), CapabilityDenial> {
    let allowed = string_set(grant.constraints.get(allowed_key));
    match (requested, allowed.is_empty()) {
        (None, true) => Ok(()),
        (None, false) => Err(CapabilityDenial::new(
            kind,
            format!("request must declare `{allowed_key}`"),
        )),
        (Some(requested), _) => ensure_allowed_one(grant, allowed_key, requested, kind),
    }
}

fn ensure_schema_constraint(
    grant: &Grant,
    requested: &BTreeSet<String>,
) -> Result<(), CapabilityDenial> {
    let mut allowed = string_set(grant.constraints.get("schemas"));
    allowed.extend(string_set(grant.constraints.get("change_schemas")));
    if requested.is_empty() && !allowed.is_empty() {
        return Err(CapabilityDenial::new(
            CapabilityDenialKind::Schema,
            "request must declare schema fingerprints",
        ));
    }
    if requested.is_empty() {
        return Ok(());
    }
    if requested.is_subset(&allowed) {
        Ok(())
    } else {
        Err(CapabilityDenial::new(
            CapabilityDenialKind::Schema,
            "request schema is outside grant",
        ))
    }
}

fn ensure_limits(grant: &Grant, request: &CapabilityGrantRequest) -> Result<(), CapabilityDenial> {
    for key in [
        "max_usd_micro",
        "max_calls",
        "max_concurrent",
        "timeout_s",
        "max_rows",
        "max_materialized_bytes",
    ] {
        if grant.limit_value(key).is_some() && requested_limit(&request.limits, key).is_none() {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Limit,
                format!("request must declare `{key}` usage"),
            ));
        }
    }
    ensure_limit(
        grant,
        "max_usd_micro",
        request.limits.usd_micro,
        CapabilityDenialKind::Limit,
    )?;
    ensure_limit(
        grant,
        "max_calls",
        request.limits.calls,
        CapabilityDenialKind::Limit,
    )?;
    ensure_limit(
        grant,
        "max_concurrent",
        request.limits.concurrent,
        CapabilityDenialKind::Limit,
    )?;
    ensure_limit(
        grant,
        "timeout_s",
        request.limits.timeout_s,
        CapabilityDenialKind::Limit,
    )?;
    ensure_limit(
        grant,
        "max_rows",
        request.limits.rows,
        CapabilityDenialKind::Limit,
    )?;
    ensure_limit(
        grant,
        "max_materialized_bytes",
        request.limits.materialized_bytes,
        CapabilityDenialKind::Limit,
    )
}

fn requested_limit(limits: &CapabilityLimitUsage, key: &str) -> Option<u64> {
    match key {
        "max_usd_micro" => limits.usd_micro,
        "max_calls" => limits.calls,
        "max_concurrent" => limits.concurrent,
        "timeout_s" => limits.timeout_s,
        "max_rows" => limits.rows,
        "max_materialized_bytes" => limits.materialized_bytes,
        _ => None,
    }
}

fn ensure_limit(
    grant: &Grant,
    key: &str,
    requested: Option<u64>,
    kind: CapabilityDenialKind,
) -> Result<(), CapabilityDenial> {
    let Some(requested) = requested else {
        return Ok(());
    };
    let Some(max) = grant.limit_value(key) else {
        return Err(CapabilityDenial::new(
            kind,
            format!("grant has no `{key}` limit"),
        ));
    };
    if requested <= max {
        Ok(())
    } else {
        Err(CapabilityDenial::new(
            kind,
            format!("requested `{key}` exceeds grant limit"),
        ))
    }
}

fn value_allows(allowed: &Value, requested: &Value) -> bool {
    match allowed {
        Value::Array(values) => match requested {
            Value::Array(requested_values) => requested_values
                .iter()
                .all(|value| values.iter().any(|allowed| allowed == value)),
            _ => values.iter().any(|value| value == requested),
        },
        _ => allowed == requested,
    }
}

fn string_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

#[derive(Clone, Debug, Deserialize)]
struct DelegationPolicy {
    may_delegate: bool,
    max_depth: u64,
    must_attenuate: bool,
    #[serde(default)]
    allowed_actions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct Issuer {
    kind: String,
    id: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ExecutionPolicy {
    profile: String,
    network: String,
    subprocess: String,
    filesystem: String,
    byo_effects: String,
}

impl ExecutionPolicy {
    fn validate(&self) -> Result<(), CapabilityError> {
        if self.profile.is_empty()
            || self.network.is_empty()
            || self.subprocess.is_empty()
            || self.filesystem.is_empty()
            || self.byo_effects.is_empty()
        {
            Err(CapabilityError::InvalidDocument {
                message: "execution policy fields must be explicit".to_owned(),
            })
        } else {
            Ok(())
        }
    }
}

fn require_prefix(field: &str, value: &str, prefix: &str) -> Result<(), CapabilityError> {
    if value.starts_with(prefix) {
        Ok(())
    } else {
        Err(CapabilityError::InvalidDocument {
            message: format!("{field} must start with `{prefix}`"),
        })
    }
}

fn parse_timestamp(value: &str) -> Result<DateTime<FixedOffset>, CapabilityError> {
    DateTime::parse_from_rfc3339(value).map_err(|error| CapabilityError::InvalidDocument {
        message: format!("invalid timestamp `{value}`: {error}"),
    })
}

fn validate_capability_schema(value: &Value) -> Result<(), CapabilityError> {
    let schema = read_schema(CAPABILITY_SCHEMA)?;
    let validator = jsonschema::draft202012::options()
        .with_retriever(CapabilitySchemaRetriever::active()?)
        .build(&schema)
        .map_err(|error| CapabilityError::InvalidDocument {
            message: format!("capability schema failed to compile: {error}"),
        })?;
    validator
        .validate(value)
        .map_err(|error| CapabilityError::InvalidDocument {
            message: error.to_string(),
        })
}

#[derive(Clone, Debug)]
struct CapabilitySchemaRetriever {
    schemas: HashMap<String, Value>,
}

impl CapabilitySchemaRetriever {
    fn active() -> Result<Self, CapabilityError> {
        let mut schemas = HashMap::new();
        for name in [CAPABILITY_SCHEMA, "common.schema.json"] {
            let value = read_schema(name)?;
            schemas.insert(name.to_owned(), value.clone());
            if let Some(id) = value.get("$id").and_then(Value::as_str) {
                schemas.insert(id.to_owned(), value);
            }
        }
        Ok(Self { schemas })
    }
}

impl Retrieve for CapabilitySchemaRetriever {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.schemas
            .get(uri.as_str())
            .cloned()
            .ok_or_else(|| format!("schema not found: {uri}").into())
    }
}

fn read_schema(name: &str) -> Result<Value, CapabilityError> {
    let path = active_package_root().join("schemas").join(name);
    let source = fs::read_to_string(&path).map_err(|error| CapabilityError::InvalidDocument {
        message: format!("failed to read schema `{}`: {error}", path.display()),
    })?;
    serde_json::from_str(&source).map_err(|error| CapabilityError::InvalidDocument {
        message: format!("invalid schema JSON `{}`: {error}", path.display()),
    })
}

fn active_package_root() -> PathBuf {
    source_repo_root().join(ACTIVE_PACKAGE_RELATIVE)
}

fn source_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("leaven-public-seam lives under workspace/crates")
        .to_path_buf()
}
