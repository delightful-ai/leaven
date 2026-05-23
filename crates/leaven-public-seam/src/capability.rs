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
