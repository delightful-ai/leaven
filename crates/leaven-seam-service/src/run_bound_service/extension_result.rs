use serde::Serialize;
use serde_json::{Value, json};

use super::RunBoundGraphEffectError;

pub(super) struct EventEmitExtensionContext<'a> {
    pub(super) plan_id: &'a str,
    pub(super) name: &'a str,
    pub(super) event_kind: &'a str,
    pub(super) payload_schema: &'a str,
    pub(super) payload: &'a Value,
    pub(super) visibility: &'a str,
    pub(super) event_id: &'a str,
    pub(super) base_revision: &'a str,
    pub(super) final_revision: &'a str,
    pub(super) capability_fingerprint: &'a str,
    pub(super) policy_fingerprint: &'a str,
    pub(super) started_at: &'a str,
    pub(super) completed_at: &'a str,
    pub(super) return_values: Option<&'a Value>,
}

pub(super) fn proposal_apply_extension_result(
    plan_result: &Value,
) -> Result<Value, RunBoundGraphEffectError> {
    let primary = plan_result
        .pointer("/values/apply")
        .cloned()
        .ok_or(RunBoundGraphEffectError::MissingApplyWrite)?;
    let receipts = plan_result
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or(RunBoundGraphEffectError::MissingApplyWrite)?
        .iter()
        .filter(|receipt| {
            receipt.get("write_kind").and_then(Value::as_str) == Some("apply_proposal_batch")
        })
        .cloned()
        .collect::<Vec<_>>();
    Ok(json!({
        "method": "leaven/proposal.apply",
        "primary": primary,
        "receipts": receipts,
        "redactions": plan_result.get("redactions").cloned().unwrap_or_else(|| json!([])),
        "capability_fingerprint": plan_result.get("capability_fingerprint").cloned().unwrap_or_else(|| json!("fp_cap_sha256_run_bound")),
        "policy_fingerprint": plan_result.get("policy_fingerprint").cloned().unwrap_or_else(|| json!("fp_policy_sha256_run_bound")),
        "data_classes": ["public"]
    }))
}

pub(super) fn evaluation_request_extension_result(
    plan_result: &Value,
) -> Result<Value, RunBoundGraphEffectError> {
    let primary = plan_result
        .pointer("/values/evaluation_request")
        .cloned()
        .ok_or(RunBoundGraphEffectError::MissingEvaluationRequestWrite)?;
    let mut receipts = plan_result
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or(RunBoundGraphEffectError::MissingEvaluationRequestWrite)?
        .iter()
        .filter(|receipt| {
            receipt.get("write_kind").and_then(Value::as_str) == Some("request_evaluation")
        })
        .cloned()
        .collect::<Vec<_>>();
    for receipt in &mut receipts {
        let op_name = receipt
            .get("op_var")
            .and_then(Value::as_str)
            .unwrap_or("primary");
        receipt["result_hash"] = json!(prefixed_jcs_hash(
            "fp_result_sha256_",
            &json!({
                "schema_version": "leaven.plan_write_result.v1",
                "name": op_name,
                "value": primary
            }),
        )?);
    }
    Ok(json!({
        "method": "leaven/evaluation.request",
        "primary": primary,
        "receipts": receipts,
        "redactions": plan_result.get("redactions").cloned().unwrap_or_else(|| json!([])),
        "capability_fingerprint": plan_result.get("capability_fingerprint").cloned().unwrap_or_else(|| json!("fp_cap_sha256_run_bound")),
        "policy_fingerprint": plan_result.get("policy_fingerprint").cloned().unwrap_or_else(|| json!("fp_policy_sha256_run_bound")),
        "data_classes": ["public"]
    }))
}

pub(super) fn assessment_submit_extension_result(
    plan_result: &Value,
) -> Result<Value, RunBoundGraphEffectError> {
    let primary = plan_result
        .pointer("/values/assessment_batch")
        .cloned()
        .ok_or(RunBoundGraphEffectError::MissingAssessmentWrite)?;
    let receipts = plan_result
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or(RunBoundGraphEffectError::MissingAssessmentWrite)?
        .iter()
        .filter(|receipt| {
            receipt.get("write_kind").and_then(Value::as_str) == Some("submit_assessments")
        })
        .cloned()
        .collect::<Vec<_>>();
    Ok(json!({
        "method": "leaven/assessment.submit",
        "primary": primary,
        "receipts": receipts,
        "redactions": plan_result.get("redactions").cloned().unwrap_or_else(|| json!([])),
        "capability_fingerprint": plan_result.get("capability_fingerprint").cloned().unwrap_or_else(|| json!("fp_cap_sha256_run_bound")),
        "policy_fingerprint": plan_result.get("policy_fingerprint").cloned().unwrap_or_else(|| json!("fp_policy_sha256_run_bound")),
        "data_classes": ["public"]
    }))
}

pub(super) fn event_emit_extension_result(
    context: EventEmitExtensionContext<'_>,
) -> Result<Value, RunBoundGraphEffectError> {
    let receipt_id = format!("wrec_{}", context.name);
    let request_hash = prefixed_jcs_hash(
        "fp_request_sha256_",
        &EventEmitRequestPreimage {
            schema_version: "leaven.plan_write_request.v1",
            name: context.name,
            kind: "emit_run_event",
            write: EventEmitWriteProjection {
                kind: "emit_run_event",
                event_kind: context.event_kind,
                payload_schema: context.payload_schema,
                payload: context.payload,
                visibility: context.visibility,
            },
            deps: EmptyObject {},
            dependency_data_classes: &[],
            base_revision: context.base_revision,
        },
    )?;
    let primary = EventEmitPrimary {
        kind: "emit_run_event",
        event_id: context.event_id,
        receipt: &receipt_id,
        data_classes: &["public"],
        replayability: "fully_managed",
    };
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &EventEmitResultPreimage {
            schema_version: "leaven.plan_write_result.v1",
            name: context.name,
            value: &primary,
        },
    )?;
    let result = EventEmitExtensionResult {
        method: "leaven/event.emit",
        primary,
        receipts: vec![EventEmitReceipt {
            kind: "write",
            receipt: &receipt_id,
            op_var: context.name,
            started_at: context.started_at,
            completed_at: context.completed_at,
            write_kind: "emit_run_event",
            request_hash: &request_hash,
            result_hash: &result_hash,
            base_revision: context.base_revision,
            committed_revision: context.final_revision,
            status: "succeeded",
            event_id: context.event_id,
        }],
        redactions: &[],
        capability_fingerprint: context.capability_fingerprint,
        policy_fingerprint: context.policy_fingerprint,
        data_classes: &["public"],
        plan_id: context.plan_id,
        return_values: EventEmitReturnValues::from(context.return_values),
    };
    serde_json::to_value(result).map_err(|error| RunBoundGraphEffectError::Hash(error.to_string()))
}

#[derive(Serialize)]
struct EmptyObject {}

#[derive(Serialize)]
struct EventEmitWriteProjection<'a> {
    kind: &'static str,
    event_kind: &'a str,
    payload_schema: &'a str,
    payload: &'a Value,
    visibility: &'a str,
}

#[derive(Serialize)]
struct EventEmitRequestPreimage<'a> {
    schema_version: &'static str,
    name: &'a str,
    kind: &'static str,
    write: EventEmitWriteProjection<'a>,
    deps: EmptyObject,
    dependency_data_classes: &'static [&'static str],
    base_revision: &'a str,
}

#[derive(Serialize)]
struct EventEmitPrimary<'a> {
    kind: &'static str,
    event_id: &'a str,
    receipt: &'a str,
    data_classes: &'static [&'static str],
    replayability: &'static str,
}

#[derive(Serialize)]
struct EventEmitResultPreimage<'a> {
    schema_version: &'static str,
    name: &'a str,
    value: &'a EventEmitPrimary<'a>,
}

#[derive(Serialize)]
struct EventEmitReceipt<'a> {
    kind: &'static str,
    receipt: &'a str,
    op_var: &'a str,
    started_at: &'a str,
    completed_at: &'a str,
    write_kind: &'static str,
    request_hash: &'a str,
    result_hash: &'a str,
    base_revision: &'a str,
    committed_revision: &'a str,
    status: &'static str,
    event_id: &'a str,
}

#[derive(Serialize)]
struct EventEmitExtensionResult<'a> {
    method: &'static str,
    primary: EventEmitPrimary<'a>,
    receipts: Vec<EventEmitReceipt<'a>>,
    redactions: &'static [&'static str],
    capability_fingerprint: &'a str,
    policy_fingerprint: &'a str,
    data_classes: &'static [&'static str],
    plan_id: &'a str,
    #[serde(rename = "return")]
    return_values: EventEmitReturnValues<'a>,
}

enum EventEmitReturnValues<'a> {
    Empty,
    Values(&'a Value),
}

impl<'a> EventEmitReturnValues<'a> {
    fn from(values: Option<&'a Value>) -> Self {
        values.map_or(Self::Empty, Self::Values)
    }
}

impl Serialize for EventEmitReturnValues<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Empty => <[&str; 0]>::default().serialize(serializer),
            Self::Values(values) => values.serialize(serializer),
        }
    }
}

fn prefixed_jcs_hash(
    prefix: &str,
    value: &(impl Serialize + ?Sized),
) -> Result<String, RunBoundGraphEffectError> {
    let value = serde_json::to_value(value)
        .map_err(|error| RunBoundGraphEffectError::Hash(error.to_string()))?;
    let digest = jcs_canonicalize::sha256_jcs_hex(&value)
        .map_err(|error| RunBoundGraphEffectError::Hash(error.to_string()))?;
    Ok(format!("{prefix}{digest}"))
}
