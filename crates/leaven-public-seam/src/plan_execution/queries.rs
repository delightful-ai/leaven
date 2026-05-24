use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use crate::{CapabilityDocument, CapabilityGrantRequest, PublicSeamError};

use super::{
    PlanExecutionContext,
    effects::{LiveWorkspaceHandle, require_live_workspace, workspace_ref_id},
    invalid_plan, nested_kind, object,
};

/// Lowered graph-read consistency scope for a Plan IR `graph_query`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanGraphReadScope<'a> {
    /// Read from the graph revision captured when plan execution started.
    LatestAtStart { revision: &'a str },
    /// Read from an explicitly pinned graph revision.
    AtRevision { revision: &'a str },
    /// Read a finite graph-event diff over the declared revision interval.
    SinceRevision {
        since: &'a str,
        until: Option<&'a str>,
    },
}

/// Lowered `graph_query` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanGraphQueryRequest<'a> {
    pub(super) name: &'a str,
    pub(super) expr: &'a Value,
    pub(super) scope: PlanGraphReadScope<'a>,
}

impl<'a> PlanGraphQueryRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `graph_query` expression body from the Plan IR.
    pub const fn expr(&self) -> &'a Value {
        self.expr
    }

    /// Consistency-derived graph read scope.
    pub const fn scope(&self) -> PlanGraphReadScope<'a> {
        self.scope
    }
}

/// Host outcome for a typed `graph_query` read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanGraphQueryOutcome {
    pub(super) items: Vec<Value>,
    pub(super) graph_revision: String,
    pub(super) data_classes: Vec<String>,
    pub(super) next_cursor: Option<String>,
}

impl PlanGraphQueryOutcome {
    /// Creates a graph-set outcome for a pure graph read.
    pub fn new(items: impl IntoIterator<Item = Value>, graph_revision: impl Into<String>) -> Self {
        Self {
            items: items.into_iter().collect(),
            graph_revision: graph_revision.into(),
            data_classes: vec!["public".to_owned()],
            next_cursor: None,
        }
    }

    /// Overrides the data classes carried by the graph-set value.
    #[must_use]
    pub fn with_data_classes(mut self, data_classes: impl IntoIterator<Item = String>) -> Self {
        self.data_classes = data_classes.into_iter().collect();
        self
    }

    /// Adds the next cursor returned by the graph read.
    #[must_use]
    pub fn with_next_cursor(mut self, next_cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(next_cursor.into());
        self
    }
}

/// Lowered `case_query.load` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanCaseQueryRequest<'a> {
    pub(super) name: &'a str,
    pub(super) query: &'a Value,
}

impl<'a> PlanCaseQueryRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `case_query.load` body from the Plan IR.
    pub const fn query(&self) -> &'a Value {
        self.query
    }
}

/// Host outcome for a typed `case_query.load` read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanCaseQueryOutcome {
    pub(super) case: String,
    pub(super) graph_revision: String,
    pub(super) data_classes: Vec<String>,
    pub(super) input: Option<Value>,
    pub(super) target: Option<Value>,
    pub(super) metadata: Option<Value>,
}

impl PlanCaseQueryOutcome {
    /// Creates a loaded case outcome.
    pub fn new(case: impl Into<String>, graph_revision: impl Into<String>) -> Self {
        Self {
            case: case.into(),
            graph_revision: graph_revision.into(),
            data_classes: vec!["public".to_owned()],
            input: None,
            target: None,
            metadata: None,
        }
    }

    /// Overrides the data classes carried by the case record.
    #[must_use]
    pub fn with_data_classes(mut self, data_classes: impl IntoIterator<Item = String>) -> Self {
        self.data_classes = data_classes.into_iter().collect();
        self
    }

    /// Adds case input to the loaded record.
    #[must_use]
    pub fn with_input(mut self, input: Value) -> Self {
        self.input = Some(input);
        self
    }

    /// Adds case target to the loaded record.
    #[must_use]
    pub fn with_target(mut self, target: Value) -> Self {
        self.target = Some(target);
        self
    }

    /// Adds case metadata to the loaded record.
    #[must_use]
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Lowered `workspace_query` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanWorkspaceQueryRequest<'a> {
    pub(super) name: &'a str,
    pub(super) expr: &'a Value,
    pub(super) workspace: &'a str,
    pub(super) op: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
}

impl<'a> PlanWorkspaceQueryRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `workspace_query` expression body from the Plan IR.
    pub const fn expr(&self) -> &'a Value {
        self.expr
    }

    /// Workspace handle requested for the read.
    pub const fn workspace(&self) -> &'a str {
        self.workspace
    }

    /// Workspace query operation.
    pub const fn op(&self) -> &'a Value {
        self.op
    }

    /// Resolved dependency bindings visible to this query.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Workspace query operation kind.
    pub fn op_kind(&self) -> Result<&'a str, PublicSeamError> {
        self.op
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_plan("workspace_query op must carry kind"))
    }

    /// Workspace path requested by path-shaped operations.
    pub fn path(&self) -> Result<Option<&'a str>, PublicSeamError> {
        match self.op.get("path") {
            Some(Value::String(path)) => Ok(Some(path)),
            Some(_) => Err(invalid_plan("workspace_query path must be a string")),
            None => Ok(None),
        }
    }

    /// Expected data classes declared by `read_file`.
    pub fn expected_data_classes(&self) -> Result<BTreeSet<&'a str>, PublicSeamError> {
        let Some(values) = self.op.get("expected_data_classes") else {
            return Ok(BTreeSet::new());
        };
        values
            .as_array()
            .ok_or_else(|| invalid_plan("workspace_query expected_data_classes must be an array"))?
            .iter()
            .map(|value| {
                value.as_str().ok_or_else(|| {
                    invalid_plan("workspace_query expected data classes must be strings")
                })
            })
            .collect()
    }
}

/// Host outcome for a typed `workspace_query` read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanWorkspaceQueryOutcome {
    pub(super) value: Value,
    pub(super) graph_revision: String,
    pub(super) data_classes: Vec<String>,
    pub(super) replayability: String,
}

impl PlanWorkspaceQueryOutcome {
    /// Creates a workspace read outcome whose kind-specific fields live in `value`.
    pub fn new(value: Value, graph_revision: impl Into<String>) -> Self {
        Self {
            value,
            graph_revision: graph_revision.into(),
            data_classes: vec!["public".to_owned()],
            replayability: "boundary_managed".to_owned(),
        }
    }

    /// Overrides the data classes carried by the workspace value.
    #[must_use]
    pub fn with_data_classes(mut self, data_classes: impl IntoIterator<Item = String>) -> Self {
        self.data_classes = data_classes.into_iter().collect();
        self
    }

    /// Overrides the replayability classification carried by the workspace value.
    #[must_use]
    pub fn with_replayability(mut self, replayability: impl Into<String>) -> Self {
        self.replayability = replayability.into();
        self
    }
}

pub(super) fn case_query_include(query: &Value) -> Result<BTreeSet<&str>, PublicSeamError> {
    let include = query
        .get("include")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("case_query.load must carry include"))?;
    let mut fields = BTreeSet::new();
    for field in include {
        let field = field
            .as_str()
            .ok_or_else(|| invalid_plan("case_query.load include entries must be strings"))?;
        if !matches!(field, "input" | "target" | "metadata") {
            return Err(invalid_plan(format!(
                "case_query.load include field `{field}` is not supported"
            )));
        }
        fields.insert(field);
    }
    Ok(fields)
}

pub(super) fn workspace_query_request<'a>(
    name: &'a str,
    expr: &'a Value,
    deps: &'a BTreeMap<String, Value>,
    live_workspaces: &'a BTreeMap<String, LiveWorkspaceHandle>,
) -> Result<PlanWorkspaceQueryRequest<'a>, PublicSeamError> {
    let request = workspace_query_request_from_values(name, expr, deps)?;
    require_live_workspace(request.workspace, deps, live_workspaces, "workspace_query")?;
    Ok(request)
}

pub(super) fn workspace_query_request_from_values<'a>(
    name: &'a str,
    expr: &'a Value,
    deps: &'a BTreeMap<String, Value>,
) -> Result<PlanWorkspaceQueryRequest<'a>, PublicSeamError> {
    let object = object(expr, "workspace_query")?;
    let workspace = workspace_ref_id(
        object.get("workspace"),
        "workspace_query must carry workspace",
    )?;
    let op = object
        .get("op")
        .ok_or_else(|| invalid_plan("workspace_query must carry op"))?;
    let request = PlanWorkspaceQueryRequest {
        name,
        expr,
        workspace,
        op,
        deps,
    };
    validate_workspace_query_request_op(request)?;
    Ok(request)
}

pub(super) fn workspace_query_expected_value_kind(
    request: PlanWorkspaceQueryRequest<'_>,
) -> Result<&'static str, PublicSeamError> {
    match request.op_kind()? {
        "snapshot" | "digest" => Ok("workspace_snapshot"),
        "list" | "stat" | "capture_artifacts" => Ok("workspace_listing"),
        "read_file" => Ok("workspace_file"),
        "git_log" | "git_diff" | "git_status" => Ok("workspace_diff"),
        other => Err(invalid_plan(format!(
            "unknown workspace_query op `{other}`"
        ))),
    }
}

pub(super) fn validate_workspace_query_value_shape(
    request: PlanWorkspaceQueryRequest<'_>,
    value: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    match request.op_kind()? {
        "read_file" => validate_workspace_read_file_value(request, value),
        "list" => validate_workspace_list_value(request, value),
        "stat" => validate_workspace_stat_value(request, value),
        "digest" => validate_workspace_digest_value(request, value),
        "snapshot" => validate_workspace_snapshot_value(request, value),
        "git_log" | "git_diff" | "git_status" => validate_workspace_diff_value(request, value),
        "capture_artifacts" => validate_workspace_capture_artifacts_value(request, value),
        other => Err(invalid_plan(format!(
            "unknown workspace_query op `{other}`"
        ))),
    }
}

fn validate_workspace_read_file_value(
    request: PlanWorkspaceQueryRequest<'_>,
    value: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let requested_path = request
        .path()?
        .ok_or_else(|| invalid_plan("workspace_query read_file must carry path"))?;
    validate_workspace_path("workspace_query read_file path", requested_path)?;
    let path = value
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan("workspace_query read_file result must carry path"))?;
    validate_workspace_path("workspace_query read_file result path", path)?;
    if path != requested_path {
        return Err(invalid_plan(format!(
            "workspace_query read_file result path `{path}` does not match requested `{requested_path}`"
        )));
    }
    if value.get("content").is_none() && value.get("blob_ref").is_none() {
        return Err(invalid_plan(
            "workspace_query read_file result must carry content or blob_ref",
        ));
    }
    Ok(())
}

fn validate_workspace_list_value(
    request: PlanWorkspaceQueryRequest<'_>,
    value: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let requested_path = request
        .path()?
        .ok_or_else(|| invalid_plan("workspace_query list must carry path"))?;
    validate_workspace_path("workspace_query list path", requested_path)?;
    let entries = workspace_listing_entries(value, "list")?;
    for entry in entries {
        let entry_path = workspace_listing_entry_path(entry, "list")?;
        validate_workspace_path("workspace_query list entry path", entry_path)?;
        if !path_is_at_or_below(entry_path, requested_path) {
            return Err(invalid_plan(format!(
                "workspace_query list result path `{entry_path}` is outside requested `{requested_path}`"
            )));
        }
    }
    Ok(())
}

fn validate_workspace_stat_value(
    request: PlanWorkspaceQueryRequest<'_>,
    value: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let requested_path = request
        .path()?
        .ok_or_else(|| invalid_plan("workspace_query stat must carry path"))?;
    validate_workspace_path("workspace_query stat path", requested_path)?;
    let entries = workspace_listing_entries(value, "stat")?;
    if entries.len() != 1 {
        return Err(invalid_plan(
            "workspace_query stat result must carry exactly one listing entry",
        ));
    }
    let entry_path = workspace_listing_entry_path(&entries[0], "stat")?;
    validate_workspace_path("workspace_query stat entry path", entry_path)?;
    if entry_path != requested_path {
        return Err(invalid_plan(format!(
            "workspace_query stat result path `{entry_path}` does not match requested `{requested_path}`"
        )));
    }
    Ok(())
}

fn validate_workspace_digest_value(
    request: PlanWorkspaceQueryRequest<'_>,
    value: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let requested_path = request
        .path()?
        .ok_or_else(|| invalid_plan("workspace_query digest must carry path"))?;
    validate_workspace_path("workspace_query digest path", requested_path)?;
    let algorithm = request
        .op()
        .get("algorithm")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan("workspace_query digest must carry algorithm"))?;
    let digest = value
        .get("digest")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan("workspace_query digest result must carry digest"))?;
    if !digest.starts_with(&format!("{algorithm}:")) {
        return Err(invalid_plan(format!(
            "workspace_query digest result `{digest}` does not match requested algorithm `{algorithm}`"
        )));
    }
    let workspace = value
        .get("workspace")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan("workspace_query digest result must carry workspace"))?;
    if workspace != request.workspace() {
        return Err(invalid_plan(format!(
            "workspace_query digest result workspace `{workspace}` does not match requested `{}`",
            request.workspace()
        )));
    }
    Ok(())
}

fn validate_workspace_snapshot_value(
    request: PlanWorkspaceQueryRequest<'_>,
    value: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    validate_workspace_snapshot_workspace(request, value, "snapshot")?;
    if value.get("digest").and_then(Value::as_str).is_none() {
        return Err(invalid_plan(
            "workspace_query snapshot result must carry digest",
        ));
    }
    Ok(())
}

fn validate_workspace_diff_value(
    request: PlanWorkspaceQueryRequest<'_>,
    value: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    if value.get("text").and_then(Value::as_str).is_none() && value.get("blob_ref").is_none() {
        return Err(invalid_plan(format!(
            "workspace_query {} result must carry text or blob_ref",
            request.op_kind()?
        )));
    }
    Ok(())
}

fn validate_workspace_capture_artifacts_value(
    request: PlanWorkspaceQueryRequest<'_>,
    value: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let requested = workspace_capture_requested_paths(request)?;
    let entries = workspace_listing_entries(value, "capture_artifacts")?;
    for entry in entries {
        let entry_path = workspace_listing_entry_path(entry, "capture_artifacts")?;
        validate_workspace_path("workspace_query capture_artifacts entry path", entry_path)?;
        if !requested
            .iter()
            .any(|requested_path| path_is_at_or_below(entry_path, requested_path))
        {
            return Err(invalid_plan(format!(
                "workspace_query capture_artifacts result path `{entry_path}` was not requested"
            )));
        }
    }
    Ok(())
}

fn validate_workspace_query_request_op(
    request: PlanWorkspaceQueryRequest<'_>,
) -> Result<(), PublicSeamError> {
    match request.op_kind()? {
        "read_file" => {
            let path = request
                .path()?
                .ok_or_else(|| invalid_plan("workspace_query read_file must carry path"))?;
            validate_workspace_path("workspace_query read_file path", path)?;
        }
        "list" => {
            let path = request
                .path()?
                .ok_or_else(|| invalid_plan("workspace_query list must carry path"))?;
            validate_workspace_path("workspace_query list path", path)?;
        }
        "stat" => {
            let path = request
                .path()?
                .ok_or_else(|| invalid_plan("workspace_query stat must carry path"))?;
            validate_workspace_path("workspace_query stat path", path)?;
        }
        "digest" => {
            let path = request
                .path()?
                .ok_or_else(|| invalid_plan("workspace_query digest must carry path"))?;
            validate_workspace_path("workspace_query digest path", path)?;
        }
        "snapshot" | "git_log" | "git_diff" | "git_status" => {}
        "capture_artifacts" => {
            workspace_capture_requested_paths(request)?;
        }
        other => {
            return Err(invalid_plan(format!(
                "unknown workspace_query op `{other}`"
            )));
        }
    }
    Ok(())
}

fn workspace_capture_requested_paths(
    request: PlanWorkspaceQueryRequest<'_>,
) -> Result<BTreeSet<&str>, PublicSeamError> {
    let mut requested = BTreeSet::new();
    let paths = request
        .op()
        .get("paths")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("workspace_query capture_artifacts must carry paths"))?;
    if paths.is_empty() {
        return Err(invalid_plan(
            "workspace_query capture_artifacts must request at least one path",
        ));
    }
    for path in paths {
        let path = path.as_str().ok_or_else(|| {
            invalid_plan("workspace_query capture_artifacts paths must be strings")
        })?;
        validate_workspace_path("workspace_query capture_artifacts path", path)?;
        requested.insert(path);
    }
    Ok(requested)
}

fn validate_workspace_snapshot_workspace(
    request: PlanWorkspaceQueryRequest<'_>,
    value: &Map<String, Value>,
    op: &str,
) -> Result<(), PublicSeamError> {
    let workspace = value
        .get("workspace")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("workspace_query {op} result must carry workspace")))?;
    if workspace != request.workspace() {
        return Err(invalid_plan(format!(
            "workspace_query {op} result workspace `{workspace}` does not match requested `{}`",
            request.workspace()
        )));
    }
    Ok(())
}

fn workspace_listing_entries<'a>(
    value: &'a Map<String, Value>,
    op: &str,
) -> Result<&'a Vec<Value>, PublicSeamError> {
    value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan(format!("workspace_query {op} result must carry entries")))
}

fn workspace_listing_entry_path<'a>(
    entry: &'a Value,
    op: &str,
) -> Result<&'a str, PublicSeamError> {
    entry
        .as_object()
        .and_then(|entry| entry.get("path"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("workspace_query {op} entry must carry path")))
}

fn path_is_at_or_below(path: &str, root: &str) -> bool {
    let root = root.trim_end_matches('/');
    if root == "." || root.is_empty() {
        return true;
    }
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn validate_workspace_path(field: &str, path: &str) -> Result<(), PublicSeamError> {
    if path == "." {
        return Ok(());
    }
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('~')
        || path.contains('\\')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(invalid_plan(format!(
            "{field} must be a relative workspace path without traversal"
        )));
    }
    Ok(())
}

pub(super) fn workspace_query_projection(request: PlanWorkspaceQueryRequest<'_>) -> Value {
    json!({
        "workspace": request.workspace(),
        "op": request.op()
    })
}

pub(super) fn require_requested_case_field(
    include: &BTreeSet<&str>,
    field: &'static str,
) -> Result<(), PublicSeamError> {
    if include.contains(field) {
        Ok(())
    } else {
        Err(invalid_plan(format!(
            "case_query.load host returned unrequested `{field}` material"
        )))
    }
}

pub(super) fn require_included_case_fields(
    value: &Value,
    include: &BTreeSet<&str>,
) -> Result<(), PublicSeamError> {
    for field in include {
        if value.get(*field).is_none() {
            return Err(invalid_plan(format!(
                "case_query.load host omitted requested `{field}` material"
            )));
        }
    }
    Ok(())
}

pub(super) fn case_query_projection(query: &Value) -> Result<Value, PublicSeamError> {
    Ok(json!({
        "case": query
            .get("case")
            .cloned()
            .ok_or_else(|| invalid_plan("case_query.load must carry case"))?,
        "include": query
            .get("include")
            .cloned()
            .ok_or_else(|| invalid_plan("case_query.load must carry include"))?,
        "projection_schema": query.get("projection_schema").cloned().unwrap_or(Value::Null)
    }))
}

pub(super) fn plan_contains_case_query(plan: &Value) -> Result<bool, PublicSeamError> {
    for op in plan_ops(plan)? {
        let Some(expr) = op.get("expr") else {
            continue;
        };
        if nested_kind(expr, "expr")? == "case_query" {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn validate_case_query_authority(
    plan: &Value,
    context: &PlanExecutionContext,
    capability: &CapabilityDocument,
) -> Result<(), PublicSeamError> {
    if context.capability_fingerprint != capability.capability_fingerprint() {
        return Err(invalid_plan(
            "Plan execution context capability fingerprint does not match capability document",
        ));
    }
    for op in plan_ops(plan)? {
        let Some(expr) = op.get("expr") else {
            continue;
        };
        if nested_kind(expr, "expr")? != "case_query" {
            continue;
        }
        let query = object(expr, "case_query")?
            .get("query")
            .ok_or_else(|| invalid_plan("case_query must carry query"))?;
        if nested_kind(query, "case_query.query")? != "load" {
            return Err(invalid_plan(
                "representative Plan IR harness only authorizes case_query.load",
            ));
        }
        authorize_case_query_load(query, context, capability)?;
    }
    Ok(())
}

fn authorize_case_query_load(
    query: &Value,
    context: &PlanExecutionContext,
    capability: &CapabilityDocument,
) -> Result<(), PublicSeamError> {
    let run = context
        .evaluation_run
        .as_deref()
        .ok_or_else(|| invalid_plan("case_query.load authorization requires evaluation run"))?;
    let evaluation_request_id = context.evaluation_request_id.as_deref().ok_or_else(|| {
        invalid_plan("case_query.load authorization requires evaluation_request_id")
    })?;
    ensure_case_ref_run_matches_context(query, run)?;
    let mut request = CapabilityGrantRequest::for_action("case.read")
        .with_resource("run", json!(run))
        .with_resource("evaluation_request_id", json!(evaluation_request_id));
    if let Some(partition) = &context.case_partition {
        request = request.with_partition(partition.clone());
    }
    for field in case_query_include(query)? {
        request = request
            .with_case_field(field)
            .with_input_class(case_field_data_class(field));
    }
    capability
        .authorize_grant(request)
        .map_err(|denial| invalid_plan(format!("case_query.load denied: {denial}")))?;
    Ok(())
}

fn ensure_case_ref_run_matches_context(
    query: &Value,
    expected_run: &str,
) -> Result<(), PublicSeamError> {
    let Some(case_run) = query
        .get("case")
        .and_then(Value::as_object)
        .and_then(|case_ref| case_ref.get("run"))
        .and_then(Value::as_str)
    else {
        return Ok(());
    };
    if case_run == expected_run {
        Ok(())
    } else {
        Err(invalid_plan(
            "case_query.load case ref run does not match evaluator context",
        ))
    }
}

fn case_field_data_class(field: &str) -> &'static str {
    match field {
        "input" => "case.input",
        "target" => "case.target",
        "metadata" => "case.metadata",
        _ => "case.unknown",
    }
}

fn plan_ops(plan: &Value) -> Result<&Vec<Value>, PublicSeamError> {
    plan.get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("plan ops must be an array"))
}
