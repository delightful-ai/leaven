use serde::Serialize;
use serde_json::Value;

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
    let projection = ApplyExtensionProjection::from_plan_result(plan_result)?;
    to_value(projection)
}

pub(super) fn evaluation_request_extension_result(
    plan_result: &Value,
) -> Result<Value, RunBoundGraphEffectError> {
    let projection = EvaluationRequestExtensionProjection::from_plan_result(plan_result)?;
    to_value(projection)
}

pub(super) fn assessment_submit_extension_result(
    plan_result: &Value,
) -> Result<Value, RunBoundGraphEffectError> {
    let projection = AssessmentSubmitExtensionProjection::from_plan_result(plan_result)?;
    to_value(projection)
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
struct ApplyExtensionProjection {
    method: &'static str,
    primary: ApplyPrimary,
    receipts: Vec<ApplyReceipt>,
    redactions: Vec<EmptyObject>,
    capability_fingerprint: String,
    policy_fingerprint: String,
    data_classes: &'static [&'static str],
}

impl ApplyExtensionProjection {
    fn from_plan_result(plan_result: &Value) -> Result<Self, RunBoundGraphEffectError> {
        Ok(Self {
            method: "leaven/proposal.apply",
            primary: ApplyPrimary::from_value(required_pointer(
                plan_result,
                "/values/apply",
                RunBoundGraphEffectError::MissingApplyWrite,
            )?)?,
            receipts: collect_write_receipts(
                plan_result,
                "apply_proposal_batch",
                ApplyReceipt::from_value,
                RunBoundGraphEffectError::MissingApplyWrite,
            )?,
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
struct ApplyPrimary {
    kind: &'static str,
    created_candidates: Vec<String>,
    status: String,
    graph_revision: String,
    data_classes: Vec<String>,
    replayability: String,
    receipt: String,
}

impl ApplyPrimary {
    fn from_value(value: &Value) -> Result<Self, RunBoundGraphEffectError> {
        require_kind(value, "apply_receipt", "/values/apply/kind")?;
        Ok(Self {
            kind: "apply_receipt",
            created_candidates: string_array(value, "created_candidates")?,
            status: required_string(value, "status")?.to_owned(),
            graph_revision: required_string(value, "graph_revision")?.to_owned(),
            data_classes: string_array(value, "data_classes")?,
            replayability: required_string(value, "replayability")?.to_owned(),
            receipt: required_string(value, "receipt")?.to_owned(),
        })
    }
}

#[derive(Serialize)]
struct ApplyReceipt {
    kind: &'static str,
    receipt: String,
    op_var: String,
    started_at: String,
    completed_at: String,
    write_kind: &'static str,
    request_hash: String,
    result_hash: String,
    base_revision: String,
    committed_revision: String,
    status: String,
    created_candidates: Vec<String>,
}

impl ApplyReceipt {
    fn from_value(value: &Value) -> Result<Self, RunBoundGraphEffectError> {
        require_kind(value, "write", "receipts[].kind")?;
        require_field(value, "write_kind", "apply_proposal_batch")?;
        Ok(Self {
            kind: "write",
            receipt: required_string(value, "receipt")?.to_owned(),
            op_var: required_string(value, "op_var")?.to_owned(),
            started_at: required_string(value, "started_at")?.to_owned(),
            completed_at: required_string(value, "completed_at")?.to_owned(),
            write_kind: "apply_proposal_batch",
            request_hash: required_string(value, "request_hash")?.to_owned(),
            result_hash: required_string(value, "result_hash")?.to_owned(),
            base_revision: required_string(value, "base_revision")?.to_owned(),
            committed_revision: required_string(value, "committed_revision")?.to_owned(),
            status: required_string(value, "status")?.to_owned(),
            created_candidates: string_array(value, "created_candidates")?,
        })
    }
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

#[derive(Serialize)]
struct AssessmentSubmitExtensionProjection {
    method: &'static str,
    primary: AssessmentSubmitPrimary,
    receipts: Vec<AssessmentSubmitReceipt>,
    redactions: Vec<EmptyObject>,
    capability_fingerprint: String,
    policy_fingerprint: String,
    data_classes: &'static [&'static str],
}

impl AssessmentSubmitExtensionProjection {
    fn from_plan_result(plan_result: &Value) -> Result<Self, RunBoundGraphEffectError> {
        Ok(Self {
            method: "leaven/assessment.submit",
            primary: AssessmentSubmitPrimary::from_value(required_pointer(
                plan_result,
                "/values/assessment_batch",
                RunBoundGraphEffectError::MissingAssessmentWrite,
            )?)?,
            receipts: collect_write_receipts(
                plan_result,
                "submit_assessments",
                AssessmentSubmitReceipt::from_value,
                RunBoundGraphEffectError::MissingAssessmentWrite,
            )?,
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
struct AssessmentSubmitPrimary {
    kind: &'static str,
    assessment_ids: Vec<String>,
    evaluation_request_id: String,
    per_assessment: Vec<PerAssessmentReplayability>,
    status: String,
    graph_revision: String,
    data_classes: Vec<String>,
    replayability: String,
    receipt: String,
}

impl AssessmentSubmitPrimary {
    fn from_value(value: &Value) -> Result<Self, RunBoundGraphEffectError> {
        require_kind(
            value,
            "assessment_batch_receipt",
            "/values/assessment_batch/kind",
        )?;
        Ok(Self {
            kind: "assessment_batch_receipt",
            assessment_ids: string_array(value, "assessment_ids")?,
            evaluation_request_id: required_string(value, "evaluation_request_id")?.to_owned(),
            per_assessment: required_array(value, "per_assessment")?
                .iter()
                .map(PerAssessmentReplayability::from_value)
                .collect::<Result<Vec<_>, _>>()?,
            status: required_string(value, "status")?.to_owned(),
            graph_revision: required_string(value, "graph_revision")?.to_owned(),
            data_classes: string_array(value, "data_classes")?,
            replayability: required_string(value, "replayability")?.to_owned(),
            receipt: required_string(value, "receipt")?.to_owned(),
        })
    }
}

#[derive(Serialize)]
struct PerAssessmentReplayability {
    assessment: String,
    replayability: String,
}

impl PerAssessmentReplayability {
    fn from_value(value: &Value) -> Result<Self, RunBoundGraphEffectError> {
        Ok(Self {
            assessment: required_string(value, "assessment")?.to_owned(),
            replayability: required_string(value, "replayability")?.to_owned(),
        })
    }
}

#[derive(Serialize)]
struct AssessmentSubmitReceipt {
    kind: &'static str,
    receipt: String,
    op_var: String,
    started_at: String,
    completed_at: String,
    write_kind: &'static str,
    request_hash: String,
    result_hash: String,
    base_revision: String,
    committed_revision: String,
    status: String,
    evaluation_request_id: String,
    assessment_ids: Vec<String>,
}

impl AssessmentSubmitReceipt {
    fn from_value(value: &Value) -> Result<Self, RunBoundGraphEffectError> {
        require_kind(value, "write", "receipts[].kind")?;
        require_field(value, "write_kind", "submit_assessments")?;
        Ok(Self {
            kind: "write",
            receipt: required_string(value, "receipt")?.to_owned(),
            op_var: required_string(value, "op_var")?.to_owned(),
            started_at: required_string(value, "started_at")?.to_owned(),
            completed_at: required_string(value, "completed_at")?.to_owned(),
            write_kind: "submit_assessments",
            request_hash: required_string(value, "request_hash")?.to_owned(),
            result_hash: required_string(value, "result_hash")?.to_owned(),
            base_revision: required_string(value, "base_revision")?.to_owned(),
            committed_revision: required_string(value, "committed_revision")?.to_owned(),
            status: required_string(value, "status")?.to_owned(),
            evaluation_request_id: required_string(value, "evaluation_request_id")?.to_owned(),
            assessment_ids: string_array(value, "assessment_ids")?,
        })
    }
}

#[derive(Serialize)]
struct WriteResultPreimage<'a, T: Serialize> {
    schema_version: &'static str,
    name: &'a str,
    value: &'a T,
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

fn to_value(value: impl Serialize) -> Result<Value, RunBoundGraphEffectError> {
    serde_json::to_value(value).map_err(|error| RunBoundGraphEffectError::Hash(error.to_string()))
}

fn required_pointer<'a>(
    value: &'a Value,
    pointer: &'static str,
    missing: RunBoundGraphEffectError,
) -> Result<&'a Value, RunBoundGraphEffectError> {
    value.pointer(pointer).ok_or(missing)
}

fn collect_write_receipts<T>(
    plan_result: &Value,
    write_kind: &'static str,
    parse: impl Fn(&Value) -> Result<T, RunBoundGraphEffectError>,
    missing: RunBoundGraphEffectError,
) -> Result<Vec<T>, RunBoundGraphEffectError> {
    let receipts = plan_result
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or(missing)?;
    let typed = receipts
        .iter()
        .filter(|receipt| receipt.get("write_kind").and_then(Value::as_str) == Some(write_kind))
        .map(parse)
        .collect::<Result<Vec<_>, _>>()?;
    if typed.is_empty() {
        return Err(RunBoundGraphEffectError::InvalidProjection {
            field: "receipts",
            reason: "expected at least one matching write receipt",
        });
    }
    Ok(typed)
}

fn empty_redactions(plan_result: &Value) -> Result<Vec<EmptyObject>, RunBoundGraphEffectError> {
    match plan_result.get("redactions").and_then(Value::as_array) {
        Some(redactions) if redactions.is_empty() => Ok(Vec::new()),
        None => Ok(Vec::new()),
        Some(_) => Err(RunBoundGraphEffectError::InvalidProjection {
            field: "redactions",
            reason: "run-bound graph callbacks do not yet own typed redaction projection",
        }),
    }
}

fn optional_string(value: &Value, field: &'static str, default: &'static str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_owned()
}

fn required_string<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<&'a str, RunBoundGraphEffectError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(RunBoundGraphEffectError::MissingString { field })
}

fn required_array<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<&'a [Value], RunBoundGraphEffectError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(RunBoundGraphEffectError::InvalidProjection {
            field,
            reason: "expected array",
        })
}

fn string_array(
    value: &Value,
    field: &'static str,
) -> Result<Vec<String>, RunBoundGraphEffectError> {
    required_array(value, field)?
        .iter()
        .map(|item| {
            item.as_str().map(ToOwned::to_owned).ok_or(
                RunBoundGraphEffectError::InvalidProjection {
                    field,
                    reason: "expected string array",
                },
            )
        })
        .collect()
}

fn require_kind(
    value: &Value,
    expected: &'static str,
    field: &'static str,
) -> Result<(), RunBoundGraphEffectError> {
    match value.get("kind").and_then(Value::as_str) {
        Some(actual) if actual == expected => Ok(()),
        _ => Err(RunBoundGraphEffectError::InvalidProjection {
            field,
            reason: "unexpected kind",
        }),
    }
}

fn require_field(
    value: &Value,
    field: &'static str,
    expected: &'static str,
) -> Result<(), RunBoundGraphEffectError> {
    match value.get(field).and_then(Value::as_str) {
        Some(actual) if actual == expected => Ok(()),
        _ => Err(RunBoundGraphEffectError::InvalidProjection {
            field,
            reason: "unexpected field value",
        }),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        assessment_submit_extension_result, evaluation_request_extension_result,
        proposal_apply_extension_result,
    };

    #[test]
    fn apply_extension_projection_rejects_wrong_primary_kind() {
        let mut result = apply_plan_result();
        result["values"]["apply"]["kind"] = json!("proposal_batch_receipt");

        let error = proposal_apply_extension_result(&result).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("graph-backed public-seam projection field `/values/apply/kind`"),
            "{error}"
        );
    }

    #[test]
    fn evaluation_extension_projection_requires_matching_write_receipt() {
        let mut result = evaluation_request_plan_result();
        result["receipts"][0]["write_kind"] = json!("submit_assessments");

        let error = evaluation_request_extension_result(&result).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("expected at least one matching write receipt"),
            "{error}"
        );
    }

    #[test]
    fn assessment_extension_projection_rejects_untyped_per_assessment_rows() {
        let mut result = assessment_submit_plan_result();
        result["values"]["assessment_batch"]["per_assessment"][0] =
            json!({"assessment": {"id": "assess_1"}, "replayability": "fully_managed"});

        let error = assessment_submit_extension_result(&result).unwrap_err();

        assert!(
            error.to_string().contains("assessment must be a string"),
            "{error}"
        );
    }

    fn apply_plan_result() -> serde_json::Value {
        json!({
            "capability_fingerprint": "fp_cap_sha256_run_bound",
            "policy_fingerprint": "fp_policy_sha256_run_bound",
            "values": {
                "apply": {
                    "kind": "apply_receipt",
                    "created_candidates": ["cand_1"],
                    "status": "committed",
                    "graph_revision": "rev_final",
                    "data_classes": ["public"],
                    "replayability": "fully_managed",
                    "receipt": "wrec_apply"
                }
            },
            "receipts": [{
                "kind": "write",
                "receipt": "wrec_apply",
                "op_var": "apply",
                "started_at": "2026-06-04T00:00:00Z",
                "completed_at": "2026-06-04T00:00:01Z",
                "write_kind": "apply_proposal_batch",
                "request_hash": "fp_request_sha256_apply",
                "result_hash": "fp_result_sha256_apply",
                "base_revision": "rev_base",
                "committed_revision": "rev_final",
                "status": "succeeded",
                "created_candidates": ["cand_1"]
            }],
            "redactions": []
        })
    }

    fn evaluation_request_plan_result() -> serde_json::Value {
        json!({
            "capability_fingerprint": "fp_cap_sha256_run_bound",
            "policy_fingerprint": "fp_policy_sha256_run_bound",
            "values": {
                "evaluation_request": {
                    "kind": "evaluation_request_receipt",
                    "receipt": "wrec_evalreq_1",
                    "evaluation_request_id": "evalreq_1",
                    "status": "recorded",
                    "graph_revision": "rev_base",
                    "data_classes": ["public"],
                    "replayability": "fully_managed"
                }
            },
            "receipts": [{
                "kind": "write",
                "write_kind": "request_evaluation",
                "receipt": "wrec_evalreq_1",
                "started_at": "2026-06-04T00:00:00Z",
                "completed_at": "2026-06-04T00:00:01Z",
                "request_hash": "fp_request_sha256_eval",
                "result_hash": "fp_result_sha256_eval",
                "base_revision": "rev_base",
                "committed_revision": "rev_base",
                "status": "succeeded",
                "evaluation_request_id": "evalreq_1"
            }],
            "redactions": []
        })
    }

    fn assessment_submit_plan_result() -> serde_json::Value {
        json!({
            "capability_fingerprint": "fp_cap_sha256_run_bound",
            "policy_fingerprint": "fp_policy_sha256_run_bound",
            "values": {
                "assessment_batch": {
                    "kind": "assessment_batch_receipt",
                    "assessment_ids": ["assess_1"],
                    "evaluation_request_id": "evalreq_1",
                    "per_assessment": [{
                        "assessment": "assess_1",
                        "replayability": "fully_managed"
                    }],
                    "status": "committed",
                    "graph_revision": "rev_final",
                    "data_classes": ["public"],
                    "replayability": "fully_managed",
                    "receipt": "wrec_assess"
                }
            },
            "receipts": [{
                "kind": "write",
                "receipt": "wrec_assess",
                "op_var": "assessment_batch",
                "started_at": "2026-06-04T00:00:00Z",
                "completed_at": "2026-06-04T00:00:01Z",
                "write_kind": "submit_assessments",
                "request_hash": "fp_request_sha256_assess",
                "result_hash": "fp_result_sha256_assess",
                "base_revision": "rev_base",
                "committed_revision": "rev_final",
                "status": "succeeded",
                "evaluation_request_id": "evalreq_1",
                "assessment_ids": ["assess_1"]
            }],
            "redactions": []
        })
    }
}
