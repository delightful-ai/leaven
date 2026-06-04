//! Typed projection records for RunContext-backed graph callback results.

use serde::Serialize;
use serde_json::Value;

use crate::graph_host::RunContextGraphEffectHostError;

pub(crate) fn proposal_apply_extension_result(
    plan_result: &Value,
) -> Result<Value, RunContextGraphEffectHostError> {
    let projection = ApplyExtensionProjection::from_plan_result(plan_result)?;
    to_value(projection)
}

pub(crate) fn assessment_submit_extension_result(
    plan_result: &Value,
) -> Result<Value, RunContextGraphEffectHostError> {
    let projection = AssessmentSubmitExtensionProjection::from_plan_result(plan_result)?;
    to_value(projection)
}

pub(crate) fn evaluation_request_extension_result(
    plan_result: &Value,
) -> Result<Value, RunContextGraphEffectHostError> {
    let projection = EvaluationRequestExtensionProjection::from_plan_result(plan_result)?;
    to_value(projection)
}

#[derive(Serialize)]
struct EmptyObject {}

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
    fn from_plan_result(plan_result: &Value) -> Result<Self, RunContextGraphEffectHostError> {
        Ok(Self {
            method: "leaven/proposal.apply",
            primary: ApplyPrimary::from_value(required_pointer(
                plan_result,
                "/values/apply",
                RunContextGraphEffectHostError::MissingApplyWrite,
            )?)?,
            receipts: collect_write_receipts(
                plan_result,
                "apply_proposal_batch",
                ApplyReceipt::from_value,
                RunContextGraphEffectHostError::MissingApplyWrite,
            )?,
            redactions: empty_redactions(plan_result)?,
            capability_fingerprint: optional_string(
                plan_result,
                "capability_fingerprint",
                "fp_cap_sha256_stage_bridge",
            ),
            policy_fingerprint: optional_string(
                plan_result,
                "policy_fingerprint",
                "fp_policy_sha256_stage_bridge",
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
    fn from_value(value: &Value) -> Result<Self, RunContextGraphEffectHostError> {
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
    fn from_value(value: &Value) -> Result<Self, RunContextGraphEffectHostError> {
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
    fn from_plan_result(plan_result: &Value) -> Result<Self, RunContextGraphEffectHostError> {
        let primary = EvaluationRequestPrimary::from_value(required_pointer(
            plan_result,
            "/values/evaluation_request",
            RunContextGraphEffectHostError::MissingEvaluationRequestWrite,
        )?)?;
        let mut receipts = collect_write_receipts(
            plan_result,
            "request_evaluation",
            EvaluationRequestReceipt::from_value,
            RunContextGraphEffectHostError::MissingEvaluationRequestWrite,
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
                "fp_cap_sha256_stage_bridge",
            ),
            policy_fingerprint: optional_string(
                plan_result,
                "policy_fingerprint",
                "fp_policy_sha256_stage_bridge",
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
    fn from_value(value: &Value) -> Result<Self, RunContextGraphEffectHostError> {
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
    fn from_value(value: &Value) -> Result<Self, RunContextGraphEffectHostError> {
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
    fn from_plan_result(plan_result: &Value) -> Result<Self, RunContextGraphEffectHostError> {
        Ok(Self {
            method: "leaven/assessment.submit",
            primary: AssessmentSubmitPrimary::from_value(required_pointer(
                plan_result,
                "/values/assessment_batch",
                RunContextGraphEffectHostError::MissingAssessmentWrite,
            )?)?,
            receipts: collect_write_receipts(
                plan_result,
                "submit_assessments",
                AssessmentSubmitReceipt::from_value,
                RunContextGraphEffectHostError::MissingAssessmentWrite,
            )?,
            redactions: empty_redactions(plan_result)?,
            capability_fingerprint: optional_string(
                plan_result,
                "capability_fingerprint",
                "fp_cap_sha256_stage_bridge",
            ),
            policy_fingerprint: optional_string(
                plan_result,
                "policy_fingerprint",
                "fp_policy_sha256_stage_bridge",
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
    fn from_value(value: &Value) -> Result<Self, RunContextGraphEffectHostError> {
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
    fn from_value(value: &Value) -> Result<Self, RunContextGraphEffectHostError> {
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
    fn from_value(value: &Value) -> Result<Self, RunContextGraphEffectHostError> {
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

fn to_value(value: impl Serialize) -> Result<Value, RunContextGraphEffectHostError> {
    serde_json::to_value(value)
        .map_err(|error| RunContextGraphEffectHostError::Hash(error.to_string()))
}

fn required_pointer<'a>(
    value: &'a Value,
    pointer: &'static str,
    missing: RunContextGraphEffectHostError,
) -> Result<&'a Value, RunContextGraphEffectHostError> {
    value.pointer(pointer).ok_or(missing)
}

fn collect_write_receipts<T>(
    plan_result: &Value,
    write_kind: &'static str,
    parse: impl Fn(&Value) -> Result<T, RunContextGraphEffectHostError>,
    missing: RunContextGraphEffectHostError,
) -> Result<Vec<T>, RunContextGraphEffectHostError> {
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
        return Err(RunContextGraphEffectHostError::InvalidProjection {
            field: "receipts",
            reason: "expected at least one matching write receipt",
        });
    }
    Ok(typed)
}

fn empty_redactions(
    plan_result: &Value,
) -> Result<Vec<EmptyObject>, RunContextGraphEffectHostError> {
    match plan_result.get("redactions").and_then(Value::as_array) {
        Some(redactions) if redactions.is_empty() => Ok(Vec::new()),
        None => Ok(Vec::new()),
        Some(_) => Err(RunContextGraphEffectHostError::InvalidProjection {
            field: "redactions",
            reason: "stage bridge callbacks do not yet own typed redaction projection",
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
) -> Result<&'a str, RunContextGraphEffectHostError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(RunContextGraphEffectHostError::MissingString { field })
}

fn required_array<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<&'a [Value], RunContextGraphEffectHostError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(RunContextGraphEffectHostError::InvalidProjection {
            field,
            reason: "expected array",
        })
}

fn string_array(
    value: &Value,
    field: &'static str,
) -> Result<Vec<String>, RunContextGraphEffectHostError> {
    required_array(value, field)?
        .iter()
        .map(|item| {
            item.as_str().map(ToOwned::to_owned).ok_or(
                RunContextGraphEffectHostError::InvalidProjection {
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
) -> Result<(), RunContextGraphEffectHostError> {
    match value.get("kind").and_then(Value::as_str) {
        Some(actual) if actual == expected => Ok(()),
        _ => Err(RunContextGraphEffectHostError::InvalidProjection {
            field,
            reason: "unexpected kind",
        }),
    }
}

fn require_field(
    value: &Value,
    field: &'static str,
    expected: &'static str,
) -> Result<(), RunContextGraphEffectHostError> {
    match value.get(field).and_then(Value::as_str) {
        Some(actual) if actual == expected => Ok(()),
        _ => Err(RunContextGraphEffectHostError::InvalidProjection {
            field,
            reason: "unexpected field value",
        }),
    }
}

fn prefixed_jcs_hash(
    prefix: &str,
    value: &(impl Serialize + ?Sized),
) -> Result<String, RunContextGraphEffectHostError> {
    let value = serde_json::to_value(value)
        .map_err(|error| RunContextGraphEffectHostError::Hash(error.to_string()))?;
    let digest = jcs_canonicalize::sha256_jcs_hex(&value)
        .map_err(|error| RunContextGraphEffectHostError::Hash(error.to_string()))?;
    Ok(format!("{prefix}{digest}"))
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
            "capability_fingerprint": "fp_cap_sha256_stage_bridge",
            "policy_fingerprint": "fp_policy_sha256_stage_bridge",
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
            "capability_fingerprint": "fp_cap_sha256_stage_bridge",
            "policy_fingerprint": "fp_policy_sha256_stage_bridge",
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
            "capability_fingerprint": "fp_cap_sha256_stage_bridge",
            "policy_fingerprint": "fp_policy_sha256_stage_bridge",
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
