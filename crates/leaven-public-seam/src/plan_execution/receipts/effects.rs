use std::collections::{BTreeMap, BTreeSet};

use leaven_workspace::WorkspacePath;
use serde_json::{Map, Value, json};

use super::{ReceiptValidationState, dependency_data_classes, require_receipt_field};
use crate::PublicSeamError;
use crate::plan_execution::{
    PlanExecutionContext, dependency_values,
    effects::{LiveWorkspaceHandle, require_live_workspace_ref, workspace_ref_facts},
    invalid_plan, nested_kind, object, prefixed_jcs_hash, required_string,
};

pub fn validate_agent_session_value(
    call_kind: &str,
    call: Option<&Value>,
    value: &Map<String, Value>,
    receipt_id: &str,
) -> Result<(), PublicSeamError> {
    if call_kind != "agent_run" {
        return Ok(());
    }
    if value.get("transcript_ref").is_none() {
        return Err(invalid_plan(
            "agent_run result value must carry transcript_ref",
        ));
    }
    let commands = value
        .get("commands")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("agent_run result value must carry commands"))?;
    if commands.is_empty() {
        return Err(invalid_plan(
            "agent_run result value must carry at least one command record",
        ));
    }
    for command in commands {
        let command = object(command, "agent_run command record")?;
        validate_agent_command_record(command)?;
        validate_agent_command_policy(call, command)?;
        let command_receipt = required_string(command.get("receipt"), "agent_run command receipt")?;
        if command_receipt != receipt_id {
            return Err(invalid_plan(format!(
                "agent_run command record receipt `{command_receipt}` does not match session receipt `{receipt_id}`"
            )));
        }
    }
    if value.get("cost").is_none() {
        return Err(invalid_plan("agent_run result value must carry cost"));
    }
    Ok(())
}

fn string_array<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<Vec<&'a str>, PublicSeamError> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan(format!("{field} must be an array")))?
        .iter()
        .map(|item| {
            item.as_str()
                .ok_or_else(|| invalid_plan(format!("{field} entries must be strings")))
        })
        .collect()
}

fn validate_agent_command_policy(
    call: Option<&Value>,
    command: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let allowed_commands = call
        .and_then(|call| call.get("tool_policy"))
        .and_then(Value::as_object)
        .and_then(|policy| policy.get("allowed_commands"))
        .and_then(Value::as_array)
        .map(|commands| {
            commands
                .iter()
                .map(|command| {
                    command.as_str().map(str::to_owned).ok_or_else(|| {
                        invalid_plan("agent_run allowed_commands entries must be strings")
                    })
                })
                .collect::<Result<BTreeSet<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if allowed_commands.is_empty() {
        return Ok(());
    }
    let argv = command
        .get("argv")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("agent_run command record must carry argv"))?;
    let program = argv
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan("agent_run command record argv must not be empty"))?;
    if allowed_commands.contains(program) {
        Ok(())
    } else {
        Err(invalid_plan(format!(
            "agent_run command `{program}` is outside declared allowed_commands"
        )))
    }
}

fn validate_agent_command_record(command: &Map<String, Value>) -> Result<(), PublicSeamError> {
    let argv = command
        .get("argv")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("agent_run command record must carry argv"))?;
    if argv.is_empty() {
        return Err(invalid_plan(
            "agent_run command record argv must not be empty",
        ));
    }
    for arg in argv {
        required_string(Some(arg), "agent_run command argv")?;
    }
    let status = required_string(command.get("status"), "agent_run command status")?;
    if !matches!(status, "completed" | "failed" | "cancelled" | "timeout") {
        return Err(invalid_plan(format!(
            "agent_run command status `{status}` is not a V1 command status"
        )));
    }
    validate_agent_command_blob_ref(command.get("stdout_ref"), "stdout_ref")?;
    validate_agent_command_blob_ref(command.get("stderr_ref"), "stderr_ref")?;
    if let Some(files) = command.get("files") {
        let files = files
            .as_object()
            .ok_or_else(|| invalid_plan("agent_run command files must be an object"))?;
        for (path, blob_ref) in files {
            WorkspacePath::new(path).map_err(|error| {
                invalid_plan(format!(
                    "agent_run command file path must be relative workspace path: {error}"
                ))
            })?;
            validate_agent_command_blob_ref(Some(blob_ref), "files")?;
        }
    }
    Ok(())
}

fn validate_agent_command_blob_ref(
    blob_ref: Option<&Value>,
    field: &str,
) -> Result<(), PublicSeamError> {
    let object = blob_ref
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_plan(format!("agent_run command record must carry {field}")))?;
    if object.get("kind").and_then(Value::as_str) != Some("blob_ref") {
        return Err(invalid_plan(format!(
            "agent_run command {field} must be a blob_ref"
        )));
    }
    required_string(object.get("id"), "agent_run command blob_ref id")?;
    let sha = required_string(object.get("sha256"), "agent_run command blob_ref sha256")?;
    if sha.len() != 64 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_plan(
            "agent_run command blob_ref sha256 must be 64 hex characters",
        ));
    }
    object
        .get("bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_plan("agent_run command blob_ref must carry bytes"))?;
    let data_classes = object
        .get("data_classes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("agent_run command blob_ref must carry data_classes"))?;
    if data_classes.is_empty() {
        return Err(invalid_plan(
            "agent_run command blob_ref data_classes must not be empty",
        ));
    }
    for data_class in data_classes {
        required_string(Some(data_class), "agent_run command blob_ref data_classes")?;
    }
    Ok(())
}

pub fn validate_sandbox_exec_value(
    call_kind: &str,
    value: &Map<String, Value>,
) -> Result<(), PublicSeamError> {
    if call_kind != "sandbox_exec" {
        return Ok(());
    }
    if value.get("cost").is_none() {
        return Err(invalid_plan("sandbox_exec result value must carry cost"));
    }
    let status = required_string(value.get("status"), "sandbox_exec status")?;
    if status == "completed" && value.get("exit_code").and_then(Value::as_i64).is_none() {
        return Err(invalid_plan(
            "completed sandbox_exec result value must carry exit_code",
        ));
    }
    if status == "completed"
        && (!value.contains_key("stdout_ref") || !value.contains_key("stderr_ref"))
    {
        return Err(invalid_plan(
            "completed sandbox_exec result value must carry stdout_ref and stderr_ref",
        ));
    }
    if let Some(files) = value.get("files") {
        let files = files
            .as_object()
            .ok_or_else(|| invalid_plan("sandbox_exec result files must be an object"))?;
        for path in files.keys() {
            WorkspacePath::new(path).map_err(|error| {
                invalid_plan(format!(
                    "sandbox_exec result file path must be relative workspace path: {error}"
                ))
            })?;
        }
    }
    Ok(())
}

pub(super) fn update_call_workspace_provenance(
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
                    workspace_ref_facts(value.get("workspace"), "workspace_materialize result")?;
                let lifetime = required_string(value.get("lifetime"), "workspace lifetime")?;
                let requested_lifetime =
                    required_string(call.get("lifetime"), "workspace_materialize lifetime")?;
                if lifetime != requested_lifetime {
                    return Err(invalid_plan(format!(
                        "workspace_materialize result lifetime `{lifetime}` does not match requested lifetime `{requested_lifetime}`"
                    )));
                }
                let released = value
                    .get("released")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| {
                        invalid_plan("workspace_materialize result must carry released")
                    })?;
                if released {
                    return Err(invalid_plan(
                        "workspace_materialize result must bind an unreleased handle",
                    ));
                }
                state.live_workspaces.insert(
                    name.to_owned(),
                    LiveWorkspaceHandle::live_ref(workspace, lifetime),
                );
            }
        }
        "workspace_release" => {
            let workspace = workspace_ref_facts(
                call.get("workspace"),
                "workspace_release must carry workspace",
            )?;
            let live = require_live_workspace_ref(
                &workspace,
                deps,
                &state.live_workspaces,
                "workspace_release",
            )?;
            let lifetime = live.lifetime().to_owned();
            let result_workspace =
                workspace_ref_facts(value.get("workspace"), "workspace_release result")?;
            if !result_workspace.satisfies_request(&workspace) {
                return Err(invalid_plan(format!(
                    "workspace_release result workspace `{}` does not match requested workspace `{}`",
                    result_workspace.id(),
                    workspace.id()
                )));
            }
            let result_lifetime = required_string(value.get("lifetime"), "workspace lifetime")?;
            if result_lifetime != lifetime {
                return Err(invalid_plan(format!(
                    "workspace_release result lifetime `{result_lifetime}` does not match live workspace lifetime `{lifetime}`"
                )));
            }
            let released = value
                .get("released")
                .and_then(Value::as_bool)
                .ok_or_else(|| invalid_plan("workspace_release result must carry released"))?;
            if !released {
                return Err(invalid_plan(
                    "workspace_release result must bind a released handle",
                ));
            }
            for handle in state.live_workspaces.values_mut() {
                if handle.satisfies_workspace(&workspace) {
                    handle.release();
                }
            }
            state.live_workspaces.insert(
                name.to_owned(),
                LiveWorkspaceHandle::released_ref(result_workspace, lifetime),
            );
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn validate_write_receipt(
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
    let dependency_data_classes = dependency_data_classes(op_object, &state.binding_data_classes)?;
    let request_hash = if write_kind == "submit_assessments" {
        prefixed_jcs_hash(
            "fp_request_sha256_",
            &json!({
                "schema_version": "leaven.submit_assessments_request.v1",
                "evaluation_request_id": required_string(
                    receipt.get("evaluation_request_id"),
                    "receipt.evaluation_request_id"
                )?,
                "assessment_ids": string_array(receipt.get("assessment_ids"), "receipt.assessment_ids")?
            }),
        )?
    } else {
        prefixed_jcs_hash(
            "fp_request_sha256_",
            &json!({
                "schema_version": "leaven.plan_write_request.v1",
                "name": name,
                "kind": write_kind,
                "write": write,
                "deps": deps,
                "dependency_data_classes": dependency_data_classes,
                "base_revision": context.base_revision
            }),
        )?
    };
    require_receipt_field(receipt, "request_hash", &request_hash)?;
    if write_kind == "emit_run_event" {
        let value = json!({
            "kind": "emit_run_event",
            "event_id": required_string(receipt.get("event_id"), "receipt.event_id")?,
            "receipt": required_string(receipt.get("receipt"), "receipt.receipt")?,
            "data_classes": ["public"],
            "replayability": "fully_managed"
        });
        require_receipt_field(
            receipt,
            "result_hash",
            &prefixed_jcs_hash(
                "fp_result_sha256_",
                &json!({
                    "schema_version": "leaven.plan_write_result.v1",
                    "name": name,
                    "value": value
                }),
            )?,
        )?;
        state.bindings.insert(name.to_owned(), value);
    }
    Ok(())
}
