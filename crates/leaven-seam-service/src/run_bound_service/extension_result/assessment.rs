use serde::Serialize;
use serde_json::Value;

use super::common::{
    EmptyObject, collect_write_receipts, empty_redactions, optional_string, require_field,
    require_kind, required_array, required_pointer, required_string, string_array, to_value,
};
use crate::run_bound_service::RunBoundGraphEffectError;

pub fn assessment_submit_extension_result(
    plan_result: &Value,
) -> Result<Value, RunBoundGraphEffectError> {
    let projection = AssessmentSubmitExtensionProjection::from_plan_result(plan_result)?;
    to_value(projection)
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
