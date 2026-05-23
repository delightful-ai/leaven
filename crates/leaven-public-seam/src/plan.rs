use serde_json::Value;

use crate::PublicSeamError;

/// Schema-valid public-seam Plan IR document classified by core operation family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanDocument {
    operation_kinds: Vec<PlanOperationKind>,
    return_names: Vec<String>,
    consistency_kind: String,
    since_revision: Option<String>,
    until_revision: Option<String>,
    events_since_revision_queries: usize,
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
        let consistency = object
            .get("consistency")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_plan("plan `consistency` must carry a kind"))?;
        let consistency_kind = nested_kind(object.get("consistency"), "consistency")?.to_owned();
        let since_revision = consistency
            .get("since")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let until_revision = consistency
            .get("until")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let mut events_since_revision_queries = 0;
        for op in ops {
            let kind = required_string(op, "kind")?;
            let operation_kind = match kind {
                "let" => {
                    if let Some(expr) = op.as_object().and_then(|object| object.get("expr")) {
                        events_since_revision_queries += count_events_since_revision_queries(
                            expr,
                            since_revision.as_deref(),
                            until_revision.as_deref(),
                        );
                    }
                    PlanOperationKind::Let
                }
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
            consistency_kind,
            since_revision,
            until_revision,
            events_since_revision_queries,
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

    /// Base graph revision for `since_revision` consistency.
    pub fn since_revision(&self) -> Option<&str> {
        self.since_revision.as_deref()
    }

    /// Upper graph revision for `since_revision` consistency when bounded.
    pub fn until_revision(&self) -> Option<&str> {
        self.until_revision.as_deref()
    }

    /// Number of graph event queries bound to the plan's `since_revision` base.
    pub fn events_since_revision_queries(&self) -> usize {
        self.events_since_revision_queries
    }

    /// Whether this plan is a finite event diff through `consistency.since_revision`.
    pub fn is_since_revision_event_diff(&self) -> bool {
        self.consistency_kind == "since_revision"
            && self.since_revision.is_some()
            && self.events_since_revision_queries > 0
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

fn count_events_since_revision_queries(
    value: &Value,
    since_revision: Option<&str>,
    until_revision: Option<&str>,
) -> usize {
    let Some(object) = value.as_object() else {
        return 0;
    };
    match object.get("kind").and_then(Value::as_str) {
        Some("graph_query") => usize::from(graph_query_matches_since_revision(
            object,
            since_revision,
            until_revision,
        )),
        Some("project" | "filter") => object
            .get("input")
            .map(|input| count_events_since_revision_queries(input, since_revision, until_revision))
            .unwrap_or(0),
        _ => 0,
    }
}

fn graph_query_matches_since_revision(
    object: &serde_json::Map<String, Value>,
    since_revision: Option<&str>,
    until_revision: Option<&str>,
) -> bool {
    let Some(source) = object.get("source").and_then(Value::as_object) else {
        return false;
    };
    if source.get("kind").and_then(Value::as_str) != Some("events") {
        return false;
    }
    let Some(since_revision) = since_revision else {
        return false;
    };
    if source.get("since_revision").and_then(Value::as_str) != Some(since_revision) {
        return false;
    }
    match until_revision {
        Some(until_revision) => {
            source.get("until_revision").and_then(Value::as_str) == Some(until_revision)
        }
        None => true,
    }
}

fn invalid_plan(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}
