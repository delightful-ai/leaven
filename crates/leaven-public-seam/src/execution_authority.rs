use serde_json::{Value, json};

use crate::{CapabilityDocument, CapabilityGrantRequest, CapabilityLimitUsage, PublicSeamError};

pub fn validate(plan: &Value, capability: &CapabilityDocument) -> Result<(), PublicSeamError> {
    crate::call_authority::validate(plan, capability)?;
    validate_writes(plan, capability)
}

fn validate_writes(plan: &Value, capability: &CapabilityDocument) -> Result<(), PublicSeamError> {
    let ops = plan
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_authority("plan ops must be an array"))?;
    for op in ops {
        let Some(write) = op.get("write").and_then(Value::as_object) else {
            continue;
        };
        match required_string(write.get("kind"), "write.kind")? {
            "emit_run_event" => {
                capability
                    .authorize_grant(CapabilityGrantRequest::for_action("event.emit"))
                    .map_err(|denial| invalid_authority(format!("event emit denied: {denial}")))?;
            }
            "submit_assessments" => validate_submit_assessments(write, capability)?,
            "request_evaluation" => validate_request_evaluation(write, capability)?,
            "submit_proposal_batch" | "apply_proposal_batch" => {
                crate::proposal_authority::validate(plan, capability)?;
            }
            other => {
                return Err(invalid_authority(format!(
                    "write kind `{other}` has no V1 capability action mapping"
                )));
            }
        }
    }
    Ok(())
}

fn validate_submit_assessments(
    write: &serde_json::Map<String, Value>,
    capability: &CapabilityDocument,
) -> Result<(), PublicSeamError> {
    let assessments = write
        .get("assessments")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_authority("submit_assessments must carry assessments"))?;
    capability
        .authorize_grant(
            CapabilityGrantRequest::for_action("assessment.submit")
                .with_resource(
                    "evaluation_request_id",
                    json!(required_string(
                        write.get("evaluation_request_id"),
                        "evaluation_request_id"
                    )?),
                )
                .with_limits(CapabilityLimitUsage {
                    rows: Some(assessments.len() as u64),
                    ..CapabilityLimitUsage::default()
                }),
        )
        .map_err(|denial| invalid_authority(format!("assessment submit denied: {denial}")))?;
    Ok(())
}

fn validate_request_evaluation(
    write: &serde_json::Map<String, Value>,
    capability: &CapabilityDocument,
) -> Result<(), PublicSeamError> {
    let request_object = write
        .get("request")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_authority("request_evaluation must carry request"))?;
    let candidates = request_object
        .get("candidates")
        .cloned()
        .ok_or_else(|| invalid_authority("request_evaluation must carry request.candidates"))?;
    let mut request = CapabilityGrantRequest::for_action("evaluation.request")
        .with_resource("candidate_ids", candidates);
    if let Some(purpose) = request_object.get("purpose").and_then(Value::as_str) {
        request = request.with_purpose(purpose);
    }
    capability
        .authorize_grant(request)
        .map_err(|denial| invalid_authority(format!("evaluation request denied: {denial}")))?;
    Ok(())
}

pub fn limit_usage(object: &serde_json::Map<String, Value>) -> CapabilityLimitUsage {
    CapabilityLimitUsage {
        usd_micro: object.get("max_usd_micro").and_then(Value::as_u64),
        timeout_s: object.get("timeout_s").and_then(Value::as_u64),
        ..CapabilityLimitUsage::default()
    }
}

pub fn output_authority(
    mut request: CapabilityGrantRequest,
    output: Option<&Value>,
) -> Result<CapabilityGrantRequest, PublicSeamError> {
    let Some(output) = output.and_then(Value::as_object) else {
        return Ok(request);
    };
    match output.get("kind").and_then(Value::as_str) {
        Some("json_schema") => {
            request = request.with_schema(required_string(
                output.get("schema_fingerprint"),
                "output.schema_fingerprint",
            )?);
        }
        Some("workspace_diff") => {
            request = request.with_surface(required_string(
                output.get("surface_fingerprint"),
                "output.surface_fingerprint",
            )?);
        }
        _ => {}
    }
    Ok(request)
}

pub fn workspace_ref_id<'a>(
    value: Option<&'a Value>,
    context: &str,
) -> Result<&'a str, PublicSeamError> {
    let value =
        value.ok_or_else(|| invalid_authority(format!("{context} must carry workspace")))?;
    if let Some(workspace) = value.as_str() {
        return Ok(workspace);
    }
    let object = value
        .as_object()
        .ok_or_else(|| invalid_authority(format!("{context} workspace ref must be object")))?;
    if object.get("kind").and_then(Value::as_str) != Some("workspace") {
        return Err(invalid_authority(format!(
            "{context} workspace ref object must have kind `workspace`"
        )));
    }
    required_string(object.get("id"), "workspace.id")
}

pub fn required_string<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<&'a str, PublicSeamError> {
    value.and_then(Value::as_str).ok_or_else(|| {
        invalid_authority(format!("execution authority field `{field}` is required"))
    })
}

pub fn invalid_authority(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}
