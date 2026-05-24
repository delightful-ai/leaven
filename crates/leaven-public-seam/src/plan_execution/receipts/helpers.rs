use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use super::{LiveWorkspaceHandle, require_live_workspace_ref, workspace_ref_facts};
use crate::PublicSeamError;

use crate::plan_execution::{invalid_plan, object, required_string};

pub(super) struct ReceiptValidationState {
    pub(super) bindings: BTreeMap<String, Value>,
    pub(super) binding_data_classes: BTreeMap<String, BTreeSet<String>>,
    pub(super) live_workspaces: BTreeMap<String, LiveWorkspaceHandle>,
    pub(super) charges_by_receipt: BTreeMap<String, Value>,
    pub(super) errors: Vec<Value>,
}

impl ReceiptValidationState {
    pub(super) fn new(charges: &[Value], errors: &[Value]) -> Result<Self, PublicSeamError> {
        let mut charges_by_receipt = BTreeMap::new();
        for charge in charges {
            let charge = object(charge, "charge receipt")?;
            let receipt = required_string(charge.get("receipt"), "charge.receipt")?;
            if charges_by_receipt
                .insert(receipt.to_owned(), Value::Object(charge.clone()))
                .is_some()
            {
                return Err(invalid_plan(format!(
                    "multiple charge receipts use id `{receipt}`"
                )));
            }
        }
        Ok(Self {
            bindings: BTreeMap::new(),
            binding_data_classes: BTreeMap::new(),
            live_workspaces: BTreeMap::new(),
            charges_by_receipt,
            errors: errors.to_vec(),
        })
    }
}

pub(super) fn receipts_by_op_var(
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

pub(super) fn expected_call_result_value_kind(call_kind: &str) -> Option<&'static str> {
    match call_kind {
        "lm_complete" => Some("lm_response"),
        "agent_run" => Some("agent_session"),
        "sandbox_exec" => Some("sandbox_exec"),
        "workspace_materialize" | "workspace_release" => Some("workspace_handle"),
        "human_review" => Some("human_review_result"),
        _ => None,
    }
}

pub(super) fn validate_call_workspace_provenance(
    call_kind: &str,
    call: &Value,
    deps: &BTreeMap<String, Value>,
    live_workspaces: &BTreeMap<String, LiveWorkspaceHandle>,
) -> Result<(), PublicSeamError> {
    match call_kind {
        "agent_run" => {
            let workspace =
                workspace_ref_facts(call.get("workspace"), "agent_run must carry workspace")?;
            require_live_workspace_ref(&workspace, deps, live_workspaces, "agent_run")?;
        }
        "sandbox_exec" => {
            let workspace =
                workspace_ref_facts(call.get("workspace"), "sandbox_exec must carry workspace")?;
            require_live_workspace_ref(&workspace, deps, live_workspaces, "sandbox_exec")?;
        }
        "workspace_release" => {
            let workspace = workspace_ref_facts(
                call.get("workspace"),
                "workspace_release must carry workspace",
            )?;
            require_live_workspace_ref(&workspace, deps, live_workspaces, "workspace_release")?;
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn require_receipt_field(
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
