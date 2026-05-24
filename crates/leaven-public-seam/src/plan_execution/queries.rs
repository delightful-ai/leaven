use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::{CapabilityDocument, CapabilityGrantRequest, PublicSeamError};

use super::{
    PlanExecutionContext,
    effects::{
        LiveWorkspaceHandle, WorkspaceRefFacts, require_live_workspace_ref, workspace_ref_facts,
    },
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
#[derive(Clone, Debug)]
pub struct PlanWorkspaceQueryRequest<'a> {
    pub(super) name: &'a str,
    pub(super) expr: &'a Value,
    pub(super) workspace: WorkspaceRefFacts,
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
    pub fn workspace(&self) -> &str {
        self.workspace.id()
    }

    pub(super) const fn workspace_ref(&self) -> &WorkspaceRefFacts {
        &self.workspace
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

    /// Executes finite workspace reads through the provider-neutral workspace substrate.
    ///
    /// Git-specific queries remain host-owned because the V1 workspace substrate
    /// does not expose Git preimage fields such as `against`, `porcelain`, or log
    /// entry structure.
    pub fn execute_on_workspace_view(
        &self,
        view: &leaven_workspace::WorkspaceView<'_>,
        graph_revision: impl Into<String>,
        data_classes: impl IntoIterator<Item = String>,
    ) -> Result<PlanWorkspaceQueryOutcome, PublicSeamError> {
        let data_classes = data_classes.into_iter().collect::<Vec<_>>();
        let value = workspace_query_value_from_view(self, view, &data_classes)?;
        Ok(PlanWorkspaceQueryOutcome::new(value, graph_revision).with_data_classes(data_classes))
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
    require_live_workspace_ref(
        request.workspace_ref(),
        deps,
        live_workspaces,
        "workspace_query",
    )?;
    Ok(request)
}

pub(super) fn workspace_query_request_from_values<'a>(
    name: &'a str,
    expr: &'a Value,
    deps: &'a BTreeMap<String, Value>,
) -> Result<PlanWorkspaceQueryRequest<'a>, PublicSeamError> {
    let object = object(expr, "workspace_query")?;
    let workspace = workspace_ref_facts(
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
    validate_workspace_query_request_op(&request)?;
    Ok(request)
}

pub(super) fn workspace_query_expected_value_kind(
    request: &PlanWorkspaceQueryRequest<'_>,
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
    request: &PlanWorkspaceQueryRequest<'_>,
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
    request: &PlanWorkspaceQueryRequest<'_>,
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
    request: &PlanWorkspaceQueryRequest<'_>,
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
    request: &PlanWorkspaceQueryRequest<'_>,
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
    request: &PlanWorkspaceQueryRequest<'_>,
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
    require_external_source_ref(
        value,
        "leaven.workspace.path",
        requested_path,
        "workspace_query digest result",
    )?;
    Ok(())
}

fn validate_workspace_snapshot_value(
    request: &PlanWorkspaceQueryRequest<'_>,
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
    request: &PlanWorkspaceQueryRequest<'_>,
    value: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    if value.get("text").and_then(Value::as_str).is_none() && value.get("blob_ref").is_none() {
        return Err(invalid_plan(format!(
            "workspace_query {} result must carry text or blob_ref",
            request.op_kind()?
        )));
    }
    match request.op_kind()? {
        "git_log" => {
            if let Some(max_entries) = request.op().get("max_entries").and_then(Value::as_u64) {
                require_external_source_ref(
                    value,
                    "leaven.workspace.git_log.max_entries",
                    &max_entries.to_string(),
                    "workspace_query git_log result",
                )?;
            }
        }
        "git_diff" => {
            let against = request
                .op()
                .get("against")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_plan("workspace_query git_diff must carry against"))?;
            require_external_source_ref(
                value,
                "leaven.workspace.git_diff.against",
                against,
                "workspace_query git_diff result",
            )?;
        }
        "git_status" => {
            if let Some(porcelain) = request.op().get("porcelain").and_then(Value::as_bool) {
                require_external_source_ref(
                    value,
                    "leaven.workspace.git_status.porcelain",
                    if porcelain { "true" } else { "false" },
                    "workspace_query git_status result",
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_workspace_capture_artifacts_value(
    request: &PlanWorkspaceQueryRequest<'_>,
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
    request: &PlanWorkspaceQueryRequest<'_>,
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

fn workspace_capture_requested_paths<'a>(
    request: &'a PlanWorkspaceQueryRequest<'a>,
) -> Result<BTreeSet<&'a str>, PublicSeamError> {
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
    request: &PlanWorkspaceQueryRequest<'_>,
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

fn require_external_source_ref(
    value: &Map<String, Value>,
    namespace: &str,
    id: &str,
    context: &str,
) -> Result<(), PublicSeamError> {
    let refs = value
        .get("source_refs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan(format!("{context} must carry source_refs")))?;
    let found = refs.iter().any(|source| {
        source.as_object().is_some_and(|source| {
            source.get("kind").and_then(Value::as_str) == Some("external")
                && source.get("namespace").and_then(Value::as_str) == Some(namespace)
                && source.get("id").and_then(Value::as_str) == Some(id)
        })
    });
    if found {
        Ok(())
    } else {
        Err(invalid_plan(format!(
            "{context} source_refs must include external `{namespace}` id `{id}`"
        )))
    }
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

pub(super) fn workspace_query_projection(request: &PlanWorkspaceQueryRequest<'_>) -> Value {
    json!({
        "workspace": request.workspace_ref().to_value(),
        "op": request.op()
    })
}

include!("queries/workspace_values.rs");
