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
    GraphQuery {
        source: PlanGraphQuerySource,
        artifact_selectors: Vec<PlanArtifactProjectionSelector>,
        cost_scopes: Vec<PlanCostScope>,
    },
    /// Case query expression.
    CaseQuery,
    /// Workspace query expression.
    WorkspaceQuery { workspace: WorkspaceRefExpression },
    /// Projection over another expression.
    Project {
        input: Box<Self>,
        artifact_selectors: Vec<PlanArtifactProjectionSelector>,
        cost_scopes: Vec<PlanCostScope>,
    },
    /// Predicate filter over another expression.
    Filter {
        input: Box<Self>,
        artifact_selectors: Vec<PlanArtifactProjectionSelector>,
        cost_scopes: Vec<PlanCostScope>,
    },
    /// Sort over another expression.
    Sort {
        input: Box<Self>,
        artifact_selectors: Vec<PlanArtifactProjectionSelector>,
        cost_scopes: Vec<PlanCostScope>,
    },
    /// Limit over another expression.
    Limit {
        input: Box<Self>,
        limit: u64,
        artifact_selectors: Vec<PlanArtifactProjectionSelector>,
        cost_scopes: Vec<PlanCostScope>,
    },
    /// Strict-template expression with typed variable expression dependencies.
    Template {
        vars: BTreeMap<String, Self>,
        artifact_selectors: Vec<PlanArtifactProjectionSelector>,
        cost_scopes: Vec<PlanCostScope>,
    },
    /// `JSONPath` extraction over another expression.
    Extract {
        input: Box<Self>,
        artifact_selectors: Vec<PlanArtifactProjectionSelector>,
        cost_scopes: Vec<PlanCostScope>,
    },
    /// Reference extraction from a prior Plan Result binding.
    RefsFromResult { from: String },
    /// Locked extension object expression.
    Extension {
        namespace: String,
        operation: String,
        schema_fingerprint: String,
        payload: PlanExtensionPayload,
    },
}

impl PlanExpression {
    #[allow(
        clippy::too_many_lines,
        reason = "keeps schema discriminant parsing in one audited owner"
    )]
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
                    artifact_selectors: graph_query_artifact_selectors(object),
                    cost_scopes: cost_scopes_from_graph_source(source)?,
                })
            }
            "case_query" => Ok(Self::CaseQuery),
            "workspace_query" => Ok(Self::WorkspaceQuery {
                workspace: WorkspaceRefExpression::from_value(object.get("workspace"))?,
            }),
            "project" => {
                let input = required_input_expression(object, "project")?;
                Ok(Self::Project {
                    artifact_selectors: merge_artifact_selectors(
                        &input,
                        project_artifact_selectors(object),
                    ),
                    cost_scopes: input.cost_scopes().to_vec(),
                    input: Box::new(input),
                })
            }
            "filter" => {
                let input = required_input_expression(object, "filter")?;
                Ok(Self::Filter {
                    artifact_selectors: input.artifact_selectors().to_vec(),
                    cost_scopes: input.cost_scopes().to_vec(),
                    input: Box::new(input),
                })
            }
            "sort" => {
                let input = required_input_expression(object, "sort")?;
                Ok(Self::Sort {
                    artifact_selectors: input.artifact_selectors().to_vec(),
                    cost_scopes: input.cost_scopes().to_vec(),
                    input: Box::new(input),
                })
            }
            "limit" => {
                let input = required_input_expression(object, "limit")?;
                Ok(Self::Limit {
                    artifact_selectors: input.artifact_selectors().to_vec(),
                    cost_scopes: input.cost_scopes().to_vec(),
                    input: Box::new(input),
                    limit: object
                        .get("limit")
                        .and_then(Value::as_u64)
                        .ok_or_else(|| invalid_plan("limit expr must carry an integer limit"))?,
                })
            }
            "template" => {
                let vars = template_vars(object)?;
                Ok(Self::Template {
                    artifact_selectors: vars
                        .values()
                        .flat_map(|expr| expr.artifact_selectors().iter().cloned())
                        .collect(),
                    cost_scopes: vars
                        .values()
                        .flat_map(|expr| expr.cost_scopes().iter().cloned())
                        .collect(),
                    vars,
                })
            }
            "extract" => {
                let input = required_input_expression(object, "extract")?;
                Ok(Self::Extract {
                    artifact_selectors: input.artifact_selectors().to_vec(),
                    cost_scopes: input.cost_scopes().to_vec(),
                    input: Box::new(input),
                })
            }
            "refs_from_result" => Ok(Self::RefsFromResult {
                from: required_object_string(object, "from")?.to_owned(),
            }),
            "extension" => Ok(Self::Extension {
                namespace: required_object_string(object, "namespace")?.to_owned(),
                operation: required_object_string(object, "op")?.to_owned(),
                schema_fingerprint: required_object_string(object, "schema_fingerprint")?
                    .to_owned(),
                payload: PlanExtensionPayload::from_schema_valid_value(
                    object
                        .get("payload")
                        .ok_or_else(|| invalid_plan("extension expr must carry payload"))?,
                ),
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
            Self::GraphQuery { source, .. } => {
                usize::from(source.matches_since_revision(since_revision, until_revision))
            }
            Self::Project { input, .. }
            | Self::Filter { input, .. }
            | Self::Sort { input, .. }
            | Self::Limit { input, .. }
            | Self::Extract { input, .. } => {
                input.event_query_count(since_revision, until_revision)
            }
            Self::Template { vars, .. } => vars
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
            Self::GraphQuery { source, .. } => source.validate_for_plan_consistency(
                consistency_kind,
                since_revision,
                until_revision,
            ),
            Self::Project { input, .. }
            | Self::Filter { input, .. }
            | Self::Sort { input, .. }
            | Self::Limit { input, .. }
            | Self::Extract { input, .. } => {
                input.validate_event_sources(consistency_kind, since_revision, until_revision)
            }
            Self::Template { vars, .. } => vars.values().try_for_each(|expr| {
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

    /// Artifact projection selector fragments carried by this expression tree.
    pub fn artifact_selectors(&self) -> &[PlanArtifactProjectionSelector] {
        match self {
            Self::GraphQuery {
                artifact_selectors, ..
            }
            | Self::Project {
                artifact_selectors, ..
            }
            | Self::Filter {
                artifact_selectors, ..
            }
            | Self::Sort {
                artifact_selectors, ..
            }
            | Self::Limit {
                artifact_selectors, ..
            }
            | Self::Template {
                artifact_selectors, ..
            }
            | Self::Extract {
                artifact_selectors, ..
            } => artifact_selectors,
            Self::Literal { .. }
            | Self::Var { .. }
            | Self::CaseQuery
            | Self::WorkspaceQuery { .. }
            | Self::RefsFromResult { .. }
            | Self::Extension { .. } => &[],
        }
    }

    /// Cost graph-query scope fragments carried by this expression tree.
    pub fn cost_scopes(&self) -> &[PlanCostScope] {
        match self {
            Self::GraphQuery { cost_scopes, .. }
            | Self::Project { cost_scopes, .. }
            | Self::Filter { cost_scopes, .. }
            | Self::Sort { cost_scopes, .. }
            | Self::Limit { cost_scopes, .. }
            | Self::Template { cost_scopes, .. }
            | Self::Extract { cost_scopes, .. } => cost_scopes,
            Self::Literal { .. }
            | Self::Var { .. }
            | Self::CaseQuery
            | Self::WorkspaceQuery { .. }
            | Self::RefsFromResult { .. }
            | Self::Extension { .. } => &[],
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

/// Schema-valid JSON payload carried by a Plan IR extension expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanExtensionPayload(Value);

impl PlanExtensionPayload {
    fn from_schema_valid_value(value: &Value) -> Self {
        Self(value.clone())
    }

    /// JSON payload carried on the wire by the extension expression.
    pub const fn as_json(&self) -> &Value {
        &self.0
    }
}

/// Schema-valid JSON selector carried by an artifact projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanArtifactProjectionSelector(Value);

impl PlanArtifactProjectionSelector {
    fn from_schema_valid_value(value: &Value) -> Self {
        Self(value.clone())
    }

    /// JSON selector carried on the wire by the artifact projection.
    pub const fn as_json(&self) -> &Value {
        &self.0
    }
}

/// Schema-valid JSON scope carried by a graph cost query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanCostScope(Value);

impl PlanCostScope {
    fn from_schema_valid_value(value: &Value) -> Self {
        Self(value.clone())
    }

    /// JSON scope carried on the wire by the cost query.
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
        filter: Option<PlanGraphEventFilter>,
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
                filter: object.get("filter").map(PlanGraphEventFilter::from_value),
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
            ..
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
            ..
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

    /// Returns true when this source selects RunContext-owned service events.
    pub fn selects_run_context_events(&self) -> bool {
        matches!(
            self,
            Self::Events {
                filter: Some(PlanGraphEventFilter::RunContext),
                ..
            }
        )
    }
}

/// Typed event-filter facts carried by a graph event query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanGraphEventFilter {
    /// Service-local `RunContext` event summary filter.
    RunContext,
    /// Schema-valid event filter not interpreted by this owner.
    Other(PlanGraphEventFilterPayload),
}

impl PlanGraphEventFilter {
    fn from_value(value: &Value) -> Self {
        if value
            .as_object()
            .and_then(|object| object.get("kind"))
            .and_then(Value::as_str)
            == Some("run_context")
        {
            Self::RunContext
        } else {
            Self::Other(PlanGraphEventFilterPayload::from_schema_valid_value(value))
        }
    }
}

/// Opaque schema-valid event-filter payload for filters not owned by V1 Rust.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanGraphEventFilterPayload(Value);

impl PlanGraphEventFilterPayload {
    fn from_schema_valid_value(value: &Value) -> Self {
        Self(value.clone())
    }

    /// JSON payload carried by an event filter outside the `RunContext` filter.
    pub const fn as_json(&self) -> &Value {
        &self.0
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

fn merge_artifact_selectors(
    input: &PlanExpression,
    mut local: Vec<PlanArtifactProjectionSelector>,
) -> Vec<PlanArtifactProjectionSelector> {
    let mut values = input.artifact_selectors().to_vec();
    values.append(&mut local);
    values
}

fn graph_query_artifact_selectors(
    object: &Map<String, Value>,
) -> Vec<PlanArtifactProjectionSelector> {
    let mut selectors = Vec::new();
    if let Some(projection) = object.get("projection") {
        collect_projection_artifact_selectors(projection, &mut selectors);
    }
    if let Some(steps) = object.get("steps").and_then(Value::as_array) {
        for step in steps {
            collect_graph_step_artifact_selectors(step, &mut selectors);
        }
    }
    selectors
}

fn project_artifact_selectors(object: &Map<String, Value>) -> Vec<PlanArtifactProjectionSelector> {
    let mut selectors = Vec::new();
    if let Some(projection) = object.get("projection") {
        collect_projection_artifact_selectors(projection, &mut selectors);
    }
    selectors
}

fn collect_graph_step_artifact_selectors(
    value: &Value,
    selectors: &mut Vec<PlanArtifactProjectionSelector>,
) {
    let Some(object) = value.as_object() else {
        return;
    };
    if object.get("kind").and_then(Value::as_str) == Some("project") {
        if let Some(projection) = object.get("projection") {
            collect_projection_artifact_selectors(projection, selectors);
        }
    }
}

fn collect_projection_artifact_selectors(
    value: &Value,
    selectors: &mut Vec<PlanArtifactProjectionSelector>,
) {
    let Some(object) = value.as_object() else {
        return;
    };
    if let Some("candidate_projection" | "artifact_projection") =
        object.get("kind").and_then(Value::as_str)
        && let Some(artifact) = object.get("artifact")
    {
        collect_artifact_projection_selector(artifact, selectors);
    }
}

fn collect_artifact_projection_selector(
    value: &Value,
    selectors: &mut Vec<PlanArtifactProjectionSelector>,
) {
    let Some(object) = value.as_object() else {
        return;
    };
    if let Some(selector) = object.get("selector") {
        selectors.push(PlanArtifactProjectionSelector::from_schema_valid_value(
            selector,
        ));
    }
}

fn cost_scopes_from_graph_source(value: &Value) -> Result<Vec<PlanCostScope>, PublicSeamError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_plan("graph_query source must be an object"))?;
    if object.get("kind").and_then(Value::as_str) != Some("costs") {
        return Ok(Vec::new());
    }
    let scope = object
        .get("scope")
        .ok_or_else(|| invalid_plan("costs graph source must carry scope"))?;
    Ok(vec![PlanCostScope::from_schema_valid_value(scope)])
}
