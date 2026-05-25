use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::{ReceiptValidationState, require_live_workspace_ref, require_receipt_field};
use crate::PublicSeamError;
use crate::plan_execution::{
    PlanExecutionContext, PlanWorkspaceQueryRequest, case_query_projection, dependency_values,
    graph_read_scope, graph_read_scope_value, invalid_plan, nested_kind, object, prefixed_jcs_hash,
    validate_workspace_query_value_shape, workspace_query_expected_value_kind,
    workspace_query_projection, workspace_query_request_from_values,
};

pub(super) fn validate_graph_query_receipt(
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

pub(super) fn validate_case_query_receipt(
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

pub(super) fn validate_workspace_query_receipt(
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
    require_live_workspace_ref(
        request.workspace_ref(),
        &deps,
        &state.live_workspaces,
        "workspace_query",
    )?;
    let expected_kind = workspace_query_expected_value_kind(&request)?;
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
    validate_workspace_query_value_shape(&request, value_object)?;
    validate_workspace_file_data_classes(&request, expected_kind, value)?;
    let scope = json!({
        "kind": "workspace_query",
        "workspace": object(expr, "workspace_query")?
            .get("workspace")
            .cloned()
            .ok_or_else(|| invalid_plan("workspace_query must carry workspace"))?,
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
            &workspace_query_projection(&request),
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
    request: &PlanWorkspaceQueryRequest<'_>,
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
