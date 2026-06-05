use serde_json::Value;

use crate::PlanGraphQuerySource;

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
#[derive(Clone, Debug)]
pub struct PlanGraphQueryRequest<'a> {
    pub(in crate::plan_execution) name: &'a str,
    pub(in crate::plan_execution) plan_id: &'a str,
    pub(in crate::plan_execution) expr: &'a Value,
    pub(in crate::plan_execution) source: PlanGraphQuerySource,
    pub(in crate::plan_execution) scope: PlanGraphReadScope<'a>,
}

impl<'a> PlanGraphQueryRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Plan id that owns this graph read.
    pub const fn plan_id(&self) -> &'a str {
        self.plan_id
    }

    /// Typed `graph_query` expression body from the Plan IR.
    pub const fn expr(&self) -> &'a Value {
        self.expr
    }

    /// Typed graph-query source facts from the Plan IR.
    pub const fn source(&self) -> &PlanGraphQuerySource {
        &self.source
    }

    /// Consistency-derived graph read scope.
    pub const fn scope(&self) -> PlanGraphReadScope<'a> {
        self.scope
    }
}

/// Host outcome for a typed `graph_query` read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanGraphQueryOutcome {
    pub(in crate::plan_execution) items: Vec<Value>,
    pub(in crate::plan_execution) graph_revision: String,
    pub(in crate::plan_execution) data_classes: Vec<String>,
    pub(in crate::plan_execution) next_cursor: Option<String>,
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
