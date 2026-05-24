use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::{
    PlanExecutionContext, PlanWorkspaceQueryRequest, case_query_projection, dependency_values,
    effects::{LiveWorkspaceHandle, require_live_workspace, workspace_ref_id},
    graph_read_scope, graph_read_scope_value, invalid_plan, nested_kind, object, prefixed_jcs_hash,
    required_string, validate_workspace_query_value_shape, workspace_query_expected_value_kind,
    workspace_query_projection, workspace_query_request_from_values,
};
use crate::PublicSeamError;

pub fn validate_plan_result_receipts(
    plan: &Value,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    result: &Value,
) -> Result<(), PublicSeamError> {
    let plan_object = object(plan, "plan")?;
    let ops = plan_object
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("plan ops must be an array"))?;
    let result_object = object(result, "plan result")?;
    let values = result_object
        .get("values")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_plan("plan result values must be an object"))?;
    let receipts = result_object
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("plan result receipts must be an array"))?;
    match plan_document.mode_kind() {
        "dry_run" => {
            if receipts.is_empty() {
                return Ok(());
            }
            return Err(invalid_plan(
                "dry_run Plan Result must not carry operation receipts",
            ));
        }
        "replay" => {
            return Err(invalid_plan(
                "replay mode receipts are supplied artifacts and cannot prove Plan IR preimages",
            ));
        }
        _ => {}
    }
    let receipts_by_op = receipts_by_op_var(receipts)?;
    let mut state = ReceiptValidationState::default();
    let mut seen_receipt_ops = BTreeSet::new();
    for op in ops {
        let name = required_string(object(op, "plan op")?.get("name"), "op.name")?;
        if receipts_by_op.contains_key(name) {
            seen_receipt_ops.insert(name.to_owned());
        }
        validate_op_receipt(
            op,
            plan_document,
            context,
            values,
            &receipts_by_op,
            &mut state,
        )?;
    }
    for op_var in receipts_by_op.keys() {
        if !seen_receipt_ops.contains(op_var) {
            return Err(invalid_plan(format!(
                "receipt claims operation `{op_var}` that is not present in the Plan IR"
            )));
        }
    }
    Ok(())
}

#[derive(Default)]
struct ReceiptValidationState {
    bindings: BTreeMap<String, Value>,
    live_workspaces: BTreeMap<String, LiveWorkspaceHandle>,
}

fn validate_op_receipt(
    op: &Value,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    values: &Map<String, Value>,
    receipts_by_op: &BTreeMap<String, &Map<String, Value>>,
    state: &mut ReceiptValidationState,
) -> Result<(), PublicSeamError> {
    let op_object = object(op, "plan op")?;
    let name = required_string(op_object.get("name"), "op.name")?;
    match required_string(op_object.get("kind"), "op.kind")? {
        "let" => validate_let_receipt(
            op_object,
            name,
            plan_document,
            context,
            values,
            receipts_by_op,
            state,
        ),
        "call" => validate_call_receipt(op_object, name, values, receipts_by_op, state),
        "write" => validate_write_receipt(op_object, name, context, receipts_by_op, state),
        other => Err(invalid_plan(format!(
            "unknown plan operation kind `{other}`"
        ))),
    }
}

fn validate_let_receipt(
    op_object: &Map<String, Value>,
    name: &str,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    values: &Map<String, Value>,
    receipts_by_op: &BTreeMap<String, &Map<String, Value>>,
    state: &mut ReceiptValidationState,
) -> Result<(), PublicSeamError> {
    let expr = op_object
        .get("expr")
        .ok_or_else(|| invalid_plan("let op must carry expr"))?;
    match nested_kind(expr, "expr")? {
        "literal" => {
            let value = object(expr, "literal expr")?
                .get("value")
                .cloned()
                .ok_or_else(|| invalid_plan("literal expr must carry value"))?;
            state.bindings.insert(name.to_owned(), value);
            Ok(())
        }
        "graph_query" => validate_graph_query_receipt(
            expr,
            name,
            plan_document,
            context,
            values,
            receipts_by_op,
            state,
        ),
        "case_query" => {
            validate_case_query_receipt(expr, name, context, values, receipts_by_op, state)
        }
        "workspace_query" => validate_workspace_query_receipt(
            op_object,
            expr,
            name,
            context,
            values,
            receipts_by_op,
            state,
        ),
        other => Err(invalid_plan(format!(
            "representative Plan IR receipt verifier does not inspect `{other}` let expressions"
        ))),
    }
}

fn validate_graph_query_receipt(
    expr: &Value,
    name: &str,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    values: &Map<String, Value>,
    receipts_by_op: &BTreeMap<String, &Map<String, Value>>,
    state: &mut ReceiptValidationState,
) -> Result<(), PublicSeamError> {
    let Some(receipt) = receipts_by_op.get(name) else {
        return Err(invalid_plan(format!(
            "graph_query operation `{name}` must carry a query receipt"
        )));
    };
    require_receipt_field(receipt, "kind", "query")?;
    let value = values.get(name).ok_or_else(|| {
        invalid_plan(format!(
            "query receipt for `{name}` must have a matching result value"
        ))
    })?;
    let scope = graph_read_scope(plan_document, context)?;
    let scope_value = graph_read_scope_value(scope);
    let projection = object(expr, "graph_query")?
        .get("projection")
        .ok_or_else(|| invalid_plan("graph_query must carry projection"))?;
    require_receipt_field(
        receipt,
        "op_hash",
        &prefixed_jcs_hash(
            "fp_query_sha256_",
            &json!({
                "schema_version": "leaven.plan_query_op.v1",
                "name": name,
                "expr": expr,
                "scope": scope_value
            }),
        )?,
    )?;
    require_receipt_field(
        receipt,
        "read_scope_fingerprint",
        &prefixed_jcs_hash("fp_scope_sha256_", &scope_value)?,
    )?;
    require_receipt_field(
        receipt,
        "projection_fingerprint",
        &prefixed_jcs_hash("fp_projection_sha256_", projection)?,
    )?;
    state.bindings.insert(name.to_owned(), value.clone());
    Ok(())
}

fn validate_case_query_receipt(
    expr: &Value,
    name: &str,
    context: &PlanExecutionContext,
    values: &Map<String, Value>,
    receipts_by_op: &BTreeMap<String, &Map<String, Value>>,
    state: &mut ReceiptValidationState,
) -> Result<(), PublicSeamError> {
    let Some(receipt) = receipts_by_op.get(name) else {
        return Err(invalid_plan(format!(
            "case_query operation `{name}` must carry a query receipt"
        )));
    };
    require_receipt_field(receipt, "kind", "query")?;
    let value = values.get(name).ok_or_else(|| {
        invalid_plan(format!(
            "case_query receipt for `{name}` must have a matching result value"
        ))
    })?;
    let query = object(expr, "case_query")?
        .get("query")
        .ok_or_else(|| invalid_plan("case_query must carry query"))?;
    if nested_kind(query, "case_query.query")? != "load" {
        return Err(invalid_plan(
            "representative Plan IR receipt verifier only inspects case_query.load",
        ));
    }
    let scope = json!({
        "kind": "case_query.load",
        "base_revision": context.base_revision
    });
    require_receipt_field(
        receipt,
        "op_hash",
        &prefixed_jcs_hash(
            "fp_query_sha256_",
            &json!({
                "schema_version": "leaven.plan_query_op.v1",
                "name": name,
                "expr": expr,
                "scope": scope
            }),
        )?,
    )?;
    require_receipt_field(
        receipt,
        "read_scope_fingerprint",
        &prefixed_jcs_hash("fp_scope_sha256_", &scope)?,
    )?;
    require_receipt_field(
        receipt,
        "projection_fingerprint",
        &prefixed_jcs_hash("fp_projection_sha256_", &case_query_projection(query)?)?,
    )?;
    require_receipt_field(
        receipt,
        "result_hash",
        &prefixed_jcs_hash(
            "fp_result_sha256_",
            &json!({
                "schema_version": "leaven.plan_query_result.v1",
                "name": name,
                "value": value
            }),
        )?,
    )?;
    state.bindings.insert(name.to_owned(), value.clone());
    Ok(())
}

fn validate_workspace_query_receipt(
    op_object: &Map<String, Value>,
    expr: &Value,
    name: &str,
    context: &PlanExecutionContext,
    values: &Map<String, Value>,
    receipts_by_op: &BTreeMap<String, &Map<String, Value>>,
    state: &mut ReceiptValidationState,
) -> Result<(), PublicSeamError> {
    let Some(receipt) = receipts_by_op.get(name) else {
        return Err(invalid_plan(format!(
            "workspace_query operation `{name}` must carry a query receipt"
        )));
    };
    require_receipt_field(receipt, "kind", "query")?;
    let value = values.get(name).ok_or_else(|| {
        invalid_plan(format!(
            "workspace_query receipt for `{name}` must have a matching result value"
        ))
    })?;
    let deps = dependency_values(op_object, &state.bindings)?;
    let request = workspace_query_request_from_values(name, expr, &deps)?;
    require_live_workspace(
        request.workspace(),
        &deps,
        &state.live_workspaces,
        "workspace_query",
    )?;
    let expected_kind = workspace_query_expected_value_kind(request)?;
    let value_kind = value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan("workspace_query result value must carry kind"))?;
    if value_kind != expected_kind {
        return Err(invalid_plan(format!(
            "workspace_query `{}` result value kind `{value_kind}` does not match `{expected_kind}`",
            request.op_kind()?
        )));
    }
    let value_object = value
        .as_object()
        .ok_or_else(|| invalid_plan("workspace_query result value must be an object"))?;
    validate_workspace_query_value_shape(request, value_object)?;
    validate_workspace_file_data_classes(request, expected_kind, value)?;
    let scope = json!({
        "kind": "workspace_query",
        "workspace": workspace_ref_id(
            object(expr, "workspace_query")?.get("workspace"),
            "workspace_query must carry workspace"
        )?,
        "base_revision": context.base_revision
    });
    require_receipt_field(
        receipt,
        "op_hash",
        &prefixed_jcs_hash(
            "fp_query_sha256_",
            &json!({
                "schema_version": "leaven.plan_query_op.v1",
                "name": name,
                "expr": expr,
                "scope": scope
            }),
        )?,
    )?;
    require_receipt_field(
        receipt,
        "read_scope_fingerprint",
        &prefixed_jcs_hash("fp_scope_sha256_", &scope)?,
    )?;
    require_receipt_field(
        receipt,
        "projection_fingerprint",
        &prefixed_jcs_hash(
            "fp_projection_sha256_",
            &workspace_query_projection(request),
        )?,
    )?;
    require_receipt_field(
        receipt,
        "result_hash",
        &prefixed_jcs_hash(
            "fp_result_sha256_",
            &json!({
                "schema_version": "leaven.plan_query_result.v1",
                "name": name,
                "value": value
            }),
        )?,
    )?;
    state.bindings.insert(name.to_owned(), value.clone());
    Ok(())
}

fn validate_workspace_file_data_classes(
    request: PlanWorkspaceQueryRequest<'_>,
    expected_kind: &str,
    value: &Value,
) -> Result<(), PublicSeamError> {
    if expected_kind != "workspace_file" {
        return Ok(());
    }
    let data_classes = value
        .get("data_classes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("workspace_query file result must carry data_classes"))?
        .iter()
        .map(|value| {
            value.as_str().ok_or_else(|| {
                invalid_plan("workspace_query file result data_classes must be strings")
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    for expected in request.expected_data_classes()? {
        if !data_classes.contains(expected) {
            return Err(invalid_plan(format!(
                "workspace_query read_file result missing expected data class `{expected}`"
            )));
        }
    }
    Ok(())
}

fn validate_call_receipt(
    op_object: &Map<String, Value>,
    name: &str,
    values: &Map<String, Value>,
    receipts_by_op: &BTreeMap<String, &Map<String, Value>>,
    state: &mut ReceiptValidationState,
) -> Result<(), PublicSeamError> {
    let Some(receipt) = receipts_by_op.get(name) else {
        return Err(invalid_plan(format!(
            "call operation `{name}` must carry a call receipt"
        )));
    };
    let call = op_object
        .get("call")
        .ok_or_else(|| invalid_plan("call op must carry call"))?;
    let call_kind = nested_kind(call, "call")?;
    require_receipt_field(receipt, "kind", "call")?;
    require_receipt_field(receipt, "call_kind", call_kind)?;
    let deps = dependency_values(op_object, &state.bindings)?;
    validate_call_workspace_provenance(call_kind, call, &deps, &state.live_workspaces)?;
    require_receipt_field(
        receipt,
        "request_hash",
        &prefixed_jcs_hash(
            "fp_request_sha256_",
            &json!({
                "schema_version": "leaven.plan_call_request.v1",
                "name": name,
                "kind": call_kind,
                "call": call,
                "deps": deps
            }),
        )?,
    )?;
    match required_string(receipt.get("status"), "receipt.status")? {
        "succeeded" => {
            let value = values.get(name).ok_or_else(|| {
                invalid_plan(format!(
                    "succeeded call receipt for `{name}` must have a matching result value"
                ))
            })?;
            update_call_workspace_provenance(name, call_kind, call, value, &deps, state)?;
            state.bindings.insert(name.to_owned(), value.clone());
        }
        "failed" => {
            require_receipt_field(
                receipt,
                "result_hash",
                &prefixed_jcs_hash(
                    "fp_result_sha256_",
                    &json!({
                        "schema_version": "leaven.plan_call_result.v1",
                        "name": name,
                        "error": receipt.get("error"),
                        "cost": receipt.get("cost"),
                        "charge_receipts": receipt.get("charge_receipts").cloned().unwrap_or_else(|| json!([]))
                    }),
                )?,
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn validate_call_workspace_provenance(
    call_kind: &str,
    call: &Value,
    deps: &BTreeMap<String, Value>,
    live_workspaces: &BTreeMap<String, LiveWorkspaceHandle>,
) -> Result<(), PublicSeamError> {
    match call_kind {
        "agent_run" => {
            let workspace =
                workspace_ref_id(call.get("workspace"), "agent_run must carry workspace")?;
            require_live_workspace(workspace, deps, live_workspaces, "agent_run")?;
        }
        "sandbox_exec" => {
            let workspace =
                workspace_ref_id(call.get("workspace"), "sandbox_exec must carry workspace")?;
            require_live_workspace(workspace, deps, live_workspaces, "sandbox_exec")?;
        }
        "workspace_release" => {
            let workspace = workspace_ref_id(
                call.get("workspace"),
                "workspace_release must carry workspace",
            )?;
            require_live_workspace(workspace, deps, live_workspaces, "workspace_release")?;
        }
        _ => {}
    }
    Ok(())
}

fn update_call_workspace_provenance(
    name: &str,
    call_kind: &str,
    call: &Value,
    value: &Value,
    deps: &BTreeMap<String, Value>,
    state: &mut ReceiptValidationState,
) -> Result<(), PublicSeamError> {
    match call_kind {
        "workspace_materialize" => {
            if value.get("kind").and_then(Value::as_str) == Some("workspace_handle") {
                let workspace =
                    workspace_ref_id(value.get("workspace"), "workspace_materialize result")?;
                let lifetime = required_string(value.get("lifetime"), "workspace lifetime")?;
                if value
                    .get("released")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    return Err(invalid_plan(
                        "workspace_materialize result must bind an unreleased handle",
                    ));
                }
                state.live_workspaces.insert(
                    name.to_owned(),
                    LiveWorkspaceHandle::live(workspace, lifetime),
                );
            }
        }
        "workspace_release" => {
            let workspace = workspace_ref_id(
                call.get("workspace"),
                "workspace_release must carry workspace",
            )?;
            let live = require_live_workspace(
                workspace,
                deps,
                &state.live_workspaces,
                "workspace_release",
            )?;
            let lifetime = live.lifetime().to_owned();
            let result_workspace =
                workspace_ref_id(value.get("workspace"), "workspace_release result")?;
            if result_workspace != workspace {
                return Err(invalid_plan(format!(
                    "workspace_release result workspace `{result_workspace}` does not match requested workspace `{workspace}`"
                )));
            }
            let result_lifetime = required_string(value.get("lifetime"), "workspace lifetime")?;
            if result_lifetime != lifetime {
                return Err(invalid_plan(format!(
                    "workspace_release result lifetime `{result_lifetime}` does not match live workspace lifetime `{lifetime}`"
                )));
            }
            for handle in state.live_workspaces.values_mut() {
                if handle.workspace() == workspace {
                    handle.release();
                }
            }
            state.live_workspaces.insert(
                name.to_owned(),
                LiveWorkspaceHandle::released(workspace, lifetime),
            );
        }
        _ => {}
    }
    Ok(())
}

fn validate_write_receipt(
    op_object: &Map<String, Value>,
    name: &str,
    context: &PlanExecutionContext,
    receipts_by_op: &BTreeMap<String, &Map<String, Value>>,
    state: &mut ReceiptValidationState,
) -> Result<(), PublicSeamError> {
    let Some(receipt) = receipts_by_op.get(name) else {
        return Err(invalid_plan(format!(
            "write operation `{name}` must carry a write receipt"
        )));
    };
    let write = op_object
        .get("write")
        .ok_or_else(|| invalid_plan("write op must carry write"))?;
    let write_kind = nested_kind(write, "write")?;
    require_receipt_field(receipt, "kind", "write")?;
    require_receipt_field(receipt, "write_kind", write_kind)?;
    require_receipt_field(receipt, "base_revision", &context.base_revision)?;
    let deps = dependency_values(op_object, &state.bindings)?;
    require_receipt_field(
        receipt,
        "request_hash",
        &prefixed_jcs_hash(
            "fp_request_sha256_",
            &json!({
                "schema_version": "leaven.plan_write_request.v1",
                "name": name,
                "kind": write_kind,
                "write": write,
                "deps": deps,
                "base_revision": context.base_revision
            }),
        )?,
    )?;
    if write_kind == "emit_run_event" {
        require_receipt_field(
            receipt,
            "result_hash",
            &prefixed_jcs_hash(
                "fp_result_sha256_",
                &json!({
                    "schema_version": "leaven.plan_write_result.v1",
                    "name": name,
                    "event_id": required_string(receipt.get("event_id"), "receipt.event_id")?,
                    "committed_revision": receipt.get("committed_revision").cloned().unwrap_or(Value::Null)
                }),
            )?,
        )?;
        state.bindings.insert(
            name.to_owned(),
            json!({
                "kind": "emit_run_event",
                "event_id": required_string(receipt.get("event_id"), "receipt.event_id")?
            }),
        );
    }
    Ok(())
}

fn receipts_by_op_var(
    receipts: &[Value],
) -> Result<BTreeMap<String, &Map<String, Value>>, PublicSeamError> {
    let mut by_op = BTreeMap::new();
    for receipt in receipts {
        let receipt = object(receipt, "receipt")?;
        let op_var = required_string(receipt.get("op_var"), "receipt.op_var")?;
        if by_op.insert(op_var.to_owned(), receipt).is_some() {
            return Err(invalid_plan(format!(
                "multiple receipts claim operation `{op_var}`"
            )));
        }
    }
    Ok(by_op)
}

fn require_receipt_field(
    receipt: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), PublicSeamError> {
    let actual = required_string(receipt.get(field), field)?;
    if actual != expected {
        return Err(invalid_plan(format!(
            "receipt {field} for `{}` does not match Plan IR preimage",
            receipt
                .get("receipt")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>")
        )));
    }
    Ok(())
}
