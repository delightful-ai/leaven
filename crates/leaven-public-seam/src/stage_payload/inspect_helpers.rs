use std::collections::BTreeSet;

use serde_json::Value;

use crate::PublicSeamError;

pub(super) fn require_field(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), PublicSeamError> {
    object
        .get(field)
        .ok_or_else(|| invalid_stage_payload(format!("stage payload must carry `{field}`")))?;
    Ok(())
}

pub(super) fn require_non_empty_array(
    value: Option<&Value>,
    field: &str,
) -> Result<(), PublicSeamError> {
    if required_array(value, field)?.is_empty() {
        return Err(invalid_stage_payload(format!(
            "stage payload field `{field}` must be non-empty"
        )));
    }
    Ok(())
}

pub(super) fn reject_target_leakage(
    value: Option<&Value>,
    context: &str,
) -> Result<(), PublicSeamError> {
    let Some(value) = value else {
        return Ok(());
    };
    reject_target_leakage_value(value, context)
}

pub(super) fn required_object<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a serde_json::Map<String, Value>, PublicSeamError> {
    value
        .as_object()
        .ok_or_else(|| invalid_stage_payload(format!("{field} must be an object")))
}

pub(super) fn matching_string(
    left: &serde_json::Map<String, Value>,
    right: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<String, PublicSeamError> {
    let left = required_string(left.get(field), field)?;
    let right = required_string(right.get(field), field)?;
    if left != right {
        return Err(invalid_stage_payload(format!(
            "reflect/propose handoff field `{field}` must match"
        )));
    }
    Ok(left.to_owned())
}

pub(super) fn matching_source_ref(
    left: &serde_json::Map<String, Value>,
    right: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<String, PublicSeamError> {
    let left = source_ref_key(
        left.get(field)
            .ok_or_else(|| invalid_stage_payload(format!("missing `{field}`")))?,
    )?;
    let right = source_ref_key(
        right
            .get(field)
            .ok_or_else(|| invalid_stage_payload(format!("missing `{field}`")))?,
    )?;
    if left != right {
        return Err(invalid_stage_payload(format!(
            "reflect/propose handoff field `{field}` must match"
        )));
    }
    Ok(left)
}

pub(super) fn validate_handoff_stage_receipts(
    handoff: &Value,
    reflect_stage_call_id: &str,
    propose_stage_call_id: &str,
    reflection_result_fingerprint: &str,
) -> Result<(String, String), PublicSeamError> {
    let receipts = handoff
        .as_object()
        .and_then(|object| object.get("stage_receipts"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid_stage_payload("reflect/propose handoff must carry stage_receipts")
        })?;
    let mut reflect_receipt = None;
    let mut propose_receipt = None;
    for receipt in receipts {
        let receipt = receipt
            .as_object()
            .ok_or_else(|| invalid_stage_payload("stage_receipts entries must be objects"))?;
        if required_string(receipt.get("kind"), "stage_receipts.kind")? != "stage_receipt" {
            return Err(invalid_stage_payload(
                "stage_receipts entries must have kind `stage_receipt`",
            ));
        }
        let id = required_string(receipt.get("id"), "stage_receipts.id")?;
        if !id.starts_with("stagerec_") {
            return Err(invalid_stage_payload(
                "stage receipt ids must use the `stagerec_` prefix",
            ));
        }
        let stage_call_id =
            required_string(receipt.get("stage_call_id"), "stage_receipts.stage_call_id")?;
        let stage_role = required_string(receipt.get("stage_role"), "stage_receipts.stage_role")?;
        if stage_call_id == reflect_stage_call_id && stage_role == "reflector" {
            validate_reflect_receipt_produces(receipt, reflection_result_fingerprint)?;
            reflect_receipt = Some(id.to_owned());
        } else if stage_call_id == propose_stage_call_id && stage_role == "proposer" {
            validate_propose_receipt_consumes(receipt, reflection_result_fingerprint)?;
            propose_receipt = Some(id.to_owned());
        }
    }
    let reflect_receipt = reflect_receipt.ok_or_else(|| {
        invalid_stage_payload("reflect/propose handoff missing reflector stage receipt")
    })?;
    let propose_receipt = propose_receipt.ok_or_else(|| {
        invalid_stage_payload("reflect/propose handoff missing proposer stage receipt")
    })?;
    if reflect_receipt == propose_receipt {
        return Err(invalid_stage_payload(
            "reflect and propose stages must use distinct stage receipt ids",
        ));
    }
    let propose = receipts
        .iter()
        .filter_map(Value::as_object)
        .find(|receipt| {
            receipt.get("stage_call_id").and_then(Value::as_str) == Some(propose_stage_call_id)
                && receipt.get("stage_role").and_then(Value::as_str) == Some("proposer")
        })
        .ok_or_else(|| {
            invalid_stage_payload("reflect/propose handoff missing proposer stage receipt")
        })?;
    validate_propose_receipt_binds_reflect_receipt(
        propose,
        reflection_result_fingerprint,
        &reflect_receipt,
    )?;
    Ok((reflect_receipt, propose_receipt))
}

fn validate_reflect_receipt_produces(
    receipt: &serde_json::Map<String, Value>,
    reflection_result_fingerprint: &str,
) -> Result<(), PublicSeamError> {
    let produces = receipt
        .get("produces")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_stage_payload("reflector stage receipt must carry produces"))?;
    if required_string(produces.get("kind"), "stage_receipts.produces.kind")? != "reflection_result"
    {
        return Err(invalid_stage_payload(
            "reflector stage receipt must produce a reflection_result",
        ));
    }
    if required_string(
        produces.get("fingerprint"),
        "stage_receipts.produces.fingerprint",
    )? != reflection_result_fingerprint
    {
        return Err(invalid_stage_payload(
            "reflector stage receipt must fingerprint the exact ReflectionResult",
        ));
    }
    Ok(())
}

fn validate_propose_receipt_consumes(
    receipt: &serde_json::Map<String, Value>,
    reflection_result_fingerprint: &str,
) -> Result<(), PublicSeamError> {
    let consumes = receipt
        .get("consumes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_stage_payload("proposer stage receipt must carry consumes"))?;
    if consumes.is_empty() {
        return Err(invalid_stage_payload(
            "proposer stage receipt must consume the ReflectionResult",
        ));
    }
    for consume in consumes {
        let consume = consume.as_object().ok_or_else(|| {
            invalid_stage_payload("stage receipt consumes entries must be objects")
        })?;
        if consume.get("kind").and_then(Value::as_str) == Some("reflection_result")
            && consume.get("fingerprint").and_then(Value::as_str)
                == Some(reflection_result_fingerprint)
        {
            return Ok(());
        }
    }
    Err(invalid_stage_payload(
        "proposer stage receipt must consume the exact ReflectionResult fingerprint",
    ))
}

fn validate_propose_receipt_binds_reflect_receipt(
    receipt: &serde_json::Map<String, Value>,
    reflection_result_fingerprint: &str,
    reflect_receipt: &str,
) -> Result<(), PublicSeamError> {
    let consumes = receipt
        .get("consumes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_stage_payload("proposer stage receipt must carry consumes"))?;
    for consume in consumes {
        let consume = consume.as_object().ok_or_else(|| {
            invalid_stage_payload("stage receipt consumes entries must be objects")
        })?;
        if consume.get("kind").and_then(Value::as_str) == Some("reflection_result")
            && consume.get("fingerprint").and_then(Value::as_str)
                == Some(reflection_result_fingerprint)
            && consume.get("receipt").and_then(Value::as_str) == Some(reflect_receipt)
        {
            return Ok(());
        }
    }
    Err(invalid_stage_payload(
        "proposer stage receipt must cite the reflector receipt for the consumed ReflectionResult",
    ))
}

pub(super) fn reject_target_leakage_value(
    value: &Value,
    context: &str,
) -> Result<(), PublicSeamError> {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                if contains_case_target_marker(key) {
                    return Err(invalid_stage_payload(format!(
                        "{context} must not carry case.target material"
                    )));
                }
                reject_target_leakage_value(nested, context)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                reject_target_leakage_value(item, context)?;
            }
        }
        Value::String(text) if contains_case_target_marker(text) => {
            return Err(invalid_stage_payload(format!(
                "{context} must not carry case.target material"
            )));
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn contains_case_target_marker(text: &str) -> bool {
    text.to_ascii_lowercase().contains("case.target")
}

pub(super) fn prefixed_stage_payload_hash(
    prefix: &str,
    value: &Value,
) -> Result<String, PublicSeamError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value)
        .map_err(|error| invalid_stage_payload(format!("stage payload hash failed: {error}")))?;
    Ok(format!("{prefix}{digest}"))
}

pub(super) fn required_string<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<&'a str, PublicSeamError> {
    value.and_then(Value::as_str).ok_or_else(|| {
        invalid_stage_payload(format!("stage payload field `{field}` must be a string"))
    })
}

pub(super) fn optional_string(value: Option<&Value>) -> Result<Option<String>, PublicSeamError> {
    value
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid_stage_payload("optional string field is not a string"))
        })
        .transpose()
}

pub(super) fn required_array<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<&'a Vec<Value>, PublicSeamError> {
    value.and_then(Value::as_array).ok_or_else(|| {
        invalid_stage_payload(format!("stage payload field `{field}` must be an array"))
    })
}

pub(super) fn array_len(value: Option<&Value>, field: &str) -> Result<usize, PublicSeamError> {
    value.map_or(Ok(0), |value| {
        value.as_array().map(Vec::len).ok_or_else(|| {
            invalid_stage_payload(format!("stage payload field `{field}` must be an array"))
        })
    })
}

pub(super) fn string_array(
    value: Option<&Value>,
    field: &str,
) -> Result<Vec<String>, PublicSeamError> {
    value.map_or_else(
        || Ok(Vec::new()),
        |value| {
            value
                .as_array()
                .ok_or_else(|| {
                    invalid_stage_payload(format!("stage payload field `{field}` must be an array"))
                })?
                .iter()
                .map(|item| {
                    item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                        invalid_stage_payload(format!(
                            "stage payload field `{field}` must contain only strings"
                        ))
                    })
                })
                .collect()
        },
    )
}

pub(super) fn string_set(
    value: Option<&Value>,
    field: &str,
) -> Result<BTreeSet<String>, PublicSeamError> {
    string_array(value, field).map(|values| values.into_iter().collect())
}

pub(super) fn literal_expr_array_contains_string(value: &Value, needle: &str) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.get("kind").and_then(Value::as_str) != Some("literal") {
        return false;
    }
    object
        .get("value")
        .and_then(Value::as_array)
        .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(needle)))
}

pub(super) fn source_ref_set(
    value: Option<&Value>,
    field: &str,
) -> Result<BTreeSet<String>, PublicSeamError> {
    let Some(value) = value else {
        return Ok(BTreeSet::new());
    };
    let values = value.as_array().ok_or_else(|| {
        invalid_stage_payload(format!("stage payload field `{field}` must be an array"))
    })?;
    values
        .iter()
        .map(source_ref_key)
        .collect::<Result<BTreeSet<_>, _>>()
}

pub(super) fn source_ref_key(value: &Value) -> Result<String, PublicSeamError> {
    if let Some(candidate) = candidate_ref_key(value)? {
        return Ok(candidate);
    }
    jcs_canonicalize::sha256_jcs_hex(value).map_err(|error| {
        invalid_stage_payload(format!(
            "stage payload source ref is not JCS canonicalizable: {error}"
        ))
    })
}

pub(super) fn candidate_ref_key(value: &Value) -> Result<Option<String>, PublicSeamError> {
    if let Some(candidate) = value
        .as_str()
        .filter(|candidate| candidate.starts_with("cand_"))
    {
        return Ok(Some(format!("candidate:{candidate}")));
    }
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    if object.get("kind").and_then(Value::as_str) != Some("candidate") {
        return Ok(None);
    }
    let id = required_string(object.get("id"), "candidate ref id")?;
    let run = object
        .get("run")
        .and_then(Value::as_str)
        .map(|run| format!("run:{run}:"))
        .unwrap_or_default();
    Ok(Some(format!("candidate:{run}{id}")))
}

pub(super) fn receipt_ref_ids(
    value: Option<&Value>,
    field: &str,
) -> Result<Vec<String>, PublicSeamError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value.as_array().ok_or_else(|| {
        invalid_stage_payload(format!("stage payload field `{field}` must be an array"))
    })?;
    values
        .iter()
        .map(|value| receipt_ref_id(value, field))
        .collect()
}

pub(super) fn require_read_receipt_refs(
    value: Option<&Value>,
    field: &str,
) -> Result<(), PublicSeamError> {
    for receipt in receipt_ref_ids(value, field)? {
        if !is_read_receipt_id(&receipt) {
            return Err(invalid_stage_payload(format!(
                "stage payload field `{field}` must contain read receipt refs, got `{receipt}`"
            )));
        }
    }
    Ok(())
}

pub(super) fn receipt_ref_id(value: &Value, field: &str) -> Result<String, PublicSeamError> {
    if let Some(id) = value.as_str() {
        return Ok(id.to_owned());
    }
    let object = value
        .as_object()
        .ok_or_else(|| invalid_stage_payload(format!("{field} entries must be receipt refs")))?;
    if object.get("kind").and_then(Value::as_str) != Some("receipt") {
        return Err(invalid_stage_payload(format!(
            "{field} receipt ref object must have kind `receipt`"
        )));
    }
    object
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| invalid_stage_payload(format!("{field} receipt ref object must carry id")))
}

fn is_read_receipt_id(receipt: &str) -> bool {
    receipt.starts_with("qrec_")
        || receipt.starts_with("caseread_")
        || receipt.starts_with("wsread_")
}

pub(super) fn invalid_stage_payload(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidStagePayload {
        message: message.into(),
    }
}
