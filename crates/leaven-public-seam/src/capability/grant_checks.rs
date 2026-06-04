use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use jsonschema::{Retrieve, Uri};
use serde::Deserialize;
use serde_json::Value;

use super::{
    ACTIVE_PACKAGE_RELATIVE, CAPABILITY_SCHEMA, CapabilityDenial, CapabilityDenialKind,
    CapabilityError, CapabilityGrantRequest, CapabilityLimitUsage, Grant,
};

pub(super) fn ensure_resource(
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
        if !allowed.allows(requested) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Resource,
                format!("resource `{key}` does not match grant"),
            ));
        }
    }
    Ok(())
}

pub(super) fn ensure_constraints(
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
    ensure_allowed_set(
        grant,
        "allowed_commands",
        &request.commands,
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
    let forbidden = grant.constraints.string_set(forbidden_key);
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
    let allowed = grant.constraints.string_set(allowed_key);
    if requested.is_empty() && !allowed.is_empty() {
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
    let allowed = grant.constraints.string_set(allowed_key);
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
    let allowed = grant.constraints.string_set(allowed_key);
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
    let mut allowed = grant.constraints.string_set("schemas");
    allowed.extend(grant.constraints.string_set("change_schemas"));
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

pub(super) fn ensure_limits(
    grant: &Grant,
    request: &CapabilityGrantRequest,
) -> Result<(), CapabilityDenial> {
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

pub(super) fn grant_receives_target(grant: &Grant) -> bool {
    grant
        .constraints
        .string_set("case_fields")
        .contains("target")
        || grant
            .constraints
            .string_set("allowed_input_classes")
            .contains("case.target")
        || grant
            .constraints
            .optional_string("target_egress")
            .is_some_and(|egress| !matches!(egress, "none" | "denied"))
}

pub(super) fn invalid_document(message: impl Into<String>) -> CapabilityError {
    CapabilityError::InvalidDocument {
        message: message.into(),
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct DelegationPolicy {
    pub(super) may_delegate: bool,
    pub(super) max_depth: u64,
    pub(super) must_attenuate: bool,
    #[serde(default)]
    pub(super) allowed_actions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Issuer {
    pub(super) kind: String,
    pub(super) id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct ExecutionPolicy {
    pub(super) profile: String,
    pub(super) network: String,
    pub(super) subprocess: String,
    pub(super) filesystem: String,
    byo_effects: String,
}

impl ExecutionPolicy {
    pub(super) fn validate(&self) -> Result<(), CapabilityError> {
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

pub(super) fn require_prefix(
    field: &str,
    value: &str,
    prefix: &str,
) -> Result<(), CapabilityError> {
    if value.starts_with(prefix) {
        Ok(())
    } else {
        Err(CapabilityError::InvalidDocument {
            message: format!("{field} must start with `{prefix}`"),
        })
    }
}

pub(super) fn parse_timestamp(value: &str) -> Result<DateTime<FixedOffset>, CapabilityError> {
    DateTime::parse_from_rfc3339(value).map_err(|error| CapabilityError::InvalidDocument {
        message: format!("invalid timestamp `{value}`: {error}"),
    })
}

pub(super) fn validate_capability_schema(value: &Value) -> Result<(), CapabilityError> {
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
