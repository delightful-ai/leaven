use serde_json::{Map, Value};

pub fn validate_closed_plan_error(error: &Map<String, Value>) -> Result<(), String> {
    for key in error.keys() {
        if !matches!(
            key.as_str(),
            "code" | "message" | "op" | "path" | "receipt" | "retryable" | "details"
        ) {
            return Err(format!("PlanError carries unknown field `{key}`"));
        }
    }
    let code = required_string(error.get("code"), "PlanError.code")?;
    if !is_closed_plan_error_code(code) {
        return Err("PlanError code must be a closed public-seam error code".to_owned());
    }
    let message = required_string(error.get("message"), "PlanError.message")?;
    if message.trim().is_empty() {
        return Err("PlanError message must be non-empty".to_owned());
    }
    plan_error_receipt_id(error)?;
    Ok(())
}

pub fn plan_error_receipt_id(error: &Map<String, Value>) -> Result<&str, String> {
    let receipt = error
        .get("receipt")
        .ok_or_else(|| "PlanError receipt must be present".to_owned())?;
    receipt_ref_id(receipt, "PlanError receipt")
}

pub fn receipt_ref_id<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    if let Some(receipt) = value.as_str() {
        return Ok(receipt);
    }
    let object = value
        .as_object()
        .ok_or_else(|| format!("{field} must be a ReceiptRef"))?;
    if object.get("kind").and_then(Value::as_str) != Some("receipt") {
        return Err(format!("{field} object must have kind `receipt`"));
    }
    required_string(object.get("id"), &format!("{field} id"))
}

pub fn is_closed_plan_error_code(code: &str) -> bool {
    matches!(
        code,
        "token_invalid"
            | "token_expired"
            | "token_revoked"
            | "capability_denied"
            | "budget_exceeded"
            | "quota_exceeded"
            | "hidden_partition_violation"
            | "data_class_violation"
            | "schema_validation_failed"
            | "stage_runtime_error"
            | "precondition_failed"
            | "revision_stale"
            | "rate_limited"
            | "cancelled"
            | "timeout"
            | "provider_policy_denied"
            | "provider_error"
            | "workspace_policy_denied"
            | "path_denied"
            | "sandbox_denied"
            | "watch_closed"
            | "internal_error"
            | "extension_error"
    )
}

fn required_string<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, String> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} must be a string"))
}
