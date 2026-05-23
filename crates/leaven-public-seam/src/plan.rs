use serde_json::Value;

use crate::PublicSeamError;

/// Schema-valid public-seam Plan IR document classified by core operation family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanDocument {
    operation_kinds: Vec<PlanOperationKind>,
    return_names: Vec<String>,
    consistency_kind: String,
    mode_kind: String,
    commit_kind: String,
}

impl PlanDocument {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_plan("plan must be an object"))?;
        let ops = object
            .get("ops")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_plan("plan ops must be an array"))?;
        let mut operation_kinds = Vec::with_capacity(ops.len());
        for op in ops {
            let kind = required_string(op, "kind")?;
            let operation_kind = match kind {
                "let" => PlanOperationKind::Let,
                "call" => {
                    ensure_nested_kind(op, "call", "call")?;
                    PlanOperationKind::Call
                }
                "write" => {
                    ensure_nested_kind(op, "write", "write")?;
                    PlanOperationKind::Write
                }
                "extension" => {
                    return Err(invalid_plan(
                        "top-level extension plan op is not part of the locked Let/Call/Write family",
                    ));
                }
                other => {
                    return Err(invalid_plan(format!(
                        "unknown plan operation kind `{other}`"
                    )));
                }
            };
            operation_kinds.push(operation_kind);
        }

        Ok(Self {
            operation_kinds,
            return_names: string_array(object.get("return"), "return")?,
            consistency_kind: nested_kind(object.get("consistency"), "consistency")?.to_owned(),
            mode_kind: nested_kind(object.get("mode"), "mode")?.to_owned(),
            commit_kind: nested_kind(object.get("commit"), "commit")?.to_owned(),
        })
    }

    /// Core operation family in document order.
    pub fn operation_kinds(&self) -> &[PlanOperationKind] {
        &self.operation_kinds
    }

    /// Return binding names in document order.
    pub fn return_names(&self) -> &[String] {
        &self.return_names
    }

    /// Consistency mode discriminator.
    pub fn consistency_kind(&self) -> &str {
        &self.consistency_kind
    }

    /// Evaluation mode discriminator.
    pub fn mode_kind(&self) -> &str {
        &self.mode_kind
    }

    /// Commit policy discriminator.
    pub fn commit_kind(&self) -> &str {
        &self.commit_kind
    }
}

/// Locked Plan IR core operation family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanOperationKind {
    /// Pure value/query binding.
    Let,
    /// Effectful capability call.
    Call,
    /// Staged graph mutation intent.
    Write,
}

fn ensure_nested_kind(value: &Value, field: &str, owner: &str) -> Result<(), PublicSeamError> {
    let _ = value
        .get(field)
        .ok_or_else(|| invalid_plan(format!("{owner} op is missing `{field}`")))?
        .as_object()
        .and_then(|object| object.get("kind"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("{owner} op `{field}` must carry a typed kind")))?;
    Ok(())
}

fn nested_kind<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .and_then(Value::as_object)
        .and_then(|object| object.get("kind"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("plan `{field}` must carry a kind")))
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .as_object()
        .and_then(|object| object.get(field))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("plan op must carry string `{field}`")))
}

fn string_array(value: Option<&Value>, field: &str) -> Result<Vec<String>, PublicSeamError> {
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

fn invalid_plan(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}
