use serde_json::Value;

use crate::PublicSeamError;

use super::PlanDocument;
use super::assessment::AssessmentScoreOutputUsage;
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
            PlanWriteKind::SubmitProposalBatch
            | PlanWriteKind::SubmitAssessments
            | PlanWriteKind::RequestEvaluation
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

    /// Typed event write details for `emit_run_event` writes.
    pub const fn emit_run_event(&self) -> Option<&PlanEmitRunEventWrite> {
        match &self.detail {
            PlanWriteDetail::EmitRunEvent(write) => Some(write),
            PlanWriteDetail::Other => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PlanWriteDetail {
    EmitRunEvent(PlanEmitRunEventWrite),
    Other,
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
