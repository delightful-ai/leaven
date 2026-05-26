use std::collections::BTreeMap;

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
pub use grant::{AuthorizedGrant, CapabilityLimitUsage};
use grant_checks::{
    DelegationPolicy, ExecutionPolicy, Issuer, ensure_constraints, ensure_limits, ensure_resource,
    grant_receives_target, invalid_document, parse_timestamp, require_prefix, string_set,
    validate_capability_schema, value_allows,
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
        let subject = self.subject.as_object().ok_or_else(|| {
            invalid_document("capability subject must be a locked subject object")
        })?;
        match subject.get("kind").and_then(Value::as_str) {
            Some("stage_call") => match subject.get("role").and_then(Value::as_str) {
                Some("runner") => self.ensure_stage_role_cannot_receive_target("runner"),
                Some("reflector") => self.ensure_stage_role_cannot_receive_target("reflector"),
                _ => Ok(()),
            },
            Some("evaluation_stage_call") => {
                let evaluation_request_id = subject
                    .get("evaluation_request_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        invalid_document(
                            "evaluation_stage_call subject must carry evaluation_request_id",
                        )
                    })?;
                self.ensure_assessment_submit_matches_evaluation_request(evaluation_request_id)
            }
            Some("operator") => Ok(()),
            Some(other) => Err(invalid_document(format!(
                "capability subject kind `{other}` is not in the locked V1 subject set"
            ))),
            None => Err(invalid_document("capability subject must carry kind")),
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
                && grant
                    .resource
                    .get("evaluation_request_id")
                    .and_then(Value::as_str)
                    != Some(evaluation_request_id)
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
