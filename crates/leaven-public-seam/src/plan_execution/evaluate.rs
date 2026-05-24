use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::{
    ExecutionState, LiveWorkspaceHandle, PlanCaseQueryRequest, PlanExecutionContext,
    PlanExecutionHost, PlanGraphQueryRequest, case_query_include, case_query_projection,
    graph_read_scope, graph_read_scope_value, invalid_plan, nested_kind, object, prefixed_jcs_hash,
    require_included_case_fields, require_requested_case_field, required_string,
    validate_workspace_query_value_shape, workspace_query_expected_value_kind,
    workspace_query_projection, workspace_query_request,
};
use crate::PublicSeamError;

pub(super) struct ResolvedDependencies {
    pub(super) values: BTreeMap<String, Value>,
    pub(super) live_workspaces: BTreeMap<String, LiveWorkspaceHandle>,
}

pub(super) fn dependency_values(
    op: &Map<String, Value>,
    bindings: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, PublicSeamError> {
    let mut deps = BTreeMap::new();
    let Some(raw) = op.get("deps") else {
        return Ok(deps);
    };
    let raw = raw
        .as_array()
        .ok_or_else(|| invalid_plan("op deps must be an array"))?;
    for dep in raw {
        let dep = dep
            .as_str()
            .ok_or_else(|| invalid_plan("op deps must be binding names"))?;
        let value = bindings
            .get(dep)
            .ok_or_else(|| invalid_plan(format!("op references unknown dependency `{dep}`")))?;
        deps.insert(dep.to_owned(), value.clone());
    }
    Ok(deps)
}

pub(super) fn resolved_dependency_values(
    op: &Map<String, Value>,
    state: &ExecutionState,
) -> Result<ResolvedDependencies, PublicSeamError> {
    let mut values = BTreeMap::new();
    let mut live_workspaces = BTreeMap::new();
    let Some(raw) = op.get("deps") else {
        return Ok(ResolvedDependencies {
            values,
            live_workspaces,
        });
    };
    let raw = raw
        .as_array()
        .ok_or_else(|| invalid_plan("op deps must be an array"))?;
    for dep in raw {
        let dep = dep
            .as_str()
            .ok_or_else(|| invalid_plan("op deps must be binding names"))?;
        let value = state
            .bindings
            .get(dep)
            .ok_or_else(|| invalid_plan(format!("op references unknown dependency `{dep}`")))?;
        values.insert(dep.to_owned(), value.clone());
        if let Some(handle) = state.live_workspaces.get(dep) {
            live_workspaces.insert(dep.to_owned(), handle.clone());
        }
    }
    Ok(ResolvedDependencies {
        values,
        live_workspaces,
    })
}

pub(super) struct EvaluatedExpr {
    pub(super) value: Value,
    pub(super) receipt: Option<Value>,
}

pub(super) fn evaluate_expr(
    expr: &Value,
    name: &str,
    deps: &ResolvedDependencies,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    host: &mut impl PlanExecutionHost,
) -> Result<EvaluatedExpr, PublicSeamError> {
    let object = object(expr, "expr")?;
    match required_string(object.get("kind"), "expr.kind")? {
        "literal" => Ok(EvaluatedExpr {
            value: object
                .get("value")
                .cloned()
                .ok_or_else(|| invalid_plan("literal expr must carry value"))?,
            receipt: None,
        }),
        "graph_query" => super::execute_graph_query_expr(expr, name, plan_document, context, host),
        "case_query" => super::execute_case_query_expr(expr, name, context, host),
        "workspace_query" => super::execute_workspace_query_expr(expr, name, deps, context, host),
        other => Err(invalid_plan(format!(
            "representative Plan IR harness does not execute `{other}` let expressions"
        ))),
    }
}

pub(super) fn execute_graph_query_expr(
    expr: &Value,
    name: &str,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    host: &mut impl PlanExecutionHost,
) -> Result<EvaluatedExpr, PublicSeamError> {
    let scope = graph_read_scope(plan_document, context)?;
    let outcome = host.graph_query(PlanGraphQueryRequest { name, expr, scope })?;
    let receipt_id = format!("qrec_{name}");
    let mut value = json!({
        "kind": "graph_set",
        "items": outcome.items,
        "graph_revision": outcome.graph_revision,
        "data_classes": outcome.data_classes,
        "replayability": "pure_read",
        "receipt": receipt_id
    });
    if let Some(next_cursor) = outcome.next_cursor {
        value["next_cursor"] = json!(next_cursor);
    }
    let scope_value = graph_read_scope_value(scope);
    let projection = object(expr, "graph_query")?
        .get("projection")
        .ok_or_else(|| invalid_plan("graph_query must carry projection"))?;
    let op_hash = prefixed_jcs_hash(
        "fp_query_sha256_",
        &json!({
            "schema_version": "leaven.plan_query_op.v1",
            "name": name,
            "expr": expr,
            "scope": scope_value
        }),
    )?;
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_query_result.v1",
            "name": name,
            "value": value
        }),
    )?;
    let read_scope_fingerprint = prefixed_jcs_hash("fp_scope_sha256_", &scope_value)?;
    let projection_fingerprint = prefixed_jcs_hash("fp_projection_sha256_", projection)?;
    let graph_revision = required_string(value.get("graph_revision"), "graph_revision")?.to_owned();
    Ok(EvaluatedExpr {
        value,
        receipt: Some(json!({
            "kind": "query",
            "receipt": receipt_id,
            "op_var": name,
            "started_at": context.started_at,
            "completed_at": context.completed_at,
            "op_hash": op_hash,
            "result_hash": result_hash,
            "graph_revision": graph_revision,
            "read_scope_fingerprint": read_scope_fingerprint,
            "projection_fingerprint": projection_fingerprint,
            "status": "succeeded"
        })),
    })
}

pub(super) fn execute_case_query_expr(
    expr: &Value,
    name: &str,
    context: &PlanExecutionContext,
    host: &mut impl PlanExecutionHost,
) -> Result<EvaluatedExpr, PublicSeamError> {
    let query = object(expr, "case_query")?
        .get("query")
        .ok_or_else(|| invalid_plan("case_query must carry query"))?;
    if nested_kind(query, "case_query.query")? != "load" {
        return Err(invalid_plan(
            "representative Plan IR harness only executes case_query.load",
        ));
    }
    let include = case_query_include(query)?;
    let outcome = host.case_query_load(PlanCaseQueryRequest { name, query })?;
    let receipt_id = format!("qrec_{name}");
    let mut value = json!({
        "kind": "case_record",
        "case": outcome.case,
        "graph_revision": outcome.graph_revision,
        "data_classes": outcome.data_classes,
        "replayability": "pure_read",
        "receipt": receipt_id
    });
    if let Some(input) = outcome.input {
        require_requested_case_field(&include, "input")?;
        value["input"] = input;
    }
    if let Some(target) = outcome.target {
        require_requested_case_field(&include, "target")?;
        value["target"] = target;
    }
    if let Some(metadata) = outcome.metadata {
        require_requested_case_field(&include, "metadata")?;
        value["metadata"] = metadata;
    }
    require_included_case_fields(&value, &include)?;
    let op_hash = prefixed_jcs_hash(
        "fp_query_sha256_",
        &json!({
            "schema_version": "leaven.plan_query_op.v1",
            "name": name,
            "expr": expr,
            "scope": {
                "kind": "case_query.load",
                "base_revision": context.base_revision
            }
        }),
    )?;
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_query_result.v1",
            "name": name,
            "value": value
        }),
    )?;
    let graph_revision = required_string(value.get("graph_revision"), "graph_revision")?.to_owned();
    Ok(EvaluatedExpr {
        value,
        receipt: Some(json!({
            "kind": "query",
            "receipt": receipt_id,
            "op_var": name,
            "started_at": context.started_at,
            "completed_at": context.completed_at,
            "op_hash": op_hash,
            "result_hash": result_hash,
            "graph_revision": graph_revision,
            "read_scope_fingerprint": prefixed_jcs_hash("fp_scope_sha256_", &json!({
                "kind": "case_query.load",
                "base_revision": context.base_revision
            }))?,
            "projection_fingerprint": prefixed_jcs_hash("fp_projection_sha256_", &case_query_projection(query)?)?,
            "status": "succeeded"
        })),
    })
}

pub(super) fn execute_workspace_query_expr(
    expr: &Value,
    name: &str,
    deps: &ResolvedDependencies,
    context: &PlanExecutionContext,
    host: &mut impl PlanExecutionHost,
) -> Result<EvaluatedExpr, PublicSeamError> {
    let request = workspace_query_request(name, expr, &deps.values, &deps.live_workspaces)?;
    let expected_kind = workspace_query_expected_value_kind(&request)?;
    let expected_data_classes = request.expected_data_classes()?;
    let outcome = host.workspace_query(request.clone())?;
    let receipt_id = format!("qrec_{name}");
    let mut value = outcome
        .value
        .as_object()
        .cloned()
        .ok_or_else(|| invalid_plan("workspace_query host outcome value must be an object"))?;
    let value_kind = value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan("workspace_query host outcome must carry kind"))?;
    if value_kind != expected_kind {
        return Err(invalid_plan(format!(
            "workspace_query `{}` host returned `{value_kind}` instead of `{expected_kind}`",
            request.op_kind()?
        )));
    }
    validate_workspace_query_value_shape(&request, &value)?;
    if expected_kind == "workspace_file" {
        let classes = outcome
            .data_classes
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for expected in expected_data_classes {
            if !classes.contains(expected) {
                return Err(invalid_plan(format!(
                    "workspace_query read_file result missing expected data class `{expected}`"
                )));
            }
        }
    }
    value.insert("receipt".to_owned(), json!(receipt_id));
    let graph_revision = outcome.graph_revision;
    value.insert("graph_revision".to_owned(), json!(&graph_revision));
    value.insert("data_classes".to_owned(), json!(outcome.data_classes));
    value.insert("replayability".to_owned(), json!(outcome.replayability));
    let value = Value::Object(value);
    let op_hash = prefixed_jcs_hash(
        "fp_query_sha256_",
        &json!({
            "schema_version": "leaven.plan_query_op.v1",
            "name": name,
            "expr": expr,
            "scope": {
                "kind": "workspace_query",
                "workspace": request.workspace(),
                "base_revision": context.base_revision
            }
        }),
    )?;
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_query_result.v1",
            "name": name,
            "value": value
        }),
    )?;
    Ok(EvaluatedExpr {
        value,
        receipt: Some(json!({
            "kind": "query",
            "receipt": receipt_id,
            "op_var": name,
            "started_at": context.started_at,
            "completed_at": context.completed_at,
            "op_hash": op_hash,
            "result_hash": result_hash,
            "graph_revision": graph_revision,
            "read_scope_fingerprint": prefixed_jcs_hash("fp_scope_sha256_", &json!({
                "kind": "workspace_query",
                "workspace": request.workspace(),
                "base_revision": context.base_revision
            }))?,
            "projection_fingerprint": prefixed_jcs_hash("fp_projection_sha256_", &workspace_query_projection(&request))?,
            "status": "succeeded"
        })),
    })
}
