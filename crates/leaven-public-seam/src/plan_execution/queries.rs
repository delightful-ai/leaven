use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use crate::PublicSeamError;

mod case;
mod graph;
mod workspace_values;

pub use case::{PlanCaseQueryOutcome, PlanCaseQueryRequest};
pub use graph::{PlanGraphQueryOutcome, PlanGraphQueryRequest, PlanGraphReadScope};

pub(super) use case::{
    case_query_include, case_query_projection, require_included_case_fields,
    require_requested_case_field,
};
use workspace_values::workspace_query_value_from_view;
pub(super) use workspace_values::{
    plan_contains_case_query, plan_contains_workspace_query, validate_case_query_authority,
    validate_workspace_query_authority,
};

use super::{
    PlanExecutionContext,
    effects::{
        LiveWorkspaceHandle, WorkspaceRefFacts, require_live_workspace_ref, workspace_ref_facts,
    },
    invalid_plan, nested_kind, object,
};

/// Lowered `workspace_query` request passed to a plan execution host.
#[derive(Clone, Debug)]
pub struct PlanWorkspaceQueryRequest<'a> {
    pub(super) name: &'a str,
    pub(super) expr: &'a Value,
    pub(super) workspace: WorkspaceRefFacts,
    pub(super) op: WorkspaceQueryOp<'a>,
    pub(super) deps: &'a BTreeMap<String, Value>,
}

/// Typed `workspace_query.op` shape from the locked Plan IR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceQueryOp<'a> {
    Snapshot,
    List {
        path: &'a str,
        recursive: Option<bool>,
        max_entries: Option<u64>,
    },
    ReadFile {
        path: &'a str,
        max_bytes: Option<u64>,
        expected_data_classes: BTreeSet<&'a str>,
    },
    Stat {
        path: &'a str,
    },
    Digest {
        path: &'a str,
        algorithm: WorkspaceDigestAlgorithm,
    },
    GitLog {
        max_entries: Option<u64>,
    },
    GitDiff {
        against: WorkspaceGitAgainst,
        max_bytes: Option<u64>,
    },
    GitStatus {
        porcelain: Option<bool>,
    },
    CaptureArtifacts {
        paths: BTreeSet<&'a str>,
        max_bytes: Option<u64>,
    },
}

impl<'a> WorkspaceQueryOp<'a> {
    fn from_value(value: &'a Value) -> Result<Self, PublicSeamError> {
        let object = object(value, "workspace_query op")?;
        match object
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_plan("workspace_query op must carry kind"))?
        {
            "snapshot" => Ok(Self::Snapshot),
            "list" => Ok(Self::List {
                path: required_workspace_op_path(object, "list")?,
                recursive: optional_bool(object, "recursive", "workspace_query list.recursive")?,
                max_entries: optional_positive_u64(
                    object,
                    "max_entries",
                    "workspace_query list.max_entries",
                )?,
            }),
            "read_file" => Ok(Self::ReadFile {
                path: required_workspace_op_path(object, "read_file")?,
                max_bytes: optional_positive_u64(
                    object,
                    "max_bytes",
                    "workspace_query read_file.max_bytes",
                )?,
                expected_data_classes: required_string_set(
                    object,
                    "expected_data_classes",
                    "workspace_query read_file.expected_data_classes",
                )?,
            }),
            "stat" => Ok(Self::Stat {
                path: required_workspace_op_path(object, "stat")?,
            }),
            "digest" => Ok(Self::Digest {
                path: required_workspace_op_path(object, "digest")?,
                algorithm: WorkspaceDigestAlgorithm::from_str(required_object_str(
                    object,
                    "algorithm",
                    "workspace_query digest.algorithm",
                )?)?,
            }),
            "git_log" => Ok(Self::GitLog {
                max_entries: optional_positive_u64(
                    object,
                    "max_entries",
                    "workspace_query git_log.max_entries",
                )?,
            }),
            "git_diff" => Ok(Self::GitDiff {
                against: WorkspaceGitAgainst::from_str(required_object_str(
                    object,
                    "against",
                    "workspace_query git_diff.against",
                )?)?,
                max_bytes: optional_positive_u64(
                    object,
                    "max_bytes",
                    "workspace_query git_diff.max_bytes",
                )?,
            }),
            "git_status" => Ok(Self::GitStatus {
                porcelain: optional_bool(
                    object,
                    "porcelain",
                    "workspace_query git_status.porcelain",
                )?,
            }),
            "capture_artifacts" => Ok(Self::CaptureArtifacts {
                paths: required_string_set(
                    object,
                    "paths",
                    "workspace_query capture_artifacts.paths",
                )?,
                max_bytes: optional_positive_u64(
                    object,
                    "max_bytes",
                    "workspace_query capture_artifacts.max_bytes",
                )?,
            }),
            other => Err(invalid_plan(format!(
                "unknown workspace_query op `{other}`"
            ))),
        }
    }

    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Snapshot => "snapshot",
            Self::List { .. } => "list",
            Self::ReadFile { .. } => "read_file",
            Self::Stat { .. } => "stat",
            Self::Digest { .. } => "digest",
            Self::GitLog { .. } => "git_log",
            Self::GitDiff { .. } => "git_diff",
            Self::GitStatus { .. } => "git_status",
            Self::CaptureArtifacts { .. } => "capture_artifacts",
        }
    }

    pub const fn path(&self) -> Option<&'a str> {
        match self {
            Self::List { path, .. }
            | Self::ReadFile { path, .. }
            | Self::Stat { path }
            | Self::Digest { path, .. } => Some(path),
            Self::Snapshot
            | Self::GitLog { .. }
            | Self::GitDiff { .. }
            | Self::GitStatus { .. }
            | Self::CaptureArtifacts { .. } => None,
        }
    }

    pub const fn max_bytes(&self) -> Option<u64> {
        match self {
            Self::ReadFile { max_bytes, .. }
            | Self::GitDiff { max_bytes, .. }
            | Self::CaptureArtifacts { max_bytes, .. } => *max_bytes,
            Self::Snapshot
            | Self::List { .. }
            | Self::Stat { .. }
            | Self::Digest { .. }
            | Self::GitLog { .. }
            | Self::GitStatus { .. } => None,
        }
    }

    pub fn to_value(&self) -> Value {
        match self {
            Self::Snapshot => json!({"kind": "snapshot"}),
            Self::List {
                path,
                recursive,
                max_entries,
            } => {
                let mut value = json!({"kind": "list", "path": path});
                insert_optional(&mut value, "recursive", recursive.map(Value::Bool));
                insert_optional(
                    &mut value,
                    "max_entries",
                    max_entries.map(|entry| json!(entry)),
                );
                value
            }
            Self::ReadFile {
                path,
                max_bytes,
                expected_data_classes,
            } => {
                let mut value = json!({
                    "kind": "read_file",
                    "path": path,
                    "expected_data_classes": expected_data_classes
                });
                insert_optional(&mut value, "max_bytes", max_bytes.map(|bytes| json!(bytes)));
                value
            }
            Self::Stat { path } => json!({"kind": "stat", "path": path}),
            Self::Digest { path, algorithm } => {
                json!({"kind": "digest", "path": path, "algorithm": algorithm.as_str()})
            }
            Self::GitLog { max_entries } => {
                let mut value = json!({"kind": "git_log"});
                insert_optional(
                    &mut value,
                    "max_entries",
                    max_entries.map(|entry| json!(entry)),
                );
                value
            }
            Self::GitDiff { against, max_bytes } => {
                let mut value = json!({"kind": "git_diff", "against": against.as_str()});
                insert_optional(&mut value, "max_bytes", max_bytes.map(|bytes| json!(bytes)));
                value
            }
            Self::GitStatus { porcelain } => {
                let mut value = json!({"kind": "git_status"});
                insert_optional(&mut value, "porcelain", porcelain.map(Value::Bool));
                value
            }
            Self::CaptureArtifacts { paths, max_bytes } => {
                let mut value = json!({"kind": "capture_artifacts", "paths": paths});
                insert_optional(&mut value, "max_bytes", max_bytes.map(|bytes| json!(bytes)));
                value
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceDigestAlgorithm {
    Sha256,
    Blake3,
}

impl WorkspaceDigestAlgorithm {
    fn from_str(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "sha256" => Ok(Self::Sha256),
            "blake3" => Ok(Self::Blake3),
            other => Err(invalid_plan(format!(
                "workspace_query digest algorithm `{other}` is not supported"
            ))),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
            Self::Blake3 => "blake3",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceGitAgainst {
    Seed,
    Parent,
    Baseline,
    Head,
}

impl WorkspaceGitAgainst {
    fn from_str(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "seed" => Ok(Self::Seed),
            "parent" => Ok(Self::Parent),
            "baseline" => Ok(Self::Baseline),
            "head" => Ok(Self::Head),
            other => Err(invalid_plan(format!(
                "workspace_query git_diff against `{other}` is not supported"
            ))),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Seed => "seed",
            Self::Parent => "parent",
            Self::Baseline => "baseline",
            Self::Head => "head",
        }
    }
}

fn required_object_str<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a str, PublicSeamError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("{context} must be a string")))
}

fn required_workspace_op_path<'a>(
    object: &'a Map<String, Value>,
    op: &str,
) -> Result<&'a str, PublicSeamError> {
    let path = required_object_str(object, "path", &format!("workspace_query {op}.path"))?;
    validate_workspace_path(&format!("workspace_query {op} path"), path)?;
    Ok(path)
}

fn required_string_set<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<BTreeSet<&'a str>, PublicSeamError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan(format!("{context} must be an array")))?;
    let mut set = BTreeSet::new();
    for value in values {
        let string = value
            .as_str()
            .ok_or_else(|| invalid_plan(format!("{context} entries must be strings")))?;
        set.insert(string);
    }
    Ok(set)
}

fn optional_bool(
    object: &Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<Option<bool>, PublicSeamError> {
    object
        .get(key)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| invalid_plan(format!("{context} must be a boolean")))
        })
        .transpose()
}

fn optional_positive_u64(
    object: &Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<Option<u64>, PublicSeamError> {
    object
        .get(key)
        .map(|value| {
            let value = value
                .as_u64()
                .ok_or_else(|| invalid_plan(format!("{context} must be a positive integer")))?;
            if value == 0 {
                return Err(invalid_plan(format!(
                    "{context} must be greater than or equal to 1"
                )));
            }
            Ok(value)
        })
        .transpose()
}

fn insert_optional(value: &mut Value, key: &'static str, field: Option<Value>) {
    if let (Value::Object(object), Some(field)) = (value, field) {
        object.insert(key.to_owned(), field);
    }
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
    pub const fn op(&self) -> &WorkspaceQueryOp<'a> {
        &self.op
    }

    /// Resolved dependency bindings visible to this query.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Workspace query operation kind.
    pub const fn op_kind(&self) -> &'static str {
        self.op.kind()
    }

    /// Workspace path requested by path-shaped operations.
    pub const fn path(&self) -> Option<&'a str> {
        self.op.path()
    }

    /// Expected data classes declared by `read_file`.
    pub fn expected_data_classes(&self) -> BTreeSet<&'a str> {
        match &self.op {
            WorkspaceQueryOp::ReadFile {
                expected_data_classes,
                ..
            } => expected_data_classes.clone(),
            WorkspaceQueryOp::Snapshot
            | WorkspaceQueryOp::List { .. }
            | WorkspaceQueryOp::Stat { .. }
            | WorkspaceQueryOp::Digest { .. }
            | WorkspaceQueryOp::GitLog { .. }
            | WorkspaceQueryOp::GitDiff { .. }
            | WorkspaceQueryOp::GitStatus { .. }
            | WorkspaceQueryOp::CaptureArtifacts { .. } => BTreeSet::new(),
        }
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
    let op = WorkspaceQueryOp::from_value(
        object
            .get("op")
            .ok_or_else(|| invalid_plan("workspace_query must carry op"))?,
    )?;
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
) -> &'static str {
    match request.op_kind() {
        "snapshot" | "digest" => "workspace_snapshot",
        "list" | "stat" | "capture_artifacts" => "workspace_listing",
        "read_file" => "workspace_file",
        "git_log" | "git_diff" | "git_status" => "workspace_diff",
        _ => unreachable!("workspace query op was parsed before dispatch"),
    }
}

pub(super) fn validate_workspace_query_value_shape(
    request: &PlanWorkspaceQueryRequest<'_>,
    value: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    match request.op_kind() {
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
        .path()
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
        .path()
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
        .path()
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
        .path()
        .ok_or_else(|| invalid_plan("workspace_query digest must carry path"))?;
    validate_workspace_path("workspace_query digest path", requested_path)?;
    let WorkspaceQueryOp::Digest { algorithm, .. } = request.op() else {
        return Err(invalid_plan("workspace_query digest must carry algorithm"));
    };
    let digest = value
        .get("digest")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan("workspace_query digest result must carry digest"))?;
    if !digest.starts_with(&format!("{}:", algorithm.as_str())) {
        return Err(invalid_plan(format!(
            "workspace_query digest result `{digest}` does not match requested algorithm `{}`",
            algorithm.as_str()
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
            request.op_kind()
        )));
    }
    match request.op_kind() {
        "git_log" => {
            if let WorkspaceQueryOp::GitLog {
                max_entries: Some(max_entries),
            } = request.op()
            {
                require_external_source_ref(
                    value,
                    "leaven.workspace.git_log.max_entries",
                    &max_entries.to_string(),
                    "workspace_query git_log result",
                )?;
            }
        }
        "git_diff" => {
            let WorkspaceQueryOp::GitDiff { against, .. } = request.op() else {
                return Err(invalid_plan("workspace_query git_diff must carry against"));
            };
            require_external_source_ref(
                value,
                "leaven.workspace.git_diff.against",
                against.as_str(),
                "workspace_query git_diff result",
            )?;
        }
        "git_status" => {
            if let WorkspaceQueryOp::GitStatus {
                porcelain: Some(porcelain),
            } = request.op()
            {
                require_external_source_ref(
                    value,
                    "leaven.workspace.git_status.porcelain",
                    if *porcelain { "true" } else { "false" },
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
    match request.op_kind() {
        "read_file" => {
            let path = request
                .path()
                .ok_or_else(|| invalid_plan("workspace_query read_file must carry path"))?;
            validate_workspace_path("workspace_query read_file path", path)?;
        }
        "list" => {
            let path = request
                .path()
                .ok_or_else(|| invalid_plan("workspace_query list must carry path"))?;
            validate_workspace_path("workspace_query list path", path)?;
        }
        "stat" => {
            let path = request
                .path()
                .ok_or_else(|| invalid_plan("workspace_query stat must carry path"))?;
            validate_workspace_path("workspace_query stat path", path)?;
        }
        "digest" => {
            let path = request
                .path()
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
    let WorkspaceQueryOp::CaptureArtifacts { paths, .. } = request.op() else {
        return Err(invalid_plan(
            "workspace_query capture_artifacts must carry paths",
        ));
    };
    if paths.is_empty() {
        return Err(invalid_plan(
            "workspace_query capture_artifacts must request at least one path",
        ));
    }
    for path in paths {
        validate_workspace_path("workspace_query capture_artifacts path", path)?;
    }
    Ok(paths.clone())
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
        "op": request.op().to_value()
    })
}
