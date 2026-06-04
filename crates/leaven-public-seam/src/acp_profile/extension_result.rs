use serde_json::{Value, json};

use super::{
    LockedMethod, MethodPrimaryKind, MethodReceiptExpectation, invalid_acp, prefixed_jcs_hash,
    required_array, required_string, string_array,
};
use crate::{
    PublicSeamError,
    plan_execution::{validate_agent_session_value, validate_sandbox_exec_value},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpExtensionResultDocument {
    method: LockedMethod,
    primary_kind: MethodPrimaryKind,
    capability_fingerprint: String,
    receipt_count: usize,
    redaction_count: usize,
    data_classes: Vec<String>,
}

impl AcpExtensionResultDocument {
    pub(crate) fn from_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_acp("ACP extension result must be an object"))?;
        let method_name = required_string(object.get("method"), "method")?;
        if !method_name.starts_with("leaven/") || method_name.contains("mcp") {
            return Err(invalid_acp(
                "ACP extension result method must be Leaven-only",
            ));
        }
        let method = LockedMethod::parse(method_name).ok_or_else(|| {
            invalid_acp(format!(
                "ACP extension result method `{method_name}` is not in the locked profile"
            ))
        })?;
        let receipts = required_array(object.get("receipts"), "receipts")?;
        if receipts.is_empty() {
            return Err(invalid_acp("ACP extension result must carry receipts"));
        }
        let redactions = required_array(object.get("redactions"), "redactions")?;
        let data_classes = string_array(object.get("data_classes"), "data_classes")?;
        if data_classes.is_empty() {
            return Err(invalid_acp("ACP extension result must carry data classes"));
        }
        let primary_value = object
            .get("primary")
            .ok_or_else(|| invalid_acp("ACP extension result must carry primary object"))?;
        let primary = primary_value
            .as_object()
            .ok_or_else(|| invalid_acp("ACP extension result must carry primary object"))?;
        let primary_kind_name = required_string(primary.get("kind"), "primary.kind")?;
        let primary_kind = MethodPrimaryKind::parse(primary_kind_name).ok_or_else(|| {
            invalid_acp(format!(
                "ACP extension result primary kind `{primary_kind_name}` is not in the typed method model"
            ))
        })?;
        validate_primary_kind(method, primary_kind)?;
        if let Some(primary_data_classes) = primary.get("data_classes") {
            for data_class in string_array(Some(primary_data_classes), "primary.data_classes")? {
                if !data_classes.contains(&data_class) {
                    return Err(invalid_acp(format!(
                        "ACP extension result data_classes must cover primary data class `{data_class}`"
                    )));
                }
            }
        }
        validate_receipts_for_method(method, receipts)?;
        validate_primary_result_hash(method, primary_value, receipts)?;
        let expected_receipt = expected_receipt_for_method(method, receipts)?;
        validate_extension_primary_op(method, primary)?;
        validate_effect_primary_audit(method, primary, expected_receipt)?;
        if let Some(primary_receipt) = primary.get("receipt").and_then(Value::as_str) {
            ensure_primary_receipt_is_carried(primary_receipt, receipts)?;
        }
        Ok(Self {
            method,
            primary_kind,
            capability_fingerprint: required_string(
                object.get("capability_fingerprint"),
                "capability_fingerprint",
            )?
            .to_owned(),
            receipt_count: receipts.len(),
            redaction_count: redactions.len(),
            data_classes,
        })
    }

    pub(crate) fn synthetic_plan_result(value: &Value) -> Result<Value, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_acp("ACP extension result must be an object"))?;
        let primary = object
            .get("primary")
            .ok_or_else(|| invalid_acp("ACP extension result must carry primary value"))?;
        let primary_object = primary
            .as_object()
            .ok_or_else(|| invalid_acp("ACP extension primary must be an object"))?;
        let graph_revision = primary_object
            .get("graph_revision")
            .and_then(Value::as_str)
            .unwrap_or("rev_acp_extension_result");
        let replayability = primary_object
            .get("replayability")
            .and_then(Value::as_str)
            .unwrap_or("fully_managed");
        Ok(json!({
            "schema_version": "leaven.plan_result.v1",
            "plan_id": "acp_extension_result",
            "capability_fingerprint": required_string(
                object.get("capability_fingerprint"),
                "capability_fingerprint",
            )?,
            "policy_fingerprint": object
                .get("policy_fingerprint")
                .and_then(Value::as_str)
                .unwrap_or("fp_policy_sha256_acp_extension"),
            "base_revision": graph_revision,
            "final_revision": graph_revision,
            "replayability_summary": replayability,
            "values": {
                "primary": primary
            },
            "receipts": object
                .get("receipts")
                .ok_or_else(|| invalid_acp("ACP extension result must carry receipts"))?,
            "redactions": object
                .get("redactions")
                .ok_or_else(|| invalid_acp("ACP extension result must carry redactions"))?,
            "charges": [],
            "errors": []
        }))
    }

    /// Extension method name.
    pub const fn method(&self) -> LockedMethod {
        self.method
    }

    /// Primary result value kind.
    pub const fn primary_kind(&self) -> MethodPrimaryKind {
        self.primary_kind
    }

    /// Capability fingerprint attached to the result.
    pub fn capability_fingerprint(&self) -> &str {
        &self.capability_fingerprint
    }

    /// Number of receipts carried by the result.
    pub const fn receipt_count(&self) -> usize {
        self.receipt_count
    }

    /// Number of redactions carried by the result.
    pub const fn redaction_count(&self) -> usize {
        self.redaction_count
    }

    /// Data classes carried by the result.
    pub fn data_classes(&self) -> &[String] {
        &self.data_classes
    }
}

fn validate_primary_kind(
    method: LockedMethod,
    primary_kind: MethodPrimaryKind,
) -> Result<(), PublicSeamError> {
    if method.primary_kinds().contains(&primary_kind) {
        Ok(())
    } else {
        Err(invalid_acp(format!(
            "ACP extension result method `{}` cannot return primary kind `{}`",
            method.as_str(),
            primary_kind.as_str()
        )))
    }
}

fn validate_receipts_for_method(
    method: LockedMethod,
    receipts: &[Value],
) -> Result<(), PublicSeamError> {
    if receipts
        .iter()
        .any(|receipt| receipt_matches(receipt, method.receipt_expectation()))
    {
        Ok(())
    } else {
        Err(invalid_acp(format!(
            "ACP extension result method `{}` is missing its expected receipt",
            method.as_str()
        )))
    }
}

fn validate_primary_result_hash(
    method: LockedMethod,
    primary: &Value,
    receipts: &[Value],
) -> Result<(), PublicSeamError> {
    let receipt = expected_receipt_for_method(method, receipts)?;
    let schema_version = match required_string(receipt.get("kind"), "receipt.kind")? {
        "query" => "leaven.plan_query_result.v1",
        "call" => "leaven.plan_call_result.v1",
        "write" => "leaven.plan_write_result.v1",
        other => {
            return Err(invalid_acp(format!(
                "ACP extension result receipt kind `{other}` cannot bind primary value"
            )));
        }
    };
    let op_name = receipt
        .get("op_var")
        .and_then(Value::as_str)
        .unwrap_or("primary");
    let expected = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": schema_version,
            "name": op_name,
            "value": primary
        }),
    )?;
    let actual = required_string(receipt.get("result_hash"), "receipt.result_hash")?;
    if actual != expected {
        let receipt_id = required_string(receipt.get("receipt"), "receipt.receipt")?;
        return Err(invalid_acp(format!(
            "ACP extension result receipt `{receipt_id}` result_hash does not bind primary value"
        )));
    }
    Ok(())
}

fn validate_effect_primary_audit(
    method: LockedMethod,
    primary: &serde_json::Map<String, Value>,
    expected_receipt: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let expected_receipt_id = required_string(expected_receipt.get("receipt"), "receipt.receipt")?;
    match method {
        LockedMethod::LmComplete => {
            validate_effect_primary_receipt(primary, expected_receipt_id)?;
            validate_effect_primary_cost("lm_complete", primary, expected_receipt)
        }
        LockedMethod::AgentRun => {
            validate_effect_primary_receipt(primary, expected_receipt_id)?;
            validate_agent_session_value("agent_run", None, primary, expected_receipt_id)?;
            validate_effect_primary_cost("agent_run", primary, expected_receipt)
        }
        LockedMethod::SandboxExec => {
            validate_effect_primary_receipt(primary, expected_receipt_id)?;
            validate_sandbox_exec_value("sandbox_exec", primary)?;
            validate_effect_primary_cost("sandbox_exec", primary, expected_receipt)
        }
        LockedMethod::WorkspaceRelease => {
            validate_effect_primary_receipt(primary, expected_receipt_id)?;
            if primary.get("released").and_then(Value::as_bool) == Some(true) {
                Ok(())
            } else {
                Err(invalid_acp(
                    "ACP extension result workspace.release primary must be a released workspace_handle",
                ))
            }
        }
        _ => Ok(()),
    }
}

fn validate_extension_primary_op(
    _method: LockedMethod,
    primary: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    if primary.get("kind").and_then(Value::as_str) != Some("extension") {
        Ok(())
    } else {
        Ok(())
    }
}

fn validate_effect_primary_receipt(
    primary: &serde_json::Map<String, Value>,
    expected_receipt_id: &str,
) -> Result<(), PublicSeamError> {
    let primary_receipt = required_string(primary.get("receipt"), "primary.receipt")?;
    if primary_receipt == expected_receipt_id {
        Ok(())
    } else {
        Err(invalid_acp(format!(
            "ACP extension result primary receipt `{primary_receipt}` does not match expected receipt `{expected_receipt_id}`"
        )))
    }
}

fn validate_effect_primary_cost(
    call_kind: &str,
    primary: &serde_json::Map<String, Value>,
    receipt: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    match (primary.get("cost"), receipt.get("cost")) {
        (Some(primary_cost), Some(receipt_cost)) if primary_cost == receipt_cost => Ok(()),
        (Some(_), _) => Err(invalid_acp(format!(
            "ACP extension result {call_kind} primary cost must match call receipt cost"
        ))),
        (None, Some(_)) => Err(invalid_acp(format!(
            "ACP extension result {call_kind} call receipt cost must have a matching primary cost"
        ))),
        (None, None) => Err(invalid_acp(format!(
            "ACP extension result {call_kind} primary must carry cost"
        ))),
    }
}

fn expected_receipt_for_method<'a>(
    method: LockedMethod,
    receipts: &'a [Value],
) -> Result<&'a serde_json::Map<String, Value>, PublicSeamError> {
    let expectation = method.receipt_expectation();
    receipts
        .iter()
        .find(|receipt| receipt_matches(receipt, expectation))
        .and_then(Value::as_object)
        .ok_or_else(|| {
            invalid_acp(format!(
                "ACP extension result method `{}` is missing its expected receipt",
                method.as_str()
            ))
        })
}

fn receipt_matches(receipt: &Value, expectation: MethodReceiptExpectation) -> bool {
    let Some(object) = receipt.as_object() else {
        return false;
    };
    match expectation {
        MethodReceiptExpectation::StageRun => false,
        MethodReceiptExpectation::Query => {
            object.get("kind").and_then(Value::as_str) == Some("query")
        }
        MethodReceiptExpectation::Call(call_kind) => {
            object.get("kind").and_then(Value::as_str) == Some("call")
                && object.get("call_kind").and_then(Value::as_str) == Some(call_kind)
        }
        MethodReceiptExpectation::Write(write_kind) => {
            object.get("kind").and_then(Value::as_str) == Some("write")
                && object.get("write_kind").and_then(Value::as_str) == Some(write_kind)
        }
    }
}

fn ensure_primary_receipt_is_carried(
    primary_receipt: &str,
    receipts: &[Value],
) -> Result<(), PublicSeamError> {
    if receipts.iter().any(|receipt| {
        receipt
            .as_object()
            .and_then(|object| object.get("receipt"))
            .and_then(Value::as_str)
            == Some(primary_receipt)
    }) {
        Ok(())
    } else {
        Err(invalid_acp(format!(
            "ACP extension result primary receipt `{primary_receipt}` is not carried"
        )))
    }
}
