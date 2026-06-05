use serde::Serialize;
use serde_json::Value;

use crate::run_bound_service::RunBoundGraphEffectError;

#[derive(Serialize)]
pub(super) struct EmptyObject {}

#[derive(Serialize)]
pub(super) struct WriteResultPreimage<'a, T> {
    pub(super) schema_version: &'static str,
    pub(super) name: &'a str,
    pub(super) value: &'a T,
}

pub(super) fn prefixed_jcs_hash(
    prefix: &str,
    value: &(impl Serialize + ?Sized),
) -> Result<String, RunBoundGraphEffectError> {
    let value = serde_json::to_value(value)
        .map_err(|error| RunBoundGraphEffectError::Hash(error.to_string()))?;
    let digest = jcs_canonicalize::sha256_jcs_hex(&value)
        .map_err(|error| RunBoundGraphEffectError::Hash(error.to_string()))?;
    Ok(format!("{prefix}{digest}"))
}

pub(super) fn to_value(value: impl Serialize) -> Result<Value, RunBoundGraphEffectError> {
    serde_json::to_value(value).map_err(|error| RunBoundGraphEffectError::Hash(error.to_string()))
}

pub(super) fn required_pointer<'a>(
    value: &'a Value,
    pointer: &'static str,
    missing: RunBoundGraphEffectError,
) -> Result<&'a Value, RunBoundGraphEffectError> {
    value.pointer(pointer).ok_or(missing)
}

pub(super) fn collect_write_receipts<T>(
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

pub(super) fn empty_redactions(
    plan_result: &Value,
) -> Result<Vec<EmptyObject>, RunBoundGraphEffectError> {
    match plan_result.get("redactions").and_then(Value::as_array) {
        Some(redactions) if redactions.is_empty() => Ok(Vec::new()),
        None => Ok(Vec::new()),
        Some(_) => Err(RunBoundGraphEffectError::InvalidProjection {
            field: "redactions",
            reason: "run-bound graph callbacks do not yet own typed redaction projection",
        }),
    }
}

pub(super) fn optional_string(value: &Value, field: &'static str, default: &'static str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_owned()
}

pub(super) fn required_string<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<&'a str, RunBoundGraphEffectError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(RunBoundGraphEffectError::MissingString { field })
}

pub(super) fn required_array<'a>(
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

pub(super) fn string_array(
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

pub(super) fn require_kind(
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

pub(super) fn require_field(
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
