use serde_json::{Map, Value, json};

use super::PublicStagePayloadError;

pub(super) fn stage_payload_fingerprint(value: &Value) -> Result<String, PublicStagePayloadError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value)
        .map_err(|error| PublicStagePayloadError::Fingerprint(error.to_string()))?;
    Ok(format!("fp_stage_payload_sha256_{digest}"))
}

pub(super) fn schema_bound_payload(
    role: &'static str,
    run: impl Into<String>,
    stage_call_id: impl Into<String>,
    payload_field: &'static str,
    payload: Value,
    payload_schema: impl Into<String>,
    capability_fingerprint: impl Into<String>,
) -> Result<Value, PublicStagePayloadError> {
    let mut object = stage_object(role);
    insert_non_empty(&mut object, "run", run)?;
    insert_non_empty(&mut object, "stage_call_id", stage_call_id)?;
    object.insert(payload_field.to_owned(), payload);
    insert_non_empty(&mut object, "payload_schema", payload_schema)?;
    insert_non_empty(
        &mut object,
        "capability_fingerprint",
        capability_fingerprint,
    )?;
    Ok(Value::Object(object))
}

pub(super) fn stage_object(role: &'static str) -> Map<String, Value> {
    let mut object = Map::new();
    object.insert(
        "schema_version".to_owned(),
        json!("leaven.stage_payloads.v1"),
    );
    object.insert("role".to_owned(), json!(role));
    object
}

pub(super) fn insert_non_empty(
    object: &mut Map<String, Value>,
    field: &'static str,
    value: impl Into<String>,
) -> Result<(), PublicStagePayloadError> {
    object.insert(field.to_owned(), json!(non_empty(value.into(), field)?));
    Ok(())
}

pub(super) fn require_assessed_output_class(
    output: &Value,
    field: &'static str,
) -> Result<(), PublicStagePayloadError> {
    let carries_assessed_output = output
        .get("data_classes")
        .and_then(Value::as_array)
        .is_some_and(|classes| {
            classes.iter().any(|class| {
                matches!(
                    class.as_str(),
                    Some("candidate.output" | "candidate.artifact")
                )
            })
        });
    if carries_assessed_output {
        Ok(())
    } else {
        Err(PublicStagePayloadError::MissingAssessedOutputClass { field })
    }
}

pub(super) fn reject_case_target_material(
    value: &Value,
    field: &'static str,
) -> Result<(), PublicStagePayloadError> {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                if contains_case_target_marker(key) {
                    return Err(PublicStagePayloadError::TargetLeakage { field });
                }
                reject_case_target_material(nested, field)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for nested in values {
                reject_case_target_material(nested, field)?;
            }
            Ok(())
        }
        Value::String(text) if contains_case_target_marker(text) => {
            Err(PublicStagePayloadError::TargetLeakage { field })
        }
        _ => Ok(()),
    }
}

pub(super) fn non_empty(
    value: String,
    field: &'static str,
) -> Result<String, PublicStagePayloadError> {
    if value.trim().is_empty() {
        Err(PublicStagePayloadError::EmptyField { field })
    } else {
        Ok(value)
    }
}

fn contains_case_target_marker(text: &str) -> bool {
    text.to_ascii_lowercase().contains("case.target")
}
