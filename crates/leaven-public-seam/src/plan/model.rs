use serde_json::Value;

use crate::PublicSeamError;

use super::PlanDocument;
use super::assessment::{
    AssessmentScoreOutputUsage, PlanAssessmentPreferenceValue, PlanAssessmentRankingValue,
    PlanAssessmentTargetValue, PlanScoreOutputValue,
};
use super::expression::{PlanExpression, PlanExpressionKind};
use super::parse::{invalid_plan, nested_kind, required_object_string};

impl PlanDocument {
    pub const fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }

    /// Locked Plan IR schema version.
    pub const fn schema_version(&self) -> PlanSchemaVersion {
        self.schema_version
    }

    /// Core operation family in document order.
    pub fn operation_kinds(&self) -> &[PlanOperationKind] {
        &self.operation_kinds
    }

    /// Typed operation metadata in document order.
    pub fn operations(&self) -> &[PlanOperation] {
        &self.operations
    }

    /// Return binding names in document order.
    pub fn return_names(&self) -> &[String] {
        &self.return_names
    }

    /// Typed return bindings in document order.
    pub fn return_bindings(&self) -> &[PlanReturnBinding] {
        &self.return_bindings
    }

    /// Consistency mode discriminator.
    pub fn consistency_kind(&self) -> &str {
        &self.consistency_kind
    }

    /// Pinned graph revision for `at_revision` consistency.
    pub fn at_revision(&self) -> Option<&str> {
        self.at_revision.as_deref()
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

    /// Number of RFC 6901 JSON Pointer values semantically validated in the document.
    pub fn pinned_pointer_count(&self) -> usize {
        self.pinned_pointer_count
    }

    /// Number of Leaven-subset `JSONPath` values semantically validated in the document.
    pub fn pinned_jsonpath_count(&self) -> usize {
        self.pinned_jsonpath_count
    }

    /// Number of strict Mustache templates semantically validated in the document.
    pub fn strict_template_count(&self) -> usize {
        self.strict_template_count
    }

    /// Number of assessment `Score.output` values semantically validated.
    pub fn assessment_score_output_count(&self) -> usize {
        self.assessment_score_outputs.total()
    }

    /// Number of assessment evidence envelopes semantically validated.
    pub fn assessment_evidence_count(&self) -> usize {
        self.assessment_score_outputs.evidence_envelopes
    }

    /// Number of independent assessment `Score.output` values semantically validated.
    pub fn independent_assessment_score_output_count(&self) -> usize {
        self.assessment_score_outputs.independent
    }

    /// Number of pairwise assessment `Score.output` values semantically validated.
    pub fn pairwise_assessment_score_output_count(&self) -> usize {
        self.assessment_score_outputs.pairwise
    }

    /// Number of listwise assessment `Score.output` values semantically validated.
    pub fn listwise_assessment_score_output_count(&self) -> usize {
        self.assessment_score_outputs.listwise
    }

    /// Whether this plan is a finite event diff through `consistency.since_revision`.
    pub fn is_since_revision_event_diff(&self) -> bool {
        self.consistency_kind == "since_revision"
            && self.since_revision.is_some()
            && self.events_since_revision_queries > 0
    }

    /// Evaluation mode discriminator.
    pub fn mode_kind(&self) -> &str {
        self.mode.as_str()
    }

    /// Evaluation mode.
    pub const fn mode(&self) -> PlanMode {
        self.mode
    }

    /// Commit policy discriminator.
    pub fn commit_kind(&self) -> &str {
        self.commit.as_str()
    }

    /// Commit policy.
    pub const fn commit(&self) -> PlanCommitKind {
        self.commit
    }
}

/// Stable Plan IR identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanId(String);

impl PlanId {
    pub(super) fn parse(value: &str) -> Result<Self, PublicSeamError> {
        if value.trim().is_empty() {
            return Err(invalid_plan("plan_id must not be empty"));
        }
        Ok(Self(value.to_owned()))
    }

    /// String form carried on the wire.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Locked Plan IR schema version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanSchemaVersion {
    /// `leaven.plan.v1`.
    V1,
}

impl PlanSchemaVersion {
    pub(super) fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "leaven.plan.v1" => Ok(Self::V1),
            other => Err(invalid_plan(format!(
                "unknown Plan IR schema_version `{other}`"
            ))),
        }
    }

    /// String form carried on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "leaven.plan.v1",
        }
    }
}

/// Plan execution mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanMode {
    /// Execute live effects.
    Execute,
    /// Validate and return a no-effect result.
    DryRun,
    /// Resolve effect outputs from cache/replayable material only.
    RequireCached,
    /// Rebuild the result from supplied receipts.
    Replay,
}

impl PlanMode {
    pub(super) fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "execute" => Ok(Self::Execute),
            "dry_run" => Ok(Self::DryRun),
            "require_cached" => Ok(Self::RequireCached),
            "replay" => Ok(Self::Replay),
            other => Err(invalid_plan(format!("unknown plan mode `{other}`"))),
        }
    }

    /// String form carried on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Execute => "execute",
            Self::DryRun => "dry_run",
            Self::RequireCached => "require_cached",
            Self::Replay => "replay",
        }
    }
}

/// Plan graph commit policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanCommitKind {
    /// No graph writes are allowed.
    NoGraphWrites,
    /// Graph writes commit atomically or fail together.
    GraphWritesAtomic,
    /// Graph writes may commit sequentially.
    GraphWritesSequential,
}

impl PlanCommitKind {
    pub(super) fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "no_graph_writes" => Ok(Self::NoGraphWrites),
            "graph_writes_atomic" => Ok(Self::GraphWritesAtomic),
            "graph_writes_sequential" => Ok(Self::GraphWritesSequential),
            other => Err(invalid_plan(format!("unknown plan commit kind `{other}`"))),
        }
    }

    /// String form carried on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoGraphWrites => "no_graph_writes",
            Self::GraphWritesAtomic => "graph_writes_atomic",
            Self::GraphWritesSequential => "graph_writes_sequential",
        }
    }
}

/// Binding requested by a Plan IR `return` entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanReturnBinding(String);

impl PlanReturnBinding {
    pub(super) fn new(value: String) -> Self {
        Self(value)
    }

    /// Binding name carried on the wire.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Typed metadata for one top-level Plan IR operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanOperation {
    name: String,
    pub(super) kind: PlanOperationKind,
    pub(super) detail: PlanOperationDetail,
}

impl PlanOperation {
    pub(super) fn from_schema_valid_object(
        object: &serde_json::Map<String, Value>,
    ) -> Result<Self, PublicSeamError> {
        let name = required_object_string(object, "name")?.to_owned();
        let kind = PlanOperationKind::parse(required_object_string(object, "kind")?)?;
        let detail = match kind {
            PlanOperationKind::Let => {
                let expr = object
                    .get("expr")
                    .ok_or_else(|| invalid_plan("let op is missing `expr`"))?;
                PlanOperationDetail::Let {
                    expression: PlanExpression::from_schema_valid_value(expr)?,
                }
            }
            PlanOperationKind::Call => {
                let call = object
                    .get("call")
                    .ok_or_else(|| invalid_plan("call op is missing `call`"))?;
                PlanOperationDetail::Call {
                    call_kind: PlanCallKind::parse(nested_kind(Some(call), "call")?)?,
                }
            }
            PlanOperationKind::Write => {
                let write = object
                    .get("write")
                    .ok_or_else(|| invalid_plan("write op is missing `write`"))?;
                PlanOperationDetail::Write {
                    write: PlanWriteOperation::from_schema_valid_value(write)?,
                }
            }
        };
        Ok(Self { name, kind, detail })
    }

    /// Operation binding name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Top-level operation family.
    pub const fn kind(&self) -> PlanOperationKind {
        self.kind
    }

    /// Call kind for `call` operations.
    pub const fn call_kind(&self) -> Option<PlanCallKind> {
        match &self.detail {
            PlanOperationDetail::Call { call_kind } => Some(*call_kind),
            _ => None,
        }
    }

    /// Write kind for `write` operations.
    pub const fn write_kind(&self) -> Option<PlanWriteKind> {
        match &self.detail {
            PlanOperationDetail::Write { write } => Some(write.kind),
            _ => None,
        }
    }

    /// Typed write details for `write` operations.
    pub const fn write(&self) -> Option<&PlanWriteOperation> {
        match &self.detail {
            PlanOperationDetail::Write { write } => Some(write),
            _ => None,
        }
    }

    /// Direct query expression kind for `let` operations.
    pub const fn query_kind(&self) -> Option<PlanQueryKind> {
        match &self.detail {
            PlanOperationDetail::Let { expression } => expression.query_kind(),
            _ => None,
        }
    }

    /// Top-level expression kind for `let` operations.
    pub const fn expression_kind(&self) -> Option<PlanExpressionKind> {
        match &self.detail {
            PlanOperationDetail::Let { expression } => Some(expression.kind()),
            _ => None,
        }
    }

    /// Typed expression details for `let` operations.
    pub const fn expression(&self) -> Option<&PlanExpression> {
        match &self.detail {
            PlanOperationDetail::Let { expression } => Some(expression),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PlanOperationDetail {
    Let { expression: PlanExpression },
    Call { call_kind: PlanCallKind },
    Write { write: PlanWriteOperation },
}

/// Typed details for one Plan IR `write` operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanWriteOperation {
    kind: PlanWriteKind,
    pub(super) submit_assessments: AssessmentScoreOutputUsage,
    detail: PlanWriteDetail,
}

impl PlanWriteOperation {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let kind = PlanWriteKind::parse(nested_kind(Some(value), "write")?)?;
        let mut submit_assessments = AssessmentScoreOutputUsage::default();
        if kind == PlanWriteKind::SubmitAssessments {
            submit_assessments.inspect_submit_assessments(value)?;
        }
        let detail = match kind {
            PlanWriteKind::EmitRunEvent => PlanWriteDetail::EmitRunEvent(
                PlanEmitRunEventWrite::from_schema_valid_value(value)?,
            ),
            PlanWriteKind::RequestEvaluation => PlanWriteDetail::RequestEvaluation(
                PlanRequestEvaluationWrite::from_schema_valid_value(value)?,
            ),
            PlanWriteKind::SubmitProposalBatch
            | PlanWriteKind::SubmitAssessments
            | PlanWriteKind::ApplyProposalBatch => PlanWriteDetail::Other,
        };
        Ok(Self {
            kind,
            submit_assessments,
            detail,
        })
    }

    /// Locked Plan IR write kind.
    pub const fn kind(&self) -> PlanWriteKind {
        self.kind
    }

    /// Number of assessment `Score.output` values carried by this write.
    pub const fn assessment_score_output_count(&self) -> usize {
        self.submit_assessments.total()
    }

    /// Number of assessment evidence envelopes carried by this write.
    pub const fn assessment_evidence_count(&self) -> usize {
        self.submit_assessments.evidence_envelopes
    }

    /// Number of independent assessment outputs carried by this write.
    pub const fn independent_assessment_score_output_count(&self) -> usize {
        self.submit_assessments.independent
    }

    /// Number of pairwise assessment outputs carried by this write.
    pub const fn pairwise_assessment_score_output_count(&self) -> usize {
        self.submit_assessments.pairwise
    }

    /// Number of listwise assessment outputs carried by this write.
    pub const fn listwise_assessment_score_output_count(&self) -> usize {
        self.submit_assessments.listwise
    }

    /// Typed assessment `Score.output.value` records carried by this write.
    pub fn assessment_score_output_values(&self) -> &[PlanScoreOutputValue] {
        self.submit_assessments.output_values()
    }

    /// Typed assessment target values carried by this write.
    pub fn assessment_target_values(&self) -> &[PlanAssessmentTargetValue] {
        self.submit_assessments.target_values()
    }

    /// Typed pairwise assessment preference values carried by this write.
    pub fn assessment_preference_values(&self) -> &[PlanAssessmentPreferenceValue] {
        self.submit_assessments.preference_values()
    }

    /// Typed listwise assessment ranking values carried by this write.
    pub fn assessment_ranking_values(&self) -> &[PlanAssessmentRankingValue] {
        self.submit_assessments.ranking_values()
    }

    /// Typed event write details for `emit_run_event` writes.
    pub const fn emit_run_event(&self) -> Option<&PlanEmitRunEventWrite> {
        match &self.detail {
            PlanWriteDetail::EmitRunEvent(write) => Some(write),
            PlanWriteDetail::RequestEvaluation(_) | PlanWriteDetail::Other => None,
        }
    }

    /// Typed request details for `request_evaluation` writes.
    pub const fn request_evaluation(&self) -> Option<&PlanRequestEvaluationWrite> {
        match &self.detail {
            PlanWriteDetail::RequestEvaluation(write) => Some(write),
            PlanWriteDetail::Other => None,
            PlanWriteDetail::EmitRunEvent(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PlanWriteDetail {
    EmitRunEvent(PlanEmitRunEventWrite),
    RequestEvaluation(PlanRequestEvaluationWrite),
    Other,
}

/// Typed `request_evaluation` write facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanRequestEvaluationWrite {
    shape: PlanEvaluationShape,
    candidate_ids: Vec<String>,
    set: PlanEvaluationSetExpr,
    granularity: String,
    purpose: String,
    evaluator: Option<String>,
}

impl PlanRequestEvaluationWrite {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_plan("request_evaluation write must be an object"))?;
        let request = object
            .get("request")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_plan("request_evaluation write must carry request"))?;
        let candidate_ids = request
            .get("candidates")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_plan("request_evaluation request must carry candidates"))?
            .iter()
            .map(candidate_ref_id)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            shape: PlanEvaluationShape::parse(required_object_string(request, "shape")?)?,
            candidate_ids,
            set: PlanEvaluationSetExpr::from_schema_valid_value(
                request
                    .get("set")
                    .ok_or_else(|| invalid_plan("request_evaluation request must carry set"))?,
            )?,
            granularity: required_object_string(request, "granularity")?.to_owned(),
            purpose: required_object_string(request, "purpose")?.to_owned(),
            evaluator: request
                .get("evaluator")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        })
    }

    /// Evaluation shape carried by the write.
    pub const fn shape(&self) -> PlanEvaluationShape {
        self.shape
    }

    /// Candidate ids selected by the request.
    pub fn candidate_ids(&self) -> &[String] {
        &self.candidate_ids
    }

    /// Evaluation-set expression selected by the request.
    pub const fn set(&self) -> &PlanEvaluationSetExpr {
        &self.set
    }

    /// Granularity discriminator carried by the request.
    pub fn granularity(&self) -> &str {
        &self.granularity
    }

    /// Purpose discriminator carried by the request.
    pub fn purpose(&self) -> &str {
        &self.purpose
    }

    /// Optional evaluator id carried by the request.
    pub fn evaluator(&self) -> Option<&str> {
        self.evaluator.as_deref()
    }
}

/// Closed evaluation shapes supported by `request_evaluation`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanEvaluationShape {
    Independent,
    Pairwise,
    Listwise,
}

impl PlanEvaluationShape {
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "independent" => Ok(Self::Independent),
            "pairwise" => Ok(Self::Pairwise),
            "listwise" => Ok(Self::Listwise),
            other => Err(invalid_plan(format!(
                "unknown request_evaluation shape `{other}`"
            ))),
        }
    }

    /// Wire spelling for this shape.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Independent => "independent",
            Self::Pairwise => "pairwise",
            Self::Listwise => "listwise",
        }
    }
}

/// Typed evaluation-set expression facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanEvaluationSetExpr {
    Named {
        name: String,
    },
    Cases {
        case_ids: Vec<String>,
        requires_partition_resolution: bool,
    },
    Tagged {
        tag: String,
        requires_partition_resolution: bool,
    },
    Recent {
        limit: u64,
        requires_partition_resolution: bool,
    },
    Union {
        sets: Vec<PlanEvaluationSetExpr>,
    },
    Intersect {
        sets: Vec<PlanEvaluationSetExpr>,
    },
    Difference {
        base: Box<PlanEvaluationSetExpr>,
        subtract: Box<PlanEvaluationSetExpr>,
    },
    Sample {
        base: Box<PlanEvaluationSetExpr>,
        n: u64,
        seed: i64,
    },
    Stratified {
        base: Box<PlanEvaluationSetExpr>,
        by: String,
        per_bucket: u64,
        seed: i64,
    },
}

impl PlanEvaluationSetExpr {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_plan("request_evaluation set must be an object"))?;
        match required_object_string(object, "kind")? {
            "named" => Ok(Self::Named {
                name: required_object_string(object, "name")?.to_owned(),
            }),
            "cases" => Ok(Self::Cases {
                case_ids: required_array(object, "cases")?
                    .iter()
                    .map(case_ref_id)
                    .collect::<Result<Vec<_>, _>>()?,
                requires_partition_resolution: required_bool(
                    object,
                    "requires_partition_resolution",
                )?,
            }),
            "tagged" => Ok(Self::Tagged {
                tag: required_object_string(object, "tag")?.to_owned(),
                requires_partition_resolution: required_bool(
                    object,
                    "requires_partition_resolution",
                )?,
            }),
            "recent" => Ok(Self::Recent {
                limit: required_u64(object, "limit")?,
                requires_partition_resolution: required_bool(
                    object,
                    "requires_partition_resolution",
                )?,
            }),
            "union" => Ok(Self::Union {
                sets: evaluation_set_array(object, "sets")?,
            }),
            "intersect" => Ok(Self::Intersect {
                sets: evaluation_set_array(object, "sets")?,
            }),
            "difference" => Ok(Self::Difference {
                base: Box::new(evaluation_set_field(object, "base")?),
                subtract: Box::new(evaluation_set_field(object, "subtract")?),
            }),
            "sample" => Ok(Self::Sample {
                base: Box::new(evaluation_set_field(object, "base")?),
                n: required_u64(object, "n")?,
                seed: required_i64(object, "seed")?,
            }),
            "stratified" => Ok(Self::Stratified {
                base: Box::new(evaluation_set_field(object, "base")?),
                by: required_object_string(object, "by")?.to_owned(),
                per_bucket: required_u64(object, "per_bucket")?,
                seed: required_i64(object, "seed")?,
            }),
            other => Err(invalid_plan(format!(
                "unknown evaluation set kind `{other}`"
            ))),
        }
    }

    /// Evaluation-set expression kind.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Named { .. } => "named",
            Self::Cases { .. } => "cases",
            Self::Tagged { .. } => "tagged",
            Self::Recent { .. } => "recent",
            Self::Union { .. } => "union",
            Self::Intersect { .. } => "intersect",
            Self::Difference { .. } => "difference",
            Self::Sample { .. } => "sample",
            Self::Stratified { .. } => "stratified",
        }
    }

    /// Named-set identifier when this expression is `kind: "named"`.
    pub fn named_set(&self) -> Option<&str> {
        match self {
            Self::Named { name } => Some(name),
            Self::Cases { .. }
            | Self::Tagged { .. }
            | Self::Recent { .. }
            | Self::Union { .. }
            | Self::Intersect { .. }
            | Self::Difference { .. }
            | Self::Sample { .. }
            | Self::Stratified { .. } => None,
        }
    }
}

fn candidate_ref_id(value: &Value) -> Result<String, PublicSeamError> {
    if let Some(candidate_id) = value.as_str() {
        return Ok(candidate_id.to_owned());
    }
    value
        .as_object()
        .and_then(|object| object.get("id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| invalid_plan("request_evaluation candidate ref must carry id"))
}

fn case_ref_id(value: &Value) -> Result<String, PublicSeamError> {
    if let Some(case_id) = value.as_str() {
        return Ok(case_id.to_owned());
    }
    value
        .as_object()
        .and_then(|object| object.get("id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| invalid_plan("request_evaluation case ref must carry id"))
}

fn evaluation_set_array(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<Vec<PlanEvaluationSetExpr>, PublicSeamError> {
    required_array(object, field)?
        .iter()
        .map(PlanEvaluationSetExpr::from_schema_valid_value)
        .collect()
}

fn evaluation_set_field(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<PlanEvaluationSetExpr, PublicSeamError> {
    PlanEvaluationSetExpr::from_schema_valid_value(
        object
            .get(field)
            .ok_or_else(|| invalid_plan(format!("evaluation set must carry {field}")))?,
    )
}

fn required_array<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<&'a [Value], PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid_plan(format!("expected evaluation set array `{field}`")))
}

fn required_bool(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<bool, PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid_plan(format!("expected evaluation set bool `{field}`")))
}

fn required_u64(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<u64, PublicSeamError> {
    object.get(field).and_then(Value::as_u64).ok_or_else(|| {
        invalid_plan(format!(
            "expected evaluation set unsigned integer `{field}`"
        ))
    })
}

fn required_i64(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<i64, PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid_plan(format!("expected evaluation set integer `{field}`")))
}

/// Typed `emit_run_event` write facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanEmitRunEventWrite {
    event_kind: String,
    payload_schema: String,
    payload: PlanEventPayload,
    visibility: String,
}

impl PlanEmitRunEventWrite {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_plan("emit_run_event write must be an object"))?;
        Ok(Self {
            event_kind: required_object_string(object, "event_kind")?.to_owned(),
            payload_schema: required_object_string(object, "payload_schema")?.to_owned(),
            payload: PlanEventPayload::from_schema_valid_value(
                object
                    .get("payload")
                    .ok_or_else(|| invalid_plan("emit_run_event write must carry payload"))?,
            ),
            visibility: required_object_string(object, "visibility")?.to_owned(),
        })
    }

    /// Event kind carried by the write.
    pub fn event_kind(&self) -> &str {
        &self.event_kind
    }

    /// Schema fingerprint for the event payload.
    pub fn payload_schema(&self) -> &str {
        &self.payload_schema
    }

    /// Schema-valid event payload carried by the write.
    pub const fn payload(&self) -> &PlanEventPayload {
        &self.payload
    }

    /// Visibility class carried by the write.
    pub fn visibility(&self) -> &str {
        &self.visibility
    }
}

/// Schema-valid JSON payload carried by an `emit_run_event` write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanEventPayload(Value);

impl PlanEventPayload {
    fn from_schema_valid_value(value: &Value) -> Self {
        Self(value.clone())
    }

    /// JSON payload carried on the wire by the event write.
    pub const fn as_json(&self) -> &Value {
        &self.0
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

impl PlanOperationKind {
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "let" => Ok(Self::Let),
            "call" => Ok(Self::Call),
            "write" => Ok(Self::Write),
            "extension" => Err(invalid_plan(
                "top-level extension plan op is not part of the locked Let/Call/Write family",
            )),
            other => Err(invalid_plan(format!(
                "unknown plan operation kind `{other}`"
            ))),
        }
    }

    /// String form carried on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Let => "let",
            Self::Call => "call",
            Self::Write => "write",
        }
    }
}

/// Locked Plan IR call operation kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanCallKind {
    /// Provider-neutral LM completion call.
    LmComplete,
    /// Agent runtime call.
    AgentRun,
    /// Sandbox command call.
    SandboxExec,
    /// Workspace materialization call.
    WorkspaceMaterialize,
    /// Workspace release call.
    WorkspaceRelease,
}

impl PlanCallKind {
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "lm_complete" => Ok(Self::LmComplete),
            "agent_run" => Ok(Self::AgentRun),
            "sandbox_exec" => Ok(Self::SandboxExec),
            "workspace_materialize" => Ok(Self::WorkspaceMaterialize),
            "workspace_release" => Ok(Self::WorkspaceRelease),
            other => Err(invalid_plan(format!("unknown plan call kind `{other}`"))),
        }
    }

    /// String form carried on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LmComplete => "lm_complete",
            Self::AgentRun => "agent_run",
            Self::SandboxExec => "sandbox_exec",
            Self::WorkspaceMaterialize => "workspace_materialize",
            Self::WorkspaceRelease => "workspace_release",
        }
    }
}

/// Locked Plan IR write operation kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanWriteKind {
    /// Submit proposal candidates.
    SubmitProposalBatch,
    /// Submit assessment records.
    SubmitAssessments,
    /// Request evaluator execution.
    RequestEvaluation,
    /// Apply proposal candidates.
    ApplyProposalBatch,
    /// Emit a run event.
    EmitRunEvent,
}

impl PlanWriteKind {
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "submit_proposal_batch" => Ok(Self::SubmitProposalBatch),
            "submit_assessments" => Ok(Self::SubmitAssessments),
            "request_evaluation" => Ok(Self::RequestEvaluation),
            "apply_proposal_batch" => Ok(Self::ApplyProposalBatch),
            "emit_run_event" => Ok(Self::EmitRunEvent),
            other => Err(invalid_plan(format!("unknown plan write kind `{other}`"))),
        }
    }

    /// String form carried on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SubmitProposalBatch => "submit_proposal_batch",
            Self::SubmitAssessments => "submit_assessments",
            Self::RequestEvaluation => "request_evaluation",
            Self::ApplyProposalBatch => "apply_proposal_batch",
            Self::EmitRunEvent => "emit_run_event",
        }
    }
}

/// Direct query expression kind for top-level `let` operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanQueryKind {
    /// Graph query expression.
    GraphQuery,
    /// Case query expression.
    CaseQuery,
    /// Workspace query expression.
    WorkspaceQuery,
}

impl PlanQueryKind {
    /// String form carried on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GraphQuery => "graph_query",
            Self::CaseQuery => "case_query",
            Self::WorkspaceQuery => "workspace_query",
        }
    }
}
