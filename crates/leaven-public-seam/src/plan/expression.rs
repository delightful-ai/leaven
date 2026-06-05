use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::PublicSeamError;

use super::model::PlanQueryKind;
use super::parse::{invalid_plan, nested_kind, required_object_string, string_array};

/// Typed Plan IR expression shape for top-level `let` bindings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanExpression {
    /// Literal value expression with its schema-valid JSON value preserved.
    Literal {
        value: PlanLiteralValue,
        data_classes: Vec<String>,
    },
    /// Existing binding reference.
    Var { name: String },
    /// Graph query expression with typed revision-source facts.
    GraphQuery { source: PlanGraphQuerySource },
    /// Case query expression.
    CaseQuery,
    /// Workspace query expression.
    WorkspaceQuery { workspace: WorkspaceRefExpression },
    /// Projection over another expression.
    Project { input: Box<PlanExpression> },
    /// Predicate filter over another expression.
    Filter { input: Box<PlanExpression> },
    /// Sort over another expression.
    Sort { input: Box<PlanExpression> },
    /// Limit over another expression.
    Limit {
        input: Box<PlanExpression>,
        limit: u64,
    },
    /// Strict-template expression with typed variable expression dependencies.
    Template {
        vars: BTreeMap<String, PlanExpression>,
    },
    /// JSONPath extraction over another expression.
    Extract { input: Box<PlanExpression> },
    /// Reference extraction from a prior Plan Result binding.
    RefsFromResult { from: String },
    /// Locked extension object expression.
    Extension {
        namespace: String,
        operation: String,
        schema_fingerprint: String,
    },
}

impl PlanExpression {
    pub(super) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_plan("plan expr must be an object"))?;
        match nested_kind(Some(value), "expr")? {
            "literal" => Ok(Self::Literal {
                value: PlanLiteralValue::from_schema_valid_value(
                    object
                        .get("value")
                        .ok_or_else(|| invalid_plan("literal expr must carry value"))?,
                ),
                data_classes: string_array(object.get("data_classes"), "expr.data_classes")
                    .unwrap_or_default(),
            }),
            "var" => Ok(Self::Var {
                name: required_object_string(object, "name")?.to_owned(),
            }),
            "graph_query" => {
                let source = object
                    .get("source")
                    .ok_or_else(|| invalid_plan("graph_query expr must carry source"))?;
                Ok(Self::GraphQuery {
                    source: PlanGraphQuerySource::from_schema_valid_value(source)?,
                })
            }
            "case_query" => Ok(Self::CaseQuery),
            "workspace_query" => Ok(Self::WorkspaceQuery {
                workspace: WorkspaceRefExpression::from_value(object.get("workspace"))?,
            }),
            "project" => Ok(Self::Project {
                input: Box::new(required_input_expression(object, "project")?),
            }),
            "filter" => Ok(Self::Filter {
                input: Box::new(required_input_expression(object, "filter")?),
            }),
            "sort" => Ok(Self::Sort {
                input: Box::new(required_input_expression(object, "sort")?),
            }),
            "limit" => Ok(Self::Limit {
                input: Box::new(required_input_expression(object, "limit")?),
                limit: object
                    .get("limit")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| invalid_plan("limit expr must carry an integer limit"))?,
            }),
            "template" => Ok(Self::Template {
                vars: template_vars(object)?,
            }),
            "extract" => Ok(Self::Extract {
                input: Box::new(required_input_expression(object, "extract")?),
            }),
            "refs_from_result" => Ok(Self::RefsFromResult {
                from: required_object_string(object, "from")?.to_owned(),
            }),
            "extension" => Ok(Self::Extension {
                namespace: required_object_string(object, "namespace")?.to_owned(),
                operation: required_object_string(object, "op")?.to_owned(),
                schema_fingerprint: required_object_string(object, "schema_fingerprint")?
                    .to_owned(),
            }),
            other => Err(invalid_plan(format!(
                "unknown plan expression kind `{other}`"
            ))),
        }
    }

    pub(super) const fn query_kind(&self) -> Option<PlanQueryKind> {
        match self {
            Self::GraphQuery { .. } => Some(PlanQueryKind::GraphQuery),
            Self::CaseQuery => Some(PlanQueryKind::CaseQuery),
            Self::WorkspaceQuery { .. } => Some(PlanQueryKind::WorkspaceQuery),
            _ => None,
        }
    }

    pub(super) const fn kind(&self) -> PlanExpressionKind {
        match self {
            Self::Literal { .. } => PlanExpressionKind::Literal,
            Self::Var { .. } => PlanExpressionKind::Var,
            Self::GraphQuery { .. } => PlanExpressionKind::GraphQuery,
            Self::CaseQuery => PlanExpressionKind::CaseQuery,
            Self::WorkspaceQuery { .. } => PlanExpressionKind::WorkspaceQuery,
            Self::Project { .. } => PlanExpressionKind::Project,
            Self::Filter { .. } => PlanExpressionKind::Filter,
            Self::Sort { .. } => PlanExpressionKind::Sort,
            Self::Limit { .. } => PlanExpressionKind::Limit,
            Self::Template { .. } => PlanExpressionKind::Template,
            Self::Extract { .. } => PlanExpressionKind::Extract,
            Self::RefsFromResult { .. } => PlanExpressionKind::RefsFromResult,
            Self::Extension { .. } => PlanExpressionKind::Extension,
        }
    }

    pub(super) fn event_query_count(
        &self,
        since_revision: Option<&str>,
        until_revision: Option<&str>,
    ) -> usize {
        match self {
            Self::GraphQuery { source } => {
                usize::from(source.matches_since_revision(since_revision, until_revision))
            }
            Self::Project { input }
            | Self::Filter { input }
            | Self::Sort { input }
            | Self::Limit { input, .. }
            | Self::Extract { input } => input.event_query_count(since_revision, until_revision),
            Self::Template { vars } => vars
                .values()
                .map(|expr| expr.event_query_count(since_revision, until_revision))
                .sum(),
            Self::Literal { .. }
            | Self::Var { .. }
            | Self::CaseQuery
            | Self::WorkspaceQuery { .. }
            | Self::RefsFromResult { .. }
            | Self::Extension { .. } => 0,
        }
    }

    pub(super) fn validate_event_sources(
        &self,
        consistency_kind: &str,
        since_revision: Option<&str>,
        until_revision: Option<&str>,
    ) -> Result<(), PublicSeamError> {
        match self {
            Self::GraphQuery { source } => source.validate_for_plan_consistency(
                consistency_kind,
                since_revision,
                until_revision,
            ),
            Self::Project { input }
            | Self::Filter { input }
            | Self::Sort { input }
            | Self::Limit { input, .. }
            | Self::Extract { input } => {
                input.validate_event_sources(consistency_kind, since_revision, until_revision)
            }
            Self::Template { vars } => vars.values().try_for_each(|expr| {
                expr.validate_event_sources(consistency_kind, since_revision, until_revision)
            }),
            Self::Literal { .. }
            | Self::Var { .. }
            | Self::CaseQuery
            | Self::WorkspaceQuery { .. }
            | Self::RefsFromResult { .. }
            | Self::Extension { .. } => Ok(()),
        }
    }
}

/// Typed workspace reference carried by workspace-shaped Plan expressions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceRefExpression {
    id: String,
    run: Option<String>,
    snapshot_fingerprint: Option<String>,
}

impl WorkspaceRefExpression {
    fn from_value(value: Option<&Value>) -> Result<Self, PublicSeamError> {
        let value = value.ok_or_else(|| invalid_plan("workspace_query must carry workspace"))?;
        if let Some(id) = value.as_str() {
            return Ok(Self {
                id: id.to_owned(),
                run: None,
                snapshot_fingerprint: None,
            });
        }
        let object = value
            .as_object()
            .ok_or_else(|| invalid_plan("workspace_query workspace must be string or object"))?;
        if object.get("kind").and_then(Value::as_str) != Some("workspace") {
            return Err(invalid_plan(
                "workspace_query workspace object must have kind `workspace`",
            ));
        }
        Ok(Self {
            id: required_workspace_ref_string(object, "id")?.to_owned(),
            run: optional_workspace_ref_string(object, "run")?,
            snapshot_fingerprint: optional_workspace_ref_string(object, "snapshot_fingerprint")?,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn run(&self) -> Option<&str> {
        self.run.as_deref()
    }

    pub fn snapshot_fingerprint(&self) -> Option<&str> {
        self.snapshot_fingerprint.as_deref()
    }
}

fn required_workspace_ref_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("workspace_query workspace must carry `{field}`")))
}

fn optional_workspace_ref_string(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, PublicSeamError> {
    object
        .get(field)
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                invalid_plan(format!(
                    "workspace_query workspace `{field}` must be a string"
                ))
            })
        })
        .transpose()
}

/// Schema-valid JSON value carried by a Plan IR literal expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanLiteralValue(Value);

impl PlanLiteralValue {
    fn from_schema_valid_value(value: &Value) -> Self {
        Self(value.clone())
    }

    /// JSON value carried on the wire by the literal expression.
    pub const fn as_json(&self) -> &Value {
        &self.0
    }
}

/// Plan IR expression discriminator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanExpressionKind {
    Literal,
    Var,
    GraphQuery,
    CaseQuery,
    WorkspaceQuery,
    Project,
    Filter,
    Sort,
    Limit,
    Template,
    Extract,
    RefsFromResult,
    Extension,
}

/// Typed graph-query source facts needed for revision-bound Plan validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanGraphQuerySource {
    ByCandidate,
    CandidateSet,
    ByProposal,
    ByProposalBatch,
    AssessmentSet,
    RecentFailures,
    Costs,
    Events {
        since_revision: Option<String>,
        until_revision: Option<String>,
    },
    CandidateTree,
    Extension,
}

impl PlanGraphQuerySource {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_plan("graph_query source must be an object"))?;
        match nested_kind(Some(value), "graph_query.source")? {
            "by_candidate" => Ok(Self::ByCandidate),
            "candidate_set" => Ok(Self::CandidateSet),
            "by_proposal" => Ok(Self::ByProposal),
            "by_proposal_batch" => Ok(Self::ByProposalBatch),
            "assessment_set" => Ok(Self::AssessmentSet),
            "recent_failures" => Ok(Self::RecentFailures),
            "costs" => Ok(Self::Costs),
            "events" => Ok(Self::Events {
                since_revision: object
                    .get("since_revision")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                until_revision: object
                    .get("until_revision")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            }),
            "candidate_tree" => Ok(Self::CandidateTree),
            "extension" => Ok(Self::Extension),
            other => Err(invalid_plan(format!(
                "unknown graph_query source kind `{other}`"
            ))),
        }
    }

    fn validate_for_plan_consistency(
        &self,
        consistency_kind: &str,
        since_revision: Option<&str>,
        until_revision: Option<&str>,
    ) -> Result<(), PublicSeamError> {
        let Self::Events {
            since_revision: source_since,
            until_revision: source_until,
        } = self
        else {
            return Ok(());
        };
        if consistency_kind != "since_revision" {
            return Ok(());
        }
        let Some(since_revision) = since_revision else {
            return Err(invalid_plan(
                "since_revision event queries must carry a plan base revision",
            ));
        };
        if source_since.as_deref() != Some(since_revision) {
            return Err(invalid_plan(
                "events source since_revision must match plan consistency base",
            ));
        }
        if let Some(until_revision) = until_revision {
            if source_until.as_deref() != Some(until_revision) {
                return Err(invalid_plan(
                    "events source until_revision must match plan consistency bound",
                ));
            }
        }
        Ok(())
    }

    fn matches_since_revision(
        &self,
        since_revision: Option<&str>,
        until_revision: Option<&str>,
    ) -> bool {
        let Self::Events {
            since_revision: source_since,
            until_revision: source_until,
        } = self
        else {
            return false;
        };
        let Some(since_revision) = since_revision else {
            return false;
        };
        if source_since.as_deref() != Some(since_revision) {
            return false;
        }
        match until_revision {
            Some(until_revision) => source_until.as_deref() == Some(until_revision),
            None => true,
        }
    }
}

impl PlanExpressionKind {
    /// String form carried by the Plan IR `kind` field.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Literal => "literal",
            Self::Var => "var",
            Self::GraphQuery => "graph_query",
            Self::CaseQuery => "case_query",
            Self::WorkspaceQuery => "workspace_query",
            Self::Project => "project",
            Self::Filter => "filter",
            Self::Sort => "sort",
            Self::Limit => "limit",
            Self::Template => "template",
            Self::Extract => "extract",
            Self::RefsFromResult => "refs_from_result",
            Self::Extension => "extension",
        }
    }
}

fn required_input_expression(
    object: &serde_json::Map<String, Value>,
    kind: &str,
) -> Result<PlanExpression, PublicSeamError> {
    PlanExpression::from_schema_valid_value(
        object
            .get("input")
            .ok_or_else(|| invalid_plan(format!("{kind} expr must carry input")))?,
    )
}

fn template_vars(
    object: &serde_json::Map<String, Value>,
) -> Result<BTreeMap<String, PlanExpression>, PublicSeamError> {
    object
        .get("vars")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_plan("template expr must carry vars"))?
        .iter()
        .map(|(name, value)| {
            Ok((
                name.clone(),
                PlanExpression::from_schema_valid_value(value)?,
            ))
        })
        .collect()
}
