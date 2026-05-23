use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::PublicSeamError;

use super::invalid_plan;

/// Lowered graph-read consistency scope for a Plan IR `graph_query`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanGraphReadScope<'a> {
    /// Read from the graph revision captured when plan execution started.
    LatestAtStart { revision: &'a str },
    /// Read from an explicitly pinned graph revision.
    AtRevision { revision: &'a str },
    /// Read a finite graph-event diff over the declared revision interval.
    SinceRevision {
        since: &'a str,
        until: Option<&'a str>,
    },
}

/// Lowered `graph_query` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanGraphQueryRequest<'a> {
    pub(super) name: &'a str,
    pub(super) expr: &'a Value,
    pub(super) scope: PlanGraphReadScope<'a>,
}

impl<'a> PlanGraphQueryRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `graph_query` expression body from the Plan IR.
    pub const fn expr(&self) -> &'a Value {
        self.expr
    }

    /// Consistency-derived graph read scope.
    pub const fn scope(&self) -> PlanGraphReadScope<'a> {
        self.scope
    }
}

/// Host outcome for a typed `graph_query` read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanGraphQueryOutcome {
    pub(super) items: Vec<Value>,
    pub(super) graph_revision: String,
    pub(super) data_classes: Vec<String>,
    pub(super) next_cursor: Option<String>,
}

impl PlanGraphQueryOutcome {
    /// Creates a graph-set outcome for a pure graph read.
    pub fn new(items: impl IntoIterator<Item = Value>, graph_revision: impl Into<String>) -> Self {
        Self {
            items: items.into_iter().collect(),
            graph_revision: graph_revision.into(),
            data_classes: vec!["public".to_owned()],
            next_cursor: None,
        }
    }

    /// Overrides the data classes carried by the graph-set value.
    #[must_use]
    pub fn with_data_classes(mut self, data_classes: impl IntoIterator<Item = String>) -> Self {
        self.data_classes = data_classes.into_iter().collect();
        self
    }

    /// Adds the next cursor returned by the graph read.
    #[must_use]
    pub fn with_next_cursor(mut self, next_cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(next_cursor.into());
        self
    }
}

/// Lowered `case_query.load` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanCaseQueryRequest<'a> {
    pub(super) name: &'a str,
    pub(super) query: &'a Value,
}

impl<'a> PlanCaseQueryRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `case_query.load` body from the Plan IR.
    pub const fn query(&self) -> &'a Value {
        self.query
    }
}

/// Host outcome for a typed `case_query.load` read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanCaseQueryOutcome {
    pub(super) case: String,
    pub(super) graph_revision: String,
    pub(super) data_classes: Vec<String>,
    pub(super) input: Option<Value>,
    pub(super) target: Option<Value>,
    pub(super) metadata: Option<Value>,
}

impl PlanCaseQueryOutcome {
    /// Creates a loaded case outcome.
    pub fn new(case: impl Into<String>, graph_revision: impl Into<String>) -> Self {
        Self {
            case: case.into(),
            graph_revision: graph_revision.into(),
            data_classes: vec!["public".to_owned()],
            input: None,
            target: None,
            metadata: None,
        }
    }

    /// Overrides the data classes carried by the case record.
    #[must_use]
    pub fn with_data_classes(mut self, data_classes: impl IntoIterator<Item = String>) -> Self {
        self.data_classes = data_classes.into_iter().collect();
        self
    }

    /// Adds case input to the loaded record.
    #[must_use]
    pub fn with_input(mut self, input: Value) -> Self {
        self.input = Some(input);
        self
    }

    /// Adds case target to the loaded record.
    #[must_use]
    pub fn with_target(mut self, target: Value) -> Self {
        self.target = Some(target);
        self
    }

    /// Adds case metadata to the loaded record.
    #[must_use]
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

pub(super) fn case_query_include(query: &Value) -> Result<BTreeSet<&str>, PublicSeamError> {
    let include = query
        .get("include")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("case_query.load must carry include"))?;
    let mut fields = BTreeSet::new();
    for field in include {
        let field = field
            .as_str()
            .ok_or_else(|| invalid_plan("case_query.load include entries must be strings"))?;
        if !matches!(field, "input" | "target" | "metadata") {
            return Err(invalid_plan(format!(
                "case_query.load include field `{field}` is not supported"
            )));
        }
        fields.insert(field);
    }
    Ok(fields)
}

pub(super) fn require_requested_case_field(
    include: &BTreeSet<&str>,
    field: &'static str,
) -> Result<(), PublicSeamError> {
    if include.contains(field) {
        Ok(())
    } else {
        Err(invalid_plan(format!(
            "case_query.load host returned unrequested `{field}` material"
        )))
    }
}

pub(super) fn require_included_case_fields(
    value: &Value,
    include: &BTreeSet<&str>,
) -> Result<(), PublicSeamError> {
    for field in include {
        if value.get(*field).is_none() {
            return Err(invalid_plan(format!(
                "case_query.load host omitted requested `{field}` material"
            )));
        }
    }
    Ok(())
}

pub(super) fn case_query_projection(query: &Value) -> Result<Value, PublicSeamError> {
    Ok(json!({
        "case": query
            .get("case")
            .cloned()
            .ok_or_else(|| invalid_plan("case_query.load must carry case"))?,
        "include": query
            .get("include")
            .cloned()
            .ok_or_else(|| invalid_plan("case_query.load must carry include"))?,
        "projection_schema": query.get("projection_schema").cloned().unwrap_or(Value::Null)
    }))
}
