use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::PublicSeamError;

use super::invalid_plan;

/// Lowered `case_query.load` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanCaseQueryRequest<'a> {
    pub(in crate::plan_execution) name: &'a str,
    pub(in crate::plan_execution) query: &'a Value,
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
    pub(in crate::plan_execution) case: String,
    pub(in crate::plan_execution) graph_revision: String,
    pub(in crate::plan_execution) data_classes: Vec<String>,
    pub(in crate::plan_execution) input: Option<Value>,
    pub(in crate::plan_execution) target: Option<Value>,
    pub(in crate::plan_execution) metadata: Option<Value>,
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

pub(in crate::plan_execution) fn case_query_include(
    query: &Value,
) -> Result<BTreeSet<&str>, PublicSeamError> {
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

pub(in crate::plan_execution) fn require_requested_case_field(
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

pub(in crate::plan_execution) fn require_included_case_fields(
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

pub(in crate::plan_execution) fn case_query_projection(
    query: &Value,
) -> Result<Value, PublicSeamError> {
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
