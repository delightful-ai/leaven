use std::collections::BTreeSet;

use serde_json::Value;

use super::Replayability;
use crate::PublicSeamError;

pub(super) fn required_replayability(
    value: Option<&Value>,
) -> Result<Replayability, PublicSeamError> {
    let raw = required_string(value, "replayability")?;
    Replayability::parse(raw)
        .ok_or_else(|| invalid_result(format!("unknown replayability `{raw}`")))
}

pub(super) fn required_string<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<&'a str, PublicSeamError> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_result(format!("{field} must be a string")))
}

pub(super) fn required_string_set(
    value: Option<&Value>,
    field: &str,
) -> Result<BTreeSet<String>, PublicSeamError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_result(format!("{field} must be an array")))?;
    let mut set = BTreeSet::new();
    for value in values {
        let item = value
            .as_str()
            .ok_or_else(|| invalid_result(format!("{field} entries must be strings")))?;
        if !set.insert(item.to_owned()) {
            return Err(invalid_result(format!("{field} entries must be unique")));
        }
    }
    Ok(set)
}

pub(super) fn optional_string_set(
    value: Option<&Value>,
    field: &str,
) -> Result<BTreeSet<String>, PublicSeamError> {
    match value {
        Some(value) => required_string_set(Some(value), field),
        None => Ok(BTreeSet::new()),
    }
}

pub(super) fn array_len(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<usize, PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::len)
        .ok_or_else(|| invalid_result(format!("plan result {field} must be an array")))
}

pub(super) fn invalid_result(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlanResult {
        message: message.into(),
    }
}

pub(super) fn prefixed_jcs_hash(prefix: &str, value: &Value) -> Result<String, PublicSeamError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value)
        .map_err(|error| invalid_result(format!("plan result hash failed: {error}")))?;
    Ok(format!("{prefix}{digest}"))
}
