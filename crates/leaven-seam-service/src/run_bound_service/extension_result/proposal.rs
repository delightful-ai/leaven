use serde::Serialize;
use serde_json::Value;

use super::common::{
    EmptyObject, collect_write_receipts, empty_redactions, optional_string, require_field,
    require_kind, required_pointer, required_string, string_array, to_value,
};
use crate::run_bound_service::RunBoundGraphEffectError;

pub fn proposal_apply_extension_result(
    plan_result: &Value,
) -> Result<Value, RunBoundGraphEffectError> {
    let projection = ApplyExtensionProjection::from_plan_result(plan_result)?;
    to_value(projection)
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
