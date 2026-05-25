use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::{
    PlanExecutionContext, dependency_values,
    effects::{LiveWorkspaceHandle, require_live_workspace_ref, workspace_ref_facts},
    invalid_plan, nested_kind, object, prefixed_jcs_hash, required_string,
    validate_json_schema_output_payload,
};
use crate::{PublicSeamError, plan_error};

mod effects;
mod helpers;
mod queries;

use effects::{update_call_workspace_provenance, validate_write_receipt};
pub use effects::{validate_agent_session_value, validate_sandbox_exec_value};
use helpers::{
    ReceiptValidationState, expected_call_result_value_kind, receipts_by_op_var,
    require_receipt_field, validate_call_workspace_provenance,
};
use queries::{
    validate_case_query_receipt, validate_graph_query_receipt, validate_workspace_query_receipt,
};

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
    let charges = result_object
        .get("charges")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("plan result charges must be an array"))?;
    let errors = result_object
        .get("errors")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("plan result errors must be an array"))?;
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
    let mut state = ReceiptValidationState::new(charges, errors)?;
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
            let expr_object = object(expr, "literal expr")?;
            let value = expr_object
                .get("value")
                .cloned()
                .ok_or_else(|| invalid_plan("literal expr must carry value"))?;
            let data_classes = expr_data_classes(expr_object)?;
            if !data_classes.is_empty() {
                state
                    .binding_data_classes
                    .insert(name.to_owned(), data_classes);
            }
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

fn expr_data_classes(object: &Map<String, Value>) -> Result<BTreeSet<String>, PublicSeamError> {
    let Some(data_classes) = object.get("data_classes") else {
        return Ok(BTreeSet::new());
    };
    data_classes
        .as_array()
        .ok_or_else(|| invalid_plan("expr data_classes must be an array"))?
        .iter()
        .map(|data_class| {
            data_class
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid_plan("expr data_classes entries must be strings"))
        })
        .collect()
}

fn dependency_data_classes(
    op: &Map<String, Value>,
    binding_data_classes: &BTreeMap<String, BTreeSet<String>>,
) -> Result<BTreeSet<String>, PublicSeamError> {
    let mut data_classes = BTreeSet::new();
    let Some(raw) = op.get("deps") else {
        return Ok(data_classes);
    };
    let raw = raw
        .as_array()
        .ok_or_else(|| invalid_plan("op deps must be an array"))?;
    for dep in raw {
        let dep = dep
            .as_str()
            .ok_or_else(|| invalid_plan("op deps must be binding names"))?;
        if let Some(dep_data_classes) = binding_data_classes.get(dep) {
            data_classes.extend(dep_data_classes.iter().cloned());
        }
    }
    Ok(data_classes)
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
    let dependency_data_classes = dependency_data_classes(op_object, &state.binding_data_classes)?;
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
                "deps": deps,
                "dependency_data_classes": dependency_data_classes
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
            validate_successful_call_result_value(
                name,
                call_kind,
                call,
                value,
                receipt,
                required_string(receipt.get("receipt"), "receipt.receipt")?,
            )?;
            update_call_workspace_provenance(name, call_kind, call, value, &deps, state)?;
            state.bindings.insert(name.to_owned(), value.clone());
        }
        "failed" => {
            validate_failed_call_receipt(name, receipt, state)?;
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

fn validate_successful_call_result_value(
    name: &str,
    call_kind: &str,
    call: &Value,
    value: &Value,
    receipt: &Map<String, Value>,
    receipt_id: &str,
) -> Result<(), PublicSeamError> {
    let value = object(value, "call result value")?;
    let value_kind = required_string(value.get("kind"), "call result value.kind")?;
    let expected_kind = expected_call_result_value_kind(call_kind).ok_or_else(|| {
        invalid_plan(format!(
            "representative Plan IR receipt verifier does not inspect `{call_kind}` call results"
        ))
    })?;
    if value_kind != expected_kind {
        return Err(invalid_plan(format!(
            "call operation `{name}` result value kind `{value_kind}` does not match `{expected_kind}` for `{call_kind}`"
        )));
    }
    require_receipt_field(value, "receipt", receipt_id)?;
    validate_external_call_cost_binding(call_kind, value, receipt)?;
    validate_lm_response_value(call_kind, call, value, receipt)?;
    validate_structured_output_value(call_kind, call, value)?;
    validate_sandbox_stream_value(call, value)?;
    validate_agent_session_value(call_kind, Some(call), value, receipt_id)?;
    validate_sandbox_exec_value(call_kind, value)?;
    Ok(())
}

fn validate_failed_call_receipt(
    name: &str,
    receipt: &Map<String, Value>,
    state: &ReceiptValidationState,
) -> Result<(), PublicSeamError> {
    let receipt_id = required_string(receipt.get("receipt"), "receipt.receipt")?;
    let error = receipt
        .get("error")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            invalid_plan(format!(
                "failed call receipt for `{name}` must carry typed PlanError"
            ))
        })?;
    plan_error::validate_closed_plan_error(error).map_err(invalid_plan)?;
    let error_receipt = plan_error::plan_error_receipt_id(error).map_err(invalid_plan)?;
    if error_receipt != receipt_id {
        return Err(invalid_plan(
            "failed call PlanError receipt must match call receipt",
        ));
    }
    if !state
        .errors
        .iter()
        .any(|value| value.as_object() == Some(error))
    {
        return Err(invalid_plan(format!(
            "failed call receipt for `{name}` PlanError must appear in top-level errors"
        )));
    }

    let charge_receipts = receipt
        .get("charge_receipts")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if receipt.get("cost").is_some() {
        if charge_receipts.is_empty() {
            return Err(invalid_plan(format!(
                "failed paid call receipt for `{name}` must carry charge_receipts"
            )));
        }
        for charge_ref in charge_receipts {
            let charge_id = charge_ref.as_str().ok_or_else(|| {
                invalid_plan("failed call charge_receipts entries must be receipt ids")
            })?;
            let charge = state.charges_by_receipt.get(charge_id).ok_or_else(|| {
                invalid_plan(format!(
                    "failed call charge receipt `{charge_id}` is not present in top-level charges"
                ))
            })?;
            let charge = object(charge, "charge receipt")?;
            require_receipt_field(charge, "source_receipt", receipt_id)?;
        }
    } else if !charge_receipts.is_empty() {
        return Err(invalid_plan(format!(
            "failed uncharged call receipt for `{name}` must not carry charge_receipts"
        )));
    }
    Ok(())
}

fn validate_external_call_cost_binding(
    call_kind: &str,
    value: &Map<String, Value>,
    receipt: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    if !matches!(call_kind, "agent_run" | "sandbox_exec") {
        return Ok(());
    }
    match (value.get("cost"), receipt.get("cost")) {
        (Some(value_cost), Some(receipt_cost)) if value_cost == receipt_cost => Ok(()),
        (Some(_), _) => Err(invalid_plan(format!(
            "{call_kind} result value cost must match call receipt cost"
        ))),
        (None, Some(_)) => Err(invalid_plan(format!(
            "{call_kind} call receipt cost must have a matching result value cost"
        ))),
        (None, None) => Ok(()),
    }
}

fn validate_lm_response_value(
    call_kind: &str,
    call: &Value,
    value: &Map<String, Value>,
    receipt: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    if call_kind != "lm_complete" {
        return Ok(());
    }
    let cost = value
        .get("cost")
        .ok_or_else(|| invalid_plan("lm_complete result value must carry cost"))?;
    let receipt_cost = receipt
        .get("cost")
        .ok_or_else(|| invalid_plan("lm_complete call receipt must carry cost"))?;
    if receipt_cost != cost {
        return Err(invalid_plan(
            "lm_complete call receipt cost must match result value cost",
        ));
    }
    let message = value
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_plan("lm_complete result value must carry message"))?;
    let role = required_string(message.get("role"), "lm_complete result message role")?;
    if role != "assistant" {
        return Err(invalid_plan(format!(
            "lm_complete result message role `{role}` must be assistant"
        )));
    }
    if message.get("tool_call_id").is_some() || message.get("name").is_some() {
        return Err(invalid_plan(
            "lm_complete result message must not carry tool_call_id or name",
        ));
    }
    let content = message
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("lm_complete result message must carry content"))?;
    let mut text_bytes = 0usize;
    for part in content {
        let part = object(part, "lm_complete result content part")?;
        match required_string(part.get("kind"), "lm_complete result content kind")? {
            "text" => {
                text_bytes += required_string(part.get("text"), "lm_complete result text")?.len();
            }
            other => {
                return Err(invalid_plan(format!(
                    "lm_complete result content kind `{other}` is not a V1 final response"
                )));
            }
        }
    }
    if call
        .get("output")
        .and_then(Value::as_object)
        .and_then(|output| output.get("kind"))
        .and_then(Value::as_str)
        == Some("final_message")
        && let Some(max_bytes) = call
            .get("output")
            .and_then(Value::as_object)
            .and_then(|output| output.get("max_bytes"))
            .and_then(Value::as_u64)
    {
        let max_bytes = usize::try_from(max_bytes).unwrap_or(usize::MAX);
        if text_bytes > max_bytes {
            return Err(invalid_plan(format!(
                "lm_complete final_message result exceeds max_bytes {max_bytes}"
            )));
        }
    }
    Ok(())
}

fn validate_structured_output_value(
    call_kind: &str,
    call: &Value,
    value: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    if matches!(call_kind, "lm_complete" | "agent_run")
        && call
            .get("output")
            .and_then(Value::as_object)
            .and_then(|output| output.get("kind"))
            .and_then(Value::as_str)
            == Some("json_schema")
    {
        let parsed = value.get("parsed").ok_or_else(|| {
            invalid_plan(format!(
                "{call_kind} json_schema result value must carry parsed payload"
            ))
        })?;
        validate_json_schema_output_payload(call_kind, call, parsed)?;
    }
    Ok(())
}

fn validate_sandbox_stream_value(
    call: &Value,
    value: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    if call
        .get("stream_policy")
        .and_then(Value::as_str)
        .unwrap_or("buffer")
        == "blob_refs_only"
        && (!value.contains_key("stdout_ref") || !value.contains_key("stderr_ref"))
    {
        return Err(invalid_plan(
            "sandbox_exec blob_refs_only result value must carry stdout_ref and stderr_ref",
        ));
    }
    Ok(())
}
