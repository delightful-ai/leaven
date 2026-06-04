use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::Value;

mod budget;
mod delegation;
mod grant;
mod grant_checks;
mod registry;
mod request;

pub use budget::{
    CapabilityBudgetLedger, CapabilityBudgetProjectionError, CapabilityBudgetReservation,
    CapabilityBudgetUsage,
};
pub use delegation::CapabilityDelegation;
pub use grant::{AuthorizedGrant, CapabilityLimitUsage, CapabilityResourceValue};
use grant_checks::{
    DelegationPolicy, ExecutionPolicy, Issuer, ensure_constraints, ensure_limits, ensure_resource,
    grant_receives_target, invalid_document, parse_timestamp, require_prefix,
    validate_capability_schema,
};
pub use registry::CapabilityRegistry;
pub use request::{CapabilityDenial, CapabilityDenialKind, CapabilityGrantRequest};

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
    subject: CapabilitySubject,
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

    /// Aggregate evaluator budget in USD micro-units.
    pub fn max_evaluator_usd_micro(&self) -> Option<u64> {
        self.budgets.evaluator_usd_micro
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
        match &self.subject {
            CapabilitySubject::StageCall { role, .. } => Some(role.as_str()),
            _ => None,
        }
    }

    /// Execution policy profile.
    pub fn execution_policy_profile(&self) -> &str {
        &self.execution_policy.profile
    }

    pub(crate) fn execution_policy_network(&self) -> &str {
        &self.execution_policy.network
    }

    pub(crate) fn execution_policy_subprocess(&self) -> &str {
        &self.execution_policy.subprocess
    }

    pub(crate) fn execution_policy_filesystem(&self) -> &str {
        &self.execution_policy.filesystem
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
        self.validate_subject_grant_invariants()?;
        let _ = self.expiry_behavior;
        let _ = self.delegation.must_attenuate;
        Ok(())
    }

    fn validate_subject_grant_invariants(&self) -> Result<(), CapabilityError> {
        match &self.subject {
            CapabilitySubject::StageCall { role, .. } => match role.as_str() {
                "runner" => self.ensure_stage_role_cannot_receive_target("runner"),
                "reflector" => self.ensure_stage_role_cannot_receive_target("reflector"),
                _ => Ok(()),
            },
            CapabilitySubject::EvaluationStageCall {
                evaluation_request_id,
                ..
            } => self.ensure_assessment_submit_matches_evaluation_request(evaluation_request_id),
            CapabilitySubject::Operator { .. } => Ok(()),
        }
    }

    fn ensure_stage_role_cannot_receive_target(&self, role: &str) -> Result<(), CapabilityError> {
        for grant in &self.grants {
            if grant_receives_target(grant) {
                return Err(invalid_document(format!(
                    "{role} capability must not grant case.target fields or egress"
                )));
            }
        }
        Ok(())
    }

    fn ensure_assessment_submit_matches_evaluation_request(
        &self,
        evaluation_request_id: &str,
    ) -> Result<(), CapabilityError> {
        for grant in &self.grants {
            if grant.action == "assessment.submit"
                && !grant
                    .resource
                    .allows_one("evaluation_request_id", evaluation_request_id)
            {
                return Err(invalid_document(
                    "assessment.submit grant must match evaluation_stage_call evaluation_request_id",
                ));
            }
        }
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

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum CapabilitySubject {
    StageCall {
        run: String,
        stage_call_id: String,
        role: String,
    },
    EvaluationStageCall {
        run: String,
        stage_call_id: String,
        evaluation_request_id: String,
        #[serde(default)]
        evaluator: Option<String>,
    },
    Operator {
        principal: String,
    },
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
    #[serde(default, rename = "max_evaluator_usd_micro")]
    evaluator_usd_micro: Option<u64>,
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
    pub resource: CapabilityGrantResource,
    /// Constraint object.
    pub constraints: CapabilityGrantConstraints,
    /// Optional per-grant limits.
    #[serde(default)]
    pub limits: CapabilityGrantLimits,
}

impl Grant {
    /// Returns true when this grant's resource selector accepts a concrete id/name.
    pub fn allows_resource(&self, key: &str, value: &str) -> bool {
        self.resource.allows_one(key, value)
    }

    /// Numeric limit value for a schema-owned grant limit key.
    pub fn limit(&self, key: &str) -> Option<u64> {
        self.limit_value(key)
    }

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
        self.limits.value(key)
    }
}

/// Closed resource selector for a capability grant.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct CapabilityGrantResource {
    run: Option<CapabilityResourceValue>,
    runs: Option<CapabilityResourceValue>,
    run_set: Option<CapabilityResourceValue>,
    evaluation_request_id: Option<CapabilityResourceValue>,
    resolved_set: Option<CapabilityResourceValue>,
    candidate_ids: Option<CapabilityResourceValue>,
    case_ids: Option<CapabilityResourceValue>,
    workspace_ids: Option<CapabilityResourceValue>,
    lm_pool: Option<CapabilityResourceValue>,
    runtime_pool: Option<CapabilityResourceValue>,
    sandbox_pool: Option<CapabilityResourceValue>,
    namespace: Option<CapabilityResourceValue>,
}

impl CapabilityGrantResource {
    pub(super) fn keys(&self) -> Vec<&'static str> {
        let mut keys = Vec::new();
        for (key, value) in [
            ("run", &self.run),
            ("runs", &self.runs),
            ("run_set", &self.run_set),
            ("evaluation_request_id", &self.evaluation_request_id),
            ("resolved_set", &self.resolved_set),
            ("candidate_ids", &self.candidate_ids),
            ("case_ids", &self.case_ids),
            ("workspace_ids", &self.workspace_ids),
            ("lm_pool", &self.lm_pool),
            ("runtime_pool", &self.runtime_pool),
            ("sandbox_pool", &self.sandbox_pool),
            ("namespace", &self.namespace),
        ] {
            if value.is_some() {
                keys.push(key);
            }
        }
        keys
    }

    pub(super) fn get(&self, key: &str) -> Option<&CapabilityResourceValue> {
        match key {
            "run" => self.run.as_ref(),
            "runs" => self.runs.as_ref(),
            "run_set" => self.run_set.as_ref(),
            "evaluation_request_id" => self.evaluation_request_id.as_ref(),
            "resolved_set" => self.resolved_set.as_ref(),
            "candidate_ids" => self.candidate_ids.as_ref(),
            "case_ids" => self.case_ids.as_ref(),
            "workspace_ids" => self.workspace_ids.as_ref(),
            "lm_pool" => self.lm_pool.as_ref(),
            "runtime_pool" => self.runtime_pool.as_ref(),
            "sandbox_pool" => self.sandbox_pool.as_ref(),
            "namespace" => self.namespace.as_ref(),
            _ => None,
        }
    }

    pub(super) fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub(super) fn entries(&self) -> Vec<(&'static str, &CapabilityResourceValue)> {
        self.keys()
            .into_iter()
            .filter_map(|key| self.get(key).map(|value| (key, value)))
            .collect()
    }

    fn allows_one(&self, key: &str, value: &str) -> bool {
        self.get(key)
            .is_some_and(|allowed| allowed.allows(&CapabilityResourceValue::one(value)))
    }
}

/// Closed constraint object for a capability grant.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct CapabilityGrantConstraints {
    queries: BTreeSet<String>,
    fields: BTreeSet<String>,
    case_fields: BTreeSet<String>,
    forbidden_case_fields: BTreeSet<String>,
    partitions: BTreeSet<String>,
    visibility_classes: BTreeSet<String>,
    target_egress: Option<String>,
    allowed_input_classes: BTreeSet<String>,
    forbidden_input_classes: BTreeSet<String>,
    evidence_visibility: Option<String>,
    count_policy: Option<String>,
    revision_window: Option<RevisionWindowConstraint>,
    model_roles: BTreeSet<String>,
    models: BTreeSet<String>,
    purposes: BTreeSet<String>,
    raw_prompt_logging: Option<String>,
    raw_completion_logging: Option<String>,
    raw_transcript_visibility: Option<String>,
    workspace_ops: BTreeSet<String>,
    deny_paths: BTreeSet<String>,
    allow_paths: BTreeSet<String>,
    allowed_commands: BTreeSet<String>,
    effects: BTreeSet<String>,
    allowed_surfaces: BTreeSet<String>,
    change_schemas: BTreeSet<String>,
    may_apply: Option<bool>,
    assessment_shapes: BTreeSet<String>,
    granularity: Option<String>,
    allowed_candidates: BTreeSet<String>,
    allowed_cases: BTreeSet<String>,
    namespaces: BTreeSet<String>,
    ops: BTreeSet<String>,
    schemas: BTreeSet<String>,
}

impl CapabilityGrantConstraints {
    pub(super) fn string_set(&self, key: &str) -> BTreeSet<String> {
        match key {
            "queries" => self.queries.clone(),
            "fields" => self.fields.clone(),
            "case_fields" => self.case_fields.clone(),
            "forbidden_case_fields" => self.forbidden_case_fields.clone(),
            "partitions" => self.partitions.clone(),
            "visibility_classes" => self.visibility_classes.clone(),
            "allowed_input_classes" => self.allowed_input_classes.clone(),
            "forbidden_input_classes" => self.forbidden_input_classes.clone(),
            "model_roles" => self.model_roles.clone(),
            "models" => self.models.clone(),
            "purposes" => self.purposes.clone(),
            "workspace_ops" => self.workspace_ops.clone(),
            "deny_paths" => self.deny_paths.clone(),
            "allow_paths" => self.allow_paths.clone(),
            "allowed_commands" => self.allowed_commands.clone(),
            "effects" => self.effects.clone(),
            "allowed_surfaces" => self.allowed_surfaces.clone(),
            "change_schemas" => self.change_schemas.clone(),
            "assessment_shapes" => self.assessment_shapes.clone(),
            "allowed_candidates" => self.allowed_candidates.clone(),
            "allowed_cases" => self.allowed_cases.clone(),
            "namespaces" => self.namespaces.clone(),
            "ops" => self.ops.clone(),
            "schemas" => self.schemas.clone(),
            _ => BTreeSet::new(),
        }
    }

    pub(super) fn optional_string(&self, key: &str) -> Option<&str> {
        match key {
            "target_egress" => self.target_egress.as_deref(),
            "evidence_visibility" => self.evidence_visibility.as_deref(),
            "count_policy" => self.count_policy.as_deref(),
            "raw_prompt_logging" => self.raw_prompt_logging.as_deref(),
            "raw_completion_logging" => self.raw_completion_logging.as_deref(),
            "raw_transcript_visibility" => self.raw_transcript_visibility.as_deref(),
            "granularity" => self.granularity.as_deref(),
            _ => None,
        }
    }

    pub(super) fn optional_bool(&self, key: &str) -> Option<bool> {
        match key {
            "may_apply" => self.may_apply,
            _ => None,
        }
    }

    pub(super) fn keys(&self) -> Vec<&'static str> {
        let mut keys = Vec::new();
        for key in [
            "queries",
            "fields",
            "case_fields",
            "forbidden_case_fields",
            "partitions",
            "visibility_classes",
            "allowed_input_classes",
            "forbidden_input_classes",
            "model_roles",
            "models",
            "purposes",
            "workspace_ops",
            "deny_paths",
            "allow_paths",
            "allowed_commands",
            "effects",
            "allowed_surfaces",
            "change_schemas",
            "assessment_shapes",
            "allowed_candidates",
            "allowed_cases",
            "namespaces",
            "ops",
            "schemas",
        ] {
            if !self.string_set(key).is_empty() {
                keys.push(key);
            }
        }
        for key in [
            "target_egress",
            "evidence_visibility",
            "count_policy",
            "raw_prompt_logging",
            "raw_completion_logging",
            "raw_transcript_visibility",
            "granularity",
        ] {
            if self.optional_string(key).is_some() {
                keys.push(key);
            }
        }
        if self.may_apply.is_some() {
            keys.push("may_apply");
        }
        keys
    }

    pub(super) fn get(&self, key: &str) -> Option<CapabilityConstraintValue> {
        let values = self.string_set(key);
        if !values.is_empty() {
            return Some(CapabilityConstraintValue::Set(values));
        }
        if let Some(value) = self.optional_string(key) {
            return Some(CapabilityConstraintValue::String(value.to_owned()));
        }
        self.optional_bool(key).map(CapabilityConstraintValue::Bool)
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
struct RevisionWindowConstraint {
    min: Option<String>,
    max: Option<String>,
}

/// Typed value for one capability grant constraint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CapabilityConstraintValue {
    Set(BTreeSet<String>),
    String(String),
    Bool(bool),
}

impl CapabilityConstraintValue {
    pub(super) fn allows(&self, child: &Self) -> bool {
        match (self, child) {
            (Self::Set(parent), Self::Set(child)) => child.is_subset(parent),
            (Self::String(parent), Self::String(child)) => parent == child,
            (Self::Bool(parent), Self::Bool(child)) => parent == child,
            _ => false,
        }
    }

    pub(super) fn narrows(&self, child: &Self) -> bool {
        self.allows(child) && !child.allows(self)
    }

    pub(super) fn forbidden_attenuates(&self, child: &Self) -> bool {
        match (self, child) {
            (Self::Set(parent), Self::Set(child)) => parent.is_subset(child),
            _ => false,
        }
    }

    pub(super) fn forbidden_narrows(&self, child: &Self) -> bool {
        match (self, child) {
            (Self::Set(parent), Self::Set(child)) => child.len() > parent.len(),
            _ => false,
        }
    }
}

/// Closed per-grant limit object.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct CapabilityGrantLimits {
    max_usd_micro: Option<u64>,
    max_calls: Option<u64>,
    max_concurrent: Option<u64>,
    timeout_s: Option<u64>,
    max_rows: Option<u64>,
    max_materialized_bytes: Option<u64>,
}

impl CapabilityGrantLimits {
    pub(super) fn value(&self, key: &str) -> Option<u64> {
        match key {
            "max_usd_micro" => self.max_usd_micro,
            "max_calls" => self.max_calls,
            "max_concurrent" => self.max_concurrent,
            "timeout_s" => self.timeout_s,
            "max_rows" => self.max_rows,
            "max_materialized_bytes" => self.max_materialized_bytes,
            _ => None,
        }
    }
}
