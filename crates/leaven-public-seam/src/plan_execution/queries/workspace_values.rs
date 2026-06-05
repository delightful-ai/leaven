use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};

use base64::Engine as _;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{CapabilityDocument, CapabilityGrantRequest, PublicSeamError};

use super::{
    PlanExecutionContext, PlanWorkspaceQueryRequest, WorkspaceDigestAlgorithm, WorkspaceQueryOp,
    case_query_include, invalid_plan, nested_kind, object, validate_workspace_path,
    workspace_capture_requested_paths, workspace_query_request_from_values,
};

pub(super) fn workspace_query_value_from_view(
    request: &PlanWorkspaceQueryRequest<'_>,
    view: &leaven_workspace::WorkspaceView<'_>,
    data_classes: &[String],
) -> Result<Value, PublicSeamError> {
    match request.op_kind() {
        "read_file" => workspace_read_file_value_from_view(request, view),
        "list" => workspace_list_value_from_view(request, view, data_classes),
        "stat" => workspace_stat_value_from_view(request, view, data_classes),
        "digest" => workspace_digest_value_from_view(request, view),
        "snapshot" => workspace_snapshot_value_from_view(request, view),
        "capture_artifacts" => {
            workspace_capture_artifacts_value_from_view(request, view, data_classes)
        }
        "git_log" | "git_diff" | "git_status" => Err(invalid_plan(format!(
            "workspace_query `{}` requires a host-provided Git workspace outcome",
            request.op_kind()
        ))),
        other => Err(invalid_plan(format!(
            "unknown workspace_query op `{other}`"
        ))),
    }
}

fn workspace_read_file_value_from_view(
    request: &PlanWorkspaceQueryRequest<'_>,
    view: &leaven_workspace::WorkspaceView<'_>,
) -> Result<Value, PublicSeamError> {
    let path = request
        .path()
        .ok_or_else(|| invalid_plan("workspace_query read_file must carry path"))?;
    let workspace_path = workspace_path(path, "workspace_query read_file path")?;
    let bytes = view
        .read_file(&workspace_path)
        .map_err(|error| workspace_error(&error))?;
    enforce_max_bytes(request.op().max_bytes(), bytes.len() as u64, "read_file")?;
    let content = String::from_utf8(bytes).map_err(|_| {
        invalid_plan(
            "workspace_query read_file produced non-utf8 content; host must provide blob_ref",
        )
    })?;
    Ok(json!({
        "kind": "workspace_file",
        "path": path,
        "content": content
    }))
}

fn workspace_list_value_from_view(
    request: &PlanWorkspaceQueryRequest<'_>,
    view: &leaven_workspace::WorkspaceView<'_>,
    data_classes: &[String],
) -> Result<Value, PublicSeamError> {
    let path = request
        .path()
        .ok_or_else(|| invalid_plan("workspace_query list must carry path"))?;
    let workspace_path = workspace_path(path, "workspace_query list path")?;
    let WorkspaceQueryOp::List {
        recursive,
        max_entries,
        ..
    } = request.op()
    else {
        return Err(invalid_plan("workspace_query list must carry typed op"));
    };
    let recursive = recursive.unwrap_or(true);
    let mut paths = view
        .list_files(&workspace_path)
        .map_err(|error| workspace_error(&error))?;
    paths = filter_list_recursion(paths, &workspace_path, recursive);
    paths.sort();
    if let Some(max_entries) = max_entries {
        let max_entries = usize::try_from(*max_entries).map_err(|_| {
            invalid_plan("workspace_query list max_entries exceeds platform addressable entries")
        })?;
        paths.truncate(max_entries);
    }
    Ok(workspace_listing_value_from_paths(paths, data_classes))
}

fn workspace_stat_value_from_view(
    request: &PlanWorkspaceQueryRequest<'_>,
    view: &leaven_workspace::WorkspaceView<'_>,
    data_classes: &[String],
) -> Result<Value, PublicSeamError> {
    let path = request
        .path()
        .ok_or_else(|| invalid_plan("workspace_query stat must carry path"))?;
    let workspace_path = workspace_path(path, "workspace_query stat path")?;
    let bytes = view
        .read_file(&workspace_path)
        .map_err(|error| workspace_error(&error))?
        .len() as u64;
    Ok(json!({
        "kind": "workspace_listing",
        "entries": [{
            "path": path,
            "kind": "file",
            "bytes": bytes,
            "data_classes": data_classes
        }]
    }))
}

fn workspace_digest_value_from_view(
    request: &PlanWorkspaceQueryRequest<'_>,
    view: &leaven_workspace::WorkspaceView<'_>,
) -> Result<Value, PublicSeamError> {
    let path = request
        .path()
        .ok_or_else(|| invalid_plan("workspace_query digest must carry path"))?;
    let workspace_path = workspace_path(path, "workspace_query digest path")?;
    let WorkspaceQueryOp::Digest { algorithm, .. } = request.op() else {
        return Err(invalid_plan("workspace_query digest must carry algorithm"));
    };
    let bytes = view
        .read_file(&workspace_path)
        .map_err(|error| workspace_error(&error))?;
    let digest = match algorithm {
        WorkspaceDigestAlgorithm::Sha256 => format!("sha256:{:x}", Sha256::digest(&bytes)),
        WorkspaceDigestAlgorithm::Blake3 => {
            let mut builder = leaven_kernel::FingerprintBuilder::new();
            builder.update(&bytes);
            format!("blake3:{}", hex_bytes(builder.finish().0.as_slice()))
        }
    };
    Ok(json!({
        "kind": "workspace_snapshot",
        "workspace": request.workspace(),
        "digest": digest,
        "source_refs": [{
            "kind": "external",
            "namespace": "leaven.workspace.path",
            "id": path
        }]
    }))
}

fn workspace_snapshot_value_from_view(
    request: &PlanWorkspaceQueryRequest<'_>,
    view: &leaven_workspace::WorkspaceView<'_>,
) -> Result<Value, PublicSeamError> {
    let tree = leaven_workspace::fingerprint_tree(view, &leaven_workspace::WorkspacePath::root())
        .map_err(|error| workspace_error(&error))?;
    Ok(json!({
        "kind": "workspace_snapshot",
        "workspace": request.workspace(),
        "digest": format!("blake3:{}", hex_bytes(tree.fingerprint.0.as_slice()))
    }))
}

fn workspace_capture_artifacts_value_from_view(
    request: &PlanWorkspaceQueryRequest<'_>,
    view: &leaven_workspace::WorkspaceView<'_>,
    data_classes: &[String],
) -> Result<Value, PublicSeamError> {
    let mut paths = Vec::new();
    for path in workspace_capture_requested_paths(request)? {
        let workspace_path = workspace_path(path, "workspace_query capture_artifacts path")?;
        match view.list_files(&workspace_path) {
            Ok(files) if !files.is_empty() => paths.extend(files),
            _ => {
                view.read_file(&workspace_path)
                    .map_err(|error| workspace_error(&error))?;
                paths.push(workspace_path);
            }
        }
    }
    paths.sort();
    paths.dedup();
    let mut total_bytes = 0_u64;
    let entries = paths
        .into_iter()
        .map(|path| {
            let bytes = view
                .read_file(&path)
                .map_err(|error| workspace_error(&error))?;
            let byte_count = bytes.len() as u64;
            total_bytes = total_bytes.checked_add(byte_count).ok_or_else(|| {
                invalid_plan("workspace_query capture_artifacts byte count overflowed")
            })?;
            enforce_max_bytes(request.op().max_bytes(), total_bytes, "capture_artifacts")?;
            let sha256 = format!("{:x}", Sha256::digest(&bytes));
            let blob_id = format!("blob_workspace_capture_{sha256}");
            Ok(json!({
                "path": path.as_str(),
                "kind": "file",
                "bytes": byte_count,
                "sha256": sha256,
                "content_base64": base64::engine::general_purpose::STANDARD.encode(&bytes),
                "blob_ref": {
                    "kind": "blob_ref",
                    "id": blob_id,
                    "sha256": sha256,
                    "bytes": byte_count,
                    "uri": format!("leaven-blob://workspace.capture_artifacts/{sha256}"),
                    "data_classes": data_classes
                },
                "data_classes": data_classes
            }))
        })
        .collect::<Result<Vec<_>, PublicSeamError>>()?;
    Ok(json!({
        "kind": "workspace_listing",
        "entries": entries
    }))
}

fn workspace_listing_value_from_paths(
    paths: Vec<leaven_workspace::WorkspacePath>,
    data_classes: &[String],
) -> Value {
    let entries = paths
        .into_iter()
        .map(|path| {
            json!({
                "path": path.as_str(),
                "kind": "file",
                "data_classes": data_classes
            })
        })
        .collect::<Vec<_>>();
    json!({
        "kind": "workspace_listing",
        "entries": entries
    })
}

fn workspace_path(
    path: &str,
    field: &'static str,
) -> Result<leaven_workspace::WorkspacePath, PublicSeamError> {
    validate_workspace_path(field, path)?;
    if path == "." {
        return Ok(leaven_workspace::WorkspacePath::root());
    }
    leaven_workspace::WorkspacePath::new(path).map_err(|error| {
        invalid_plan(format!(
            "{field} must be a relative workspace path without traversal: {error}"
        ))
    })
}

fn workspace_error(error: &leaven_workspace::WorkspaceError) -> PublicSeamError {
    let message = error.to_string();
    invalid_plan(format!("workspace_query workspace view failed: {message}"))
}

fn enforce_max_bytes(
    max_bytes: Option<u64>,
    bytes: u64,
    context: &'static str,
) -> Result<(), PublicSeamError> {
    if let Some(max_bytes) = max_bytes
        && bytes > max_bytes
    {
        return Err(invalid_plan(format!(
            "workspace_query {context} exceeded max_bytes; host must provide a bounded blob_ref outcome"
        )));
    }
    Ok(())
}

fn filter_list_recursion(
    paths: Vec<leaven_workspace::WorkspacePath>,
    root: &leaven_workspace::WorkspacePath,
    recursive: bool,
) -> Vec<leaven_workspace::WorkspacePath> {
    if recursive {
        return paths;
    }
    let root = root.as_str();
    paths
        .into_iter()
        .filter(|path| {
            let raw = path.as_str();
            let relative = if root.is_empty() {
                raw
            } else if raw == root {
                ""
            } else if let Some(stripped) = raw.strip_prefix(root) {
                stripped.strip_prefix('/').unwrap_or(stripped)
            } else {
                raw
            };
            !relative.contains('/')
        })
        .collect()
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    hex
}

pub(in crate::plan_execution) fn plan_contains_case_query(
    plan: &Value,
) -> Result<bool, PublicSeamError> {
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

pub(in crate::plan_execution) fn plan_contains_workspace_query(
    plan: &Value,
) -> Result<bool, PublicSeamError> {
    for op in plan_ops(plan)? {
        let Some(expr) = op.get("expr") else {
            continue;
        };
        if nested_kind(expr, "expr")? == "workspace_query" {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(in crate::plan_execution) fn validate_case_query_authority(
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

pub(in crate::plan_execution) fn validate_workspace_query_authority(
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
        if nested_kind(expr, "expr")? != "workspace_query" {
            continue;
        }
        let name = op
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_plan("workspace_query op must carry name"))?;
        let deps = BTreeMap::new();
        let request = workspace_query_request_from_values(name, expr, &deps)?;
        authorize_workspace_query_read(&request, capability)?;
    }
    Ok(())
}

fn authorize_workspace_query_read(
    request: &PlanWorkspaceQueryRequest<'_>,
    capability: &CapabilityDocument,
) -> Result<(), PublicSeamError> {
    let op_kind = request.op_kind();
    let input_classes = workspace_query_authorized_input_classes(request)?;
    let mut grant = CapabilityGrantRequest::for_action("workspace.read")
        .with_resource("workspace_ids", json!(request.workspace()))
        .with_workspace_op(op_kind);
    for data_class in &input_classes {
        grant = grant.with_input_class(data_class);
    }
    capability.authorize_grant(grant).map_err(|denial| {
        invalid_plan(format!(
            "workspace_query `{op_kind}` denied for input classes {input_classes:?}: {denial}"
        ))
    })?;
    Ok(())
}

fn workspace_query_authorized_input_classes(
    request: &PlanWorkspaceQueryRequest<'_>,
) -> Result<BTreeSet<String>, PublicSeamError> {
    if request.op_kind() == "read_file" {
        return Ok(request
            .expected_data_classes()
            .into_iter()
            .map(str::to_owned)
            .collect());
    }
    Ok(BTreeSet::from(["candidate.artifact".to_owned()]))
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
