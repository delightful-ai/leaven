use std::collections::BTreeSet;

use serde::Deserialize;

use super::CapabilityResourceValue;

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

    pub(super) fn allows_one(&self, key: &str, value: &str) -> bool {
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
