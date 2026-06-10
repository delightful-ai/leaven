use serde_json::{Value, json};

use super::{
    LockedMethod, MethodPrimaryKind, MethodReceiptExpectation, invalid_acp, prefixed_jcs_hash,
    required_array, required_string, string_array,
};
use crate::{
    PlanResultReceiptKind, PublicSeamError,
    plan_execution::{validate_agent_session_value, validate_sandbox_exec_value},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpExtensionResultDocument {
    method: LockedMethod,
    primary: AcpExtensionPrimaryFact,
    expected_receipt: AcpExtensionReceiptFact,
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
        let receipts = AcpExtensionReceiptFacts::from_values(required_array(
            object.get("receipts"),
            "receipts",
        )?)?;
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
        let expected_receipt = receipts.expected_for_method(method)?;
        validate_primary_result_hash(primary_value, expected_receipt)?;
        validate_effect_primary_audit(method, primary, expected_receipt)?;
        let primary_receipt = primary.get("receipt").and_then(Value::as_str);
        if let Some(primary_receipt) = primary_receipt {
            receipts.ensure_receipt_is_carried(primary_receipt)?;
        }
        Ok(Self {
            method,
            primary: AcpExtensionPrimaryFact {
                kind: primary_kind,
                receipt: primary_receipt.map(ToOwned::to_owned),
            },
            expected_receipt: expected_receipt.clone(),
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

    /// Typed primary result facts validated against the locked method.
    pub const fn primary(&self) -> &AcpExtensionPrimaryFact {
        &self.primary
    }

    /// Typed receipt facts for the method-specific receipt that binds the primary value.
    pub const fn expected_receipt(&self) -> &AcpExtensionReceiptFact {
        &self.expected_receipt
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

/// Typed primary result facts for an ACP extension result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpExtensionPrimaryFact {
    kind: MethodPrimaryKind,
    receipt: Option<String>,
}

impl AcpExtensionPrimaryFact {
    /// Primary result value kind.
    pub const fn kind(&self) -> MethodPrimaryKind {
        self.kind
    }

    /// Receipt id carried by effect/write primaries, when the primary schema has one.
    pub fn receipt(&self) -> Option<&str> {
        self.receipt.as_deref()
    }
}

/// Typed receipt facts for the ACP extension receipt that binds the primary value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpExtensionReceiptFact {
    receipt: String,
    kind: PlanResultReceiptKind,
    call_kind: Option<String>,
    write_kind: Option<String>,
    op_var: Option<String>,
    result_hash: Option<String>,
    cost_fingerprint: Option<String>,
}

impl AcpExtensionReceiptFact {
    fn from_value(value: &Value) -> Result<Self, PublicSeamError> {
        let receipt = value
            .as_object()
            .ok_or_else(|| invalid_acp("ACP extension result receipt must be an object"))?;
        let receipt_id = required_string(receipt.get("receipt"), "receipt.receipt")?.to_owned();
        let kind_name = required_string(receipt.get("kind"), "receipt.kind")?;
        let kind = PlanResultReceiptKind::parse(kind_name)
            .ok_or_else(|| invalid_acp(format!("unknown receipt kind `{kind_name}`")))?;
        let cost_fingerprint = receipt
            .get("cost")
            .map(acp_extension_cost_fingerprint)
            .transpose()?;
        Ok(Self {
            receipt: receipt_id,
            kind,
            call_kind: receipt
                .get("call_kind")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            write_kind: receipt
                .get("write_kind")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            op_var: receipt
                .get("op_var")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            result_hash: receipt
                .get("result_hash")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            cost_fingerprint,
        })
    }

    /// Receipt id.
    pub fn receipt(&self) -> &str {
        &self.receipt
    }

    /// Operation receipt kind.
    pub const fn kind(&self) -> PlanResultReceiptKind {
        self.kind
    }

    /// Call kind for call receipts.
    pub fn call_kind(&self) -> Option<&str> {
        self.call_kind.as_deref()
    }

    /// Write kind for write receipts.
    pub fn write_kind(&self) -> Option<&str> {
        self.write_kind.as_deref()
    }

    /// Operation variable bound by the receipt, when the receipt carries one.
    pub fn op_var(&self) -> Option<&str> {
        self.op_var.as_deref()
    }

    /// Result hash carried by the receipt, when present.
    pub fn result_hash(&self) -> Option<&str> {
        self.result_hash.as_deref()
    }

    /// Canonical fingerprint of the receipt cost payload, when the receipt carries cost.
    pub fn cost_fingerprint(&self) -> Option<&str> {
        self.cost_fingerprint.as_deref()
    }

    fn matches_expectation(&self, expectation: MethodReceiptExpectation) -> bool {
        match expectation {
            MethodReceiptExpectation::StageRun | MethodReceiptExpectation::OptimizeRun => false,
            MethodReceiptExpectation::Query => self.kind == PlanResultReceiptKind::Query,
            MethodReceiptExpectation::Call(call_kind) => {
                self.kind == PlanResultReceiptKind::Call && self.call_kind() == Some(call_kind)
            }
            MethodReceiptExpectation::Write(write_kind) => {
                self.kind == PlanResultReceiptKind::Write && self.write_kind() == Some(write_kind)
            }
        }
    }

    fn schema_version(&self) -> &'static str {
        match self.kind {
            PlanResultReceiptKind::Query => "leaven.plan_query_result.v1",
            PlanResultReceiptKind::Call => "leaven.plan_call_result.v1",
            PlanResultReceiptKind::Write => "leaven.plan_write_result.v1",
        }
    }

    fn op_name(&self) -> &str {
        self.op_var.as_deref().unwrap_or("primary")
    }

    fn required_result_hash(&self) -> Result<&str, PublicSeamError> {
        self.result_hash
            .as_deref()
            .ok_or_else(|| invalid_acp("receipt.result_hash must be a string"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AcpExtensionReceiptFacts {
    receipts: Vec<AcpExtensionReceiptFact>,
}

impl AcpExtensionReceiptFacts {
    fn from_values(values: &[Value]) -> Result<Self, PublicSeamError> {
        if values.is_empty() {
            return Err(invalid_acp("ACP extension result must carry receipts"));
        }
        let receipts = values
            .iter()
            .map(AcpExtensionReceiptFact::from_value)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { receipts })
    }

    fn expected_for_method(
        &self,
        method: LockedMethod,
    ) -> Result<&AcpExtensionReceiptFact, PublicSeamError> {
        let expectation = method.receipt_expectation();
        self.receipts
            .iter()
            .find(|receipt| receipt.matches_expectation(expectation))
            .ok_or_else(|| {
                invalid_acp(format!(
                    "ACP extension result method `{}` is missing its expected receipt",
                    method.as_str()
                ))
            })
    }

    fn ensure_receipt_is_carried(&self, primary_receipt: &str) -> Result<(), PublicSeamError> {
        if self
            .receipts
            .iter()
            .any(|receipt| receipt.receipt() == primary_receipt)
        {
            Ok(())
        } else {
            Err(invalid_acp(format!(
                "ACP extension result primary receipt `{primary_receipt}` is not carried"
            )))
        }
    }

    fn len(&self) -> usize {
        self.receipts.len()
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

fn validate_primary_result_hash(
    primary: &Value,
    expected_receipt: &AcpExtensionReceiptFact,
) -> Result<(), PublicSeamError> {
    let schema_version = expected_receipt.schema_version();
    let op_name = expected_receipt.op_name();
    let expected = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": schema_version,
            "name": op_name,
            "value": primary
        }),
    )?;
    let actual = expected_receipt.required_result_hash()?;
    if actual != expected {
        return Err(invalid_acp(format!(
            "ACP extension result receipt `{}` result_hash does not bind primary value",
            expected_receipt.receipt()
        )));
    }
    Ok(())
}

fn validate_effect_primary_audit(
    method: LockedMethod,
    primary: &serde_json::Map<String, Value>,
    expected_receipt: &AcpExtensionReceiptFact,
) -> Result<(), PublicSeamError> {
    let expected_receipt_id = expected_receipt.receipt();
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
    receipt: &AcpExtensionReceiptFact,
) -> Result<(), PublicSeamError> {
    let primary_cost = primary
        .get("cost")
        .map(acp_extension_cost_fingerprint)
        .transpose()?;
    match (primary_cost.as_deref(), receipt.cost_fingerprint()) {
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

fn acp_extension_cost_fingerprint(value: &Value) -> Result<String, PublicSeamError> {
    prefixed_jcs_hash("fp_cost_sha256_", value)
}
