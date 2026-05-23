use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use jsonschema::{Retrieve, Uri};
use serde::Deserialize;
use serde_json::Value;

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

    /// Validates that a child capability only narrows this parent capability.
    pub fn validate_delegation(
        &self,
        child: &Self,
    ) -> Result<CapabilityDelegation, CapabilityDenial> {
        if !self.delegation.may_delegate {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                "parent capability is not delegable",
            ));
        }
        if child.parent_capability_fingerprint.as_deref() != Some(self.capability_fingerprint()) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                "child capability does not record parent lineage",
            ));
        }
        let child_expires_at = parse_timestamp(child.expires_at())
            .map_err(|error| CapabilityDenial::from_invalid_document(&error))?;
        let parent_expires_at = parse_timestamp(self.expires_at())
            .map_err(|error| CapabilityDenial::from_invalid_document(&error))?;
        if child_expires_at > parent_expires_at {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                "child expiry widens parent expiry",
            ));
        }
        let expiry_narrowed = child_expires_at < parent_expires_at;
        let binding_narrowed =
            ensure_binding_attenuates(&self.token_binding, &child.token_binding)?;
        let budget_narrowed = ensure_budget_attenuates(&self.budgets, &child.budgets)?;
        let grants_narrowed = self.ensure_grants_attenuate(child)?;
        let narrowed = expiry_narrowed || binding_narrowed || budget_narrowed || grants_narrowed;
        ensure_delegation_policy_attenuates(self, child)?;
        if self.delegation.must_attenuate && !narrowed {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                "child does not narrow parent operational authority",
            ));
        }

        Ok(CapabilityDelegation {
            parent_capability_fingerprint: self.capability_fingerprint.clone(),
            child_capability_fingerprint: child.capability_fingerprint.clone(),
            allowed_actions: child
                .grants
                .iter()
                .map(|grant| grant.action.clone())
                .collect(),
        })
    }

    fn ensure_grants_attenuate(&self, child: &Self) -> Result<bool, CapabilityDenial> {
        let allowed_actions = self
            .delegation
            .allowed_actions
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parent_actions = self.grant_actions().collect::<BTreeSet<_>>();
        let child_actions = child.grant_actions().collect::<BTreeSet<_>>();
        if !child_actions.is_subset(&parent_actions) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                "child grants actions absent from parent",
            ));
        }
        let mut narrowed = child_actions.len() < parent_actions.len();
        for child_grant in &child.grants {
            if !allowed_actions.contains(&child_grant.action) {
                return Err(CapabilityDenial::new(
                    CapabilityDenialKind::Delegation,
                    format!("child action `{}` is not delegable", child_grant.action),
                ));
            }
            let Some(parent_grant) = self.grant(&child_grant.action) else {
                return Err(CapabilityDenial::new(
                    CapabilityDenialKind::Delegation,
                    format!(
                        "child action `{}` is not granted by parent",
                        child_grant.action
                    ),
                ));
            };
            narrowed |= ensure_resource_attenuates(parent_grant, child_grant)?;
            narrowed |= ensure_constraints_attenuate(parent_grant, child_grant)?;
            narrowed |= ensure_grant_limits_attenuate(parent_grant, child_grant)?;
        }
        Ok(narrowed)
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

/// In-memory capability registry for resolving opaque token handles.
#[derive(Clone, Debug, Default)]
pub struct CapabilityRegistry {
    by_opaque_token: BTreeMap<String, CapabilityDocument>,
    revoked_jtis: BTreeSet<String>,
}

impl CapabilityRegistry {
    /// Inserts a capability document under its own opaque token binding.
    pub fn insert(&mut self, document: CapabilityDocument) -> Result<(), CapabilityError> {
        let token = document
            .opaque_token_id()
            .ok_or_else(|| CapabilityError::InvalidDocument {
                message: "capability document is not bound to an opaque lookup token".to_owned(),
            })?;
        self.insert_with_opaque_handle(token.to_owned(), document)
    }

    /// Inserts a capability document under an explicit opaque handle.
    ///
    /// This is public so tests and transport adapters can prove binding
    /// mismatch refusal instead of assuming map keys are always correct.
    pub fn insert_with_opaque_handle(
        &mut self,
        token_id: impl Into<String>,
        document: CapabilityDocument,
    ) -> Result<(), CapabilityError> {
        self.by_opaque_token.insert(token_id.into(), document);
        Ok(())
    }

    /// Marks a JTI revoked.
    pub fn revoke_jti(&mut self, jti: impl Into<String>) {
        self.revoked_jtis.insert(jti.into());
    }

    /// Resolves an opaque token for a new operation.
    pub fn resolve_opaque_for_new_operation(
        &self,
        token_id: &str,
        now: &str,
    ) -> Result<&CapabilityDocument, CapabilityError> {
        let document =
            self.by_opaque_token
                .get(token_id)
                .ok_or_else(|| CapabilityError::UnknownToken {
                    token_id: token_id.to_owned(),
                })?;
        if document.opaque_token_id() != Some(token_id) {
            return Err(CapabilityError::BindingMismatch {
                token_id: token_id.to_owned(),
                bound_token_id: document.opaque_token_id().map(ToOwned::to_owned),
            });
        }
        if self.revoked_jtis.contains(document.jti()) {
            return Err(CapabilityError::Revoked {
                jti: document.jti().to_owned(),
            });
        }
        let now = parse_timestamp(now)?;
        let expires_at = parse_timestamp(document.expires_at())?;
        if now > expires_at {
            return Err(CapabilityError::Expired {
                jti: document.jti().to_owned(),
                expires_at: document.expires_at().to_owned(),
            });
        }
        Ok(document)
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

impl TokenBinding {
    fn kind(&self) -> &'static str {
        match self {
            Self::OpaqueLookup { .. } => "opaque_lookup",
            Self::SignedJwt { .. } => "signed_jwt",
            Self::MtlsBound { .. } => "mtls_bound",
        }
    }
}

fn ensure_binding_attenuates(
    parent: &TokenBinding,
    child: &TokenBinding,
) -> Result<bool, CapabilityDenial> {
    match (parent, child) {
        (
            TokenBinding::OpaqueLookup {
                lookup_audience: parent_audience,
                ..
            },
            TokenBinding::OpaqueLookup {
                lookup_audience: child_audience,
                ..
            },
        ) => ensure_optional_string_binding_attenuates(
            "lookup_audience",
            parent_audience.as_deref(),
            child_audience.as_deref(),
        ),
        (
            TokenBinding::SignedJwt {
                alg: parent_alg,
                kid: parent_kid,
            },
            TokenBinding::SignedJwt {
                alg: child_alg,
                kid: child_kid,
            },
        ) if parent_alg == child_alg && parent_kid == child_kid => Ok(false),
        (
            TokenBinding::MtlsBound {
                certificate_fingerprint: parent_fingerprint,
            },
            TokenBinding::MtlsBound {
                certificate_fingerprint: child_fingerprint,
            },
        ) if parent_fingerprint == child_fingerprint => Ok(false),
        _ if parent.kind() == child.kind() => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "child token binding weakens parent binding authority",
        )),
        _ => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "child token binding widens parent binding mode",
        )),
    }
}

fn ensure_optional_string_binding_attenuates(
    key: &str,
    parent: Option<&str>,
    child: Option<&str>,
) -> Result<bool, CapabilityDenial> {
    match (parent, child) {
        (Some(parent), Some(child)) if parent == child => Ok(false),
        (Some(_), _) => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child token binding omits or changes parent `{key}`"),
        )),
        (None, Some(_)) => Ok(true),
        (None, None) => Ok(false),
    }
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

fn ensure_budget_attenuates(
    parent: &AggregateBudgets,
    child: &AggregateBudgets,
) -> Result<bool, CapabilityDenial> {
    let mut narrowed = false;
    for (name, parent, child) in [
        (
            "max_total_usd_micro",
            parent.total_usd_micro,
            child.total_usd_micro,
        ),
        ("max_lm_usd_micro", parent.lm_usd_micro, child.lm_usd_micro),
        (
            "max_agent_usd_micro",
            parent.agent_usd_micro,
            child.agent_usd_micro,
        ),
        (
            "max_human_usd_micro",
            parent.human_usd_micro,
            child.human_usd_micro,
        ),
        ("max_wall_ms", parent.wall_ms, child.wall_ms),
        (
            "max_concurrent_calls",
            parent.concurrent_calls,
            child.concurrent_calls,
        ),
        ("max_plan_nodes", parent.plan_nodes, child.plan_nodes),
        (
            "max_materialized_bytes",
            parent.materialized_bytes,
            child.materialized_bytes,
        ),
    ] {
        narrowed |= ensure_optional_u64_attenuates(name, parent, child)?;
    }
    Ok(narrowed)
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
    model_roles: BTreeSet<String>,
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

    /// Adds a model role constraint.
    #[must_use]
    pub fn with_model_role(mut self, role: impl Into<String>) -> Self {
        self.model_roles.insert(role.into());
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

/// Per-operation usage checked against grant limits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CapabilityLimitUsage {
    /// Requested spend in USD micro-units.
    pub usd_micro: Option<u64>,
    /// Requested call count.
    pub calls: Option<u64>,
    /// Requested concurrent calls.
    pub concurrent: Option<u64>,
    /// Requested timeout in seconds.
    pub timeout_s: Option<u64>,
    /// Requested row count.
    pub rows: Option<u64>,
    /// Requested materialized bytes.
    pub materialized_bytes: Option<u64>,
}

/// Authorized grant facts surfaced to later permission decisions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedGrant {
    capability_fingerprint: String,
    policy_fingerprint: String,
    grant_action: String,
    max_usd_micro: Option<u64>,
    max_calls: Option<u64>,
    max_concurrent: Option<u64>,
    timeout_s: Option<u64>,
    max_rows: Option<u64>,
    max_materialized_bytes: Option<u64>,
}

/// Validated parent-child capability lineage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityDelegation {
    parent_capability_fingerprint: String,
    child_capability_fingerprint: String,
    allowed_actions: Vec<String>,
}

impl CapabilityDelegation {
    /// Parent capability fingerprint recorded by the child.
    pub fn parent_capability_fingerprint(&self) -> &str {
        &self.parent_capability_fingerprint
    }

    /// Child capability fingerprint.
    pub fn child_capability_fingerprint(&self) -> &str {
        &self.child_capability_fingerprint
    }

    /// Actions delegated to the child.
    pub fn allowed_actions(&self) -> &[String] {
        &self.allowed_actions
    }
}

impl AuthorizedGrant {
    /// Capability fingerprint attached to the authorization.
    pub fn capability_fingerprint(&self) -> &str {
        &self.capability_fingerprint
    }

    /// Policy fingerprint attached to the authorization.
    pub fn policy_fingerprint(&self) -> &str {
        &self.policy_fingerprint
    }

    /// Grant action that authorized the request.
    pub fn grant_action(&self) -> &str {
        &self.grant_action
    }

    /// Grant maximum spend in USD micro-units.
    pub fn max_usd_micro(&self) -> Option<u64> {
        self.max_usd_micro
    }

    /// Grant maximum call count.
    pub fn max_calls(&self) -> Option<u64> {
        self.max_calls
    }

    /// Grant maximum concurrent calls.
    pub fn max_concurrent(&self) -> Option<u64> {
        self.max_concurrent
    }

    /// Grant timeout in seconds.
    pub fn timeout_s(&self) -> Option<u64> {
        self.timeout_s
    }

    /// Grant maximum row count.
    pub fn max_rows(&self) -> Option<u64> {
        self.max_rows
    }

    /// Grant maximum materialized bytes.
    pub fn max_materialized_bytes(&self) -> Option<u64> {
        self.max_materialized_bytes
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

fn ensure_resource_attenuates(parent: &Grant, child: &Grant) -> Result<bool, CapabilityDenial> {
    for key in parent.resource.keys() {
        if !child.resource.contains_key(key) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                format!("child omits parent resource `{key}`"),
            ));
        }
    }
    let mut narrowed = false;
    for (key, child_value) in &child.resource {
        let Some(parent_value) = parent.resource.get(key) else {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                format!("child resource `{key}` is not present in parent"),
            ));
        };
        if !value_allows(parent_value, child_value) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                format!("child resource `{key}` widens parent"),
            ));
        }
        narrowed |= value_narrows(parent_value, child_value);
    }
    Ok(narrowed)
}

fn ensure_constraints_attenuate(parent: &Grant, child: &Grant) -> Result<bool, CapabilityDenial> {
    let keys = parent
        .constraints
        .keys()
        .chain(child.constraints.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut narrowed = false;
    for key in keys {
        let parent_value = parent.constraints.get(&key);
        let child_value = child.constraints.get(&key);
        narrowed |= if key.starts_with("forbidden_") {
            ensure_forbidden_constraint_attenuates(&key, parent_value, child_value)?
        } else {
            ensure_allowed_constraint_attenuates(&key, parent_value, child_value)?
        };
    }
    Ok(narrowed)
}

fn ensure_allowed_constraint_attenuates(
    key: &str,
    parent: Option<&Value>,
    child: Option<&Value>,
) -> Result<bool, CapabilityDenial> {
    match (parent, child) {
        (None, None) => Ok(false),
        (Some(_), None) => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child omits parent constraint `{key}`"),
        )),
        (None, Some(_)) => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child constraint `{key}` is absent from parent"),
        )),
        (Some(parent), Some(child)) if value_allows(parent, child) => {
            Ok(value_narrows(parent, child))
        }
        (Some(_), Some(_)) => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child constraint `{key}` widens parent"),
        )),
    }
}

fn ensure_forbidden_constraint_attenuates(
    key: &str,
    parent: Option<&Value>,
    child: Option<&Value>,
) -> Result<bool, CapabilityDenial> {
    let parent = string_set(parent);
    let child = string_set(child);
    if parent.is_subset(&child) {
        Ok(child.len() > parent.len())
    } else {
        Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child forbidden constraint `{key}` weakens parent"),
        ))
    }
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
        "model_roles",
        &request.model_roles,
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

fn ensure_grant_limits_attenuate(parent: &Grant, child: &Grant) -> Result<bool, CapabilityDenial> {
    let mut narrowed = false;
    for key in [
        "max_usd_micro",
        "max_calls",
        "max_concurrent",
        "timeout_s",
        "max_rows",
        "max_materialized_bytes",
    ] {
        narrowed |=
            ensure_optional_u64_attenuates(key, parent.limit_value(key), child.limit_value(key))?;
    }
    Ok(narrowed)
}

fn ensure_delegation_policy_attenuates(
    parent: &CapabilityDocument,
    child: &CapabilityDocument,
) -> Result<(), CapabilityDenial> {
    if parent.delegation.max_depth == 0 {
        return Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "parent delegation depth is exhausted",
        ));
    }
    if parent.delegation.must_attenuate && !child.delegation.must_attenuate {
        return Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "child disables parent attenuation requirement",
        ));
    }
    if !parent.delegation.may_delegate && child.delegation.may_delegate {
        return Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "child enables delegation denied by parent",
        ));
    }
    if child.delegation.may_delegate && child.delegation.max_depth == 0 {
        return Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "delegable child must carry remaining delegation depth",
        ));
    }
    if child.delegation.max_depth >= parent.delegation.max_depth {
        return Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "child delegation depth does not attenuate parent",
        ));
    }
    if !child.delegation.may_delegate && !child.delegation.allowed_actions.is_empty() {
        return Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "non-delegable child cannot carry delegable actions",
        ));
    }

    let child_grant_actions = child.grant_actions().collect::<BTreeSet<_>>();
    let parent_delegable_actions = parent
        .delegation
        .allowed_actions
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for action in &child.delegation.allowed_actions {
        if !parent_delegable_actions.contains(action) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                format!("child delegation action `{action}` is not allowed by parent"),
            ));
        }
        if !child_grant_actions.contains(action.as_str()) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                format!("child delegation action `{action}` is not granted to child"),
            ));
        }
    }

    Ok(())
}

fn ensure_optional_u64_attenuates(
    key: &str,
    parent: Option<u64>,
    child: Option<u64>,
) -> Result<bool, CapabilityDenial> {
    match (parent, child) {
        (Some(_), None) => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child omits parent limit `{key}`"),
        )),
        (Some(parent), Some(child)) if child > parent => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child limit `{key}` widens parent"),
        )),
        (Some(parent), Some(child)) => Ok(child < parent),
        (None, Some(_)) => Ok(true),
        (None, None) => Ok(false),
    }
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

fn value_narrows(parent: &Value, child: &Value) -> bool {
    value_allows(parent, child) && !value_allows(child, parent)
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
