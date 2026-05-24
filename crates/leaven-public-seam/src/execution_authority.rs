use serde_json::Value;

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
