use serde::Serialize;
use serde_json::Value;

use super::common::{
    EmptyObject, WriteResultPreimage, collect_write_receipts, empty_redactions, optional_string,
    prefixed_jcs_hash, require_field, require_kind, required_pointer, required_string,
    string_array, to_value,
};
use crate::run_bound_service::RunBoundGraphEffectError;

pub fn evaluation_request_extension_result(
    plan_result: &Value,
) -> Result<Value, RunBoundGraphEffectError> {
    let projection = EvaluationRequestExtensionProjection::from_plan_result(plan_result)?;
    to_value(projection)
}

#[derive(Serialize)]
struct EvaluationRequestExtensionProjection {
    method: &'static str,
    primary: EvaluationRequestPrimary,
    receipts: Vec<EvaluationRequestReceipt>,
    redactions: Vec<EmptyObject>,
    capability_fingerprint: String,
    policy_fingerprint: String,
    data_classes: &'static [&'static str],
}

impl EvaluationRequestExtensionProjection {
    fn from_plan_result(plan_result: &Value) -> Result<Self, RunBoundGraphEffectError> {
        let primary = EvaluationRequestPrimary::from_value(required_pointer(
            plan_result,
            "/values/evaluation_request",
            RunBoundGraphEffectError::MissingEvaluationRequestWrite,
        )?)?;
        let mut receipts = collect_write_receipts(
            plan_result,
            "request_evaluation",
            EvaluationRequestReceipt::from_value,
            RunBoundGraphEffectError::MissingEvaluationRequestWrite,
        )?;
        for receipt in &mut receipts {
            receipt.result_hash = prefixed_jcs_hash(
                "fp_result_sha256_",
                &WriteResultPreimage {
                    schema_version: "leaven.plan_write_result.v1",
                    name: &receipt.op_var,
                    value: &primary,
                },
            )?;
        }
        Ok(Self {
            method: "leaven/evaluation.request",
            primary,
            receipts,
            redactions: empty_redactions(plan_result)?,
            capability_fingerprint: optional_string(
                plan_result,
                "capability_fingerprint",
                "fp_cap_sha256_run_bound",
            ),
            policy_fingerprint: optional_string(
                plan_result,
                "policy_fingerprint",
                "fp_policy_sha256_run_bound",
            ),
            data_classes: &["public"],
        })
    }
}

#[derive(Serialize)]
struct EvaluationRequestPrimary {
    kind: &'static str,
    receipt: String,
    evaluation_request_id: String,
    status: String,
    graph_revision: String,
    data_classes: Vec<String>,
    replayability: String,
}

impl EvaluationRequestPrimary {
    fn from_value(value: &Value) -> Result<Self, RunBoundGraphEffectError> {
        require_kind(
            value,
            "evaluation_request_receipt",
            "/values/evaluation_request/kind",
        )?;
        Ok(Self {
            kind: "evaluation_request_receipt",
            receipt: required_string(value, "receipt")?.to_owned(),
            evaluation_request_id: required_string(value, "evaluation_request_id")?.to_owned(),
            status: required_string(value, "status")?.to_owned(),
            graph_revision: required_string(value, "graph_revision")?.to_owned(),
            data_classes: string_array(value, "data_classes")?,
            replayability: required_string(value, "replayability")?.to_owned(),
        })
    }
}

#[derive(Serialize)]
struct EvaluationRequestReceipt {
    kind: &'static str,
    write_kind: &'static str,
    receipt: String,
    started_at: String,
    completed_at: String,
    request_hash: String,
    result_hash: String,
    base_revision: String,
    committed_revision: String,
    status: String,
    evaluation_request_id: String,
    op_var: String,
}

impl EvaluationRequestReceipt {
    fn from_value(value: &Value) -> Result<Self, RunBoundGraphEffectError> {
        require_kind(value, "write", "receipts[].kind")?;
        require_field(value, "write_kind", "request_evaluation")?;
        Ok(Self {
            kind: "write",
            write_kind: "request_evaluation",
            receipt: required_string(value, "receipt")?.to_owned(),
            started_at: required_string(value, "started_at")?.to_owned(),
            completed_at: required_string(value, "completed_at")?.to_owned(),
            request_hash: required_string(value, "request_hash")?.to_owned(),
            result_hash: required_string(value, "result_hash")?.to_owned(),
            base_revision: required_string(value, "base_revision")?.to_owned(),
            committed_revision: required_string(value, "committed_revision")?.to_owned(),
            status: required_string(value, "status")?.to_owned(),
            evaluation_request_id: required_string(value, "evaluation_request_id")?.to_owned(),
            op_var: value
                .get("op_var")
                .and_then(Value::as_str)
                .unwrap_or("primary")
                .to_owned(),
        })
    }
}
