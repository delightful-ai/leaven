use serde_json::Value;

use crate::PublicSeamError;

pub(super) fn nested_kind<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<&'a str, PublicSeamError> {
    value
        .and_then(Value::as_object)
        .and_then(|object| object.get("kind"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("plan `{field}` must carry a kind")))
}

pub(super) fn required_object_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("plan object must carry string `{field}`")))
}

pub(super) fn string_array(
    value: Option<&Value>,
    field: &str,
) -> Result<Vec<String>, PublicSeamError> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan(format!("plan `{field}` must be an array")))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid_plan(format!("plan `{field}` entries must be strings")))
        })
        .collect()
}

pub(super) fn invalid_plan(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}
