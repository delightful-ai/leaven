use serde_json::Value;

use crate::evidence::EvidenceEnvelopeDocument;
use crate::{PinnedDialectEvaluator, PublicSeamError};

/// Schema-valid public-seam Plan IR document classified by core operation family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanDocument {
    plan_id: PlanId,
    schema_version: PlanSchemaVersion,
    operation_kinds: Vec<PlanOperationKind>,
    operations: Vec<PlanOperation>,
    return_names: Vec<String>,
    return_bindings: Vec<PlanReturnBinding>,
    consistency_kind: String,
    at_revision: Option<String>,
    since_revision: Option<String>,
    until_revision: Option<String>,
    events_since_revision_queries: usize,
    pinned_pointer_count: usize,
    pinned_jsonpath_count: usize,
    strict_template_count: usize,
    assessment_score_outputs: AssessmentScoreOutputUsage,
    mode: PlanMode,
    commit: PlanCommitKind,
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
        let plan_id = PlanId::parse(required_object_string(object, "plan_id")?)?;
        let schema_version =
            PlanSchemaVersion::parse(required_object_string(object, "schema_version")?)?;
        let mut operation_kinds = Vec::with_capacity(ops.len());
        let mut operations = Vec::with_capacity(ops.len());
        let consistency = object
            .get("consistency")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_plan("plan `consistency` must carry a kind"))?;
        let consistency_kind = nested_kind(object.get("consistency"), "consistency")?.to_owned();
        let at_revision = consistency
            .get("revision")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let since_revision = consistency
            .get("since")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let until_revision = consistency
            .get("until")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let mut events_since_revision_queries = 0;
        let mut dialect_usage = DialectUsage::default();
        let mut assessment_score_outputs = AssessmentScoreOutputUsage::default();
        for op in ops {
            dialect_usage.inspect_value(op)?;
            let op_object = op
                .as_object()
                .ok_or_else(|| invalid_plan("plan op must be an object"))?;
            let operation = PlanOperation::from_schema_valid_object(op_object)?;
            let operation_kind = operation.kind;
            match operation.detail {
                PlanOperationDetail::Let { .. } => {
                    if let Some(expr) = op.as_object().and_then(|object| object.get("expr")) {
                        validate_events_revision_sources(
                            expr,
                            &consistency_kind,
                            since_revision.as_deref(),
                            until_revision.as_deref(),
                        )?;
                        events_since_revision_queries += count_events_since_revision_queries(
                            expr,
                            since_revision.as_deref(),
                            until_revision.as_deref(),
                        );
                    }
                }
                PlanOperationDetail::Call { .. } => {}
                PlanOperationDetail::Write { write } => {
                    assessment_score_outputs.merge(write.submit_assessments);
                }
            };
            operation_kinds.push(operation_kind);
            operations.push(operation);
        }
        let return_names = string_array(object.get("return"), "return")?;

        Ok(Self {
            plan_id,
            schema_version,
            operation_kinds,
            operations,
            return_bindings: return_names
                .iter()
                .map(|name| PlanReturnBinding(name.clone()))
                .collect(),
            return_names,
            consistency_kind,
            at_revision,
            since_revision,
            until_revision,
            events_since_revision_queries,
            pinned_pointer_count: dialect_usage.pointers,
            pinned_jsonpath_count: dialect_usage.jsonpaths,
            strict_template_count: dialect_usage.templates,
            assessment_score_outputs,
            mode: PlanMode::parse(nested_kind(object.get("mode"), "mode")?)?,
            commit: PlanCommitKind::parse(nested_kind(object.get("commit"), "commit")?)?,
        })
    }

    /// Stable Plan IR identifier.
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
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
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
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
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
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
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
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
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
    /// Binding name carried on the wire.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Typed metadata for one top-level Plan IR operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanOperation {
    name: String,
    kind: PlanOperationKind,
    detail: PlanOperationDetail,
}

impl PlanOperation {
    fn from_schema_valid_object(
        object: &serde_json::Map<String, Value>,
    ) -> Result<Self, PublicSeamError> {
        let name = required_object_string(object, "name")?.to_owned();
        let kind = PlanOperationKind::parse(required_object_string(object, "kind")?)?;
        let detail = match kind {
            PlanOperationKind::Let => PlanOperationDetail::Let {
                query_kind: object
                    .get("expr")
                    .and_then(|expr| nested_kind(Some(expr), "expr").ok())
                    .and_then(PlanQueryKind::parse),
            },
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
        match self.detail {
            PlanOperationDetail::Call { call_kind } => Some(call_kind),
            _ => None,
        }
    }

    /// Write kind for `write` operations.
    pub const fn write_kind(&self) -> Option<PlanWriteKind> {
        match self.detail {
            PlanOperationDetail::Write { write } => Some(write.kind),
            _ => None,
        }
    }

    /// Typed write details for `write` operations.
    pub const fn write(&self) -> Option<PlanWriteOperation> {
        match self.detail {
            PlanOperationDetail::Write { write } => Some(write),
            _ => None,
        }
    }

    /// Direct query expression kind for `let` operations.
    pub const fn query_kind(&self) -> Option<PlanQueryKind> {
        match self.detail {
            PlanOperationDetail::Let { query_kind } => query_kind,
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlanOperationDetail {
    Let { query_kind: Option<PlanQueryKind> },
    Call { call_kind: PlanCallKind },
    Write { write: PlanWriteOperation },
}

/// Typed details for one Plan IR `write` operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanWriteOperation {
    kind: PlanWriteKind,
    submit_assessments: AssessmentScoreOutputUsage,
}

impl PlanWriteOperation {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let kind = PlanWriteKind::parse(nested_kind(Some(value), "write")?)?;
        let mut submit_assessments = AssessmentScoreOutputUsage::default();
        if kind == PlanWriteKind::SubmitAssessments {
            submit_assessments.inspect_submit_assessments(value)?;
        }
        Ok(Self {
            kind,
            submit_assessments,
        })
    }

    /// Locked Plan IR write kind.
    pub const fn kind(self) -> PlanWriteKind {
        self.kind
    }

    /// Number of assessment `Score.output` values carried by this write.
    pub const fn assessment_score_output_count(self) -> usize {
        self.submit_assessments.total()
    }

    /// Number of assessment evidence envelopes carried by this write.
    pub const fn assessment_evidence_count(self) -> usize {
        self.submit_assessments.evidence_envelopes
    }

    /// Number of independent assessment outputs carried by this write.
    pub const fn independent_assessment_score_output_count(self) -> usize {
        self.submit_assessments.independent
    }

    /// Number of pairwise assessment outputs carried by this write.
    pub const fn pairwise_assessment_score_output_count(self) -> usize {
        self.submit_assessments.pairwise
    }

    /// Number of listwise assessment outputs carried by this write.
    pub const fn listwise_assessment_score_output_count(self) -> usize {
        self.submit_assessments.listwise
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
    fn parse(value: &str) -> Option<Self> {
        match value {
            "graph_query" => Some(Self::GraphQuery),
            "case_query" => Some(Self::CaseQuery),
            "workspace_query" => Some(Self::WorkspaceQuery),
            _ => None,
        }
    }

    /// String form carried on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GraphQuery => "graph_query",
            Self::CaseQuery => "case_query",
            Self::WorkspaceQuery => "workspace_query",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct AssessmentScoreOutputUsage {
    independent: usize,
    pairwise: usize,
    listwise: usize,
    evidence_envelopes: usize,
}

impl AssessmentScoreOutputUsage {
    const fn merge(&mut self, other: Self) {
        self.independent += other.independent;
        self.pairwise += other.pairwise;
        self.listwise += other.listwise;
        self.evidence_envelopes += other.evidence_envelopes;
    }

    fn inspect_submit_assessments(&mut self, write: &Value) -> Result<(), PublicSeamError> {
        let assessments = write
            .as_object()
            .and_then(|object| object.get("assessments"))
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_plan("submit_assessments write must carry assessments"))?;
        for assessment in assessments {
            let object = assessment
                .as_object()
                .ok_or_else(|| invalid_plan("submit_assessments entries must be objects"))?;
            let kind = object
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_plan("submit_assessments entries must carry kind"))?;
            let output = object
                .get("score")
                .and_then(Value::as_object)
                .and_then(|score| score.get("output"))
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    invalid_plan("submit_assessments score must carry a reportable output")
                })?;
            validate_assessment_evidence(object)?;
            validate_assessment_candidates(kind, object)?;
            validate_score_output(kind, object, output)?;
            match kind {
                "independent" => self.independent += 1,
                "pairwise" => self.pairwise += 1,
                "listwise" => self.listwise += 1,
                other => return Err(invalid_plan(format!("unknown assessment kind `{other}`"))),
            }
            self.evidence_envelopes += 1;
        }
        Ok(())
    }

    const fn total(&self) -> usize {
        self.independent + self.pairwise + self.listwise
    }
}

fn validate_assessment_evidence(
    assessment: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let evidence = assessment
        .get("evidence")
        .ok_or_else(|| invalid_plan("submit_assessments assessment must carry evidence"))?;
    EvidenceEnvelopeDocument::from_schema_valid_value(evidence).map_err(|source| {
        invalid_plan(format!(
            "submit_assessments evidence must satisfy EvidenceEnvelope semantics: {source}"
        ))
    })?;
    Ok(())
}

fn validate_score_output(
    assessment_kind: &str,
    assessment: &serde_json::Map<String, Value>,
    output: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let data_classes = output
        .get("data_classes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("submit_assessments Score.output must carry data_classes"))?;
    let carries_assessed_output = data_classes.iter().any(|class| {
        matches!(
            class.as_str(),
            Some("candidate.output" | "candidate.artifact")
        )
    });
    if !carries_assessed_output {
        return Err(invalid_plan(
            "submit_assessments Score.output must carry candidate.output or candidate.artifact data class",
        ));
    }
    validate_score_output_candidate_binding(assessment_kind, assessment, output)?;
    let summary = output
        .get("summary")
        .and_then(Value::as_str)
        .filter(|summary| !summary.trim().is_empty());
    if let Some(summary) = summary {
        validate_score_output_evidence_projection(assessment, summary)?;
        return Ok(());
    }
    match output.get("value") {
        Some(Value::Null) => {
            return Err(invalid_plan(
                "submit_assessments Score.output value must not be null",
            ));
        }
        Some(Value::String(text)) if text.trim().is_empty() => {}
        Some(Value::String(text)) => {
            validate_score_output_evidence_projection(assessment, text)?;
            return Ok(());
        }
        Some(_) => {
            return Err(invalid_plan(
                "submit_assessments Score.output must carry a non-empty summary for structured output projection",
            ));
        }
        None => {}
    }
    if output.get("blob_ref").is_some()
        || output
            .get("trace_refs")
            .and_then(Value::as_array)
            .is_some_and(|trace_refs| !trace_refs.is_empty())
    {
        return Err(invalid_plan(
            "submit_assessments Score.output blob or trace output must carry a public evidence summary projection",
        ));
    }
    Err(invalid_plan(
        "submit_assessments Score.output must carry reportable output content",
    ))
}

fn validate_assessment_candidates(
    kind: &str,
    assessment: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    match kind {
        "independent" => {
            candidate_string(assessment, "candidate")?;
        }
        "pairwise" | "listwise" => {
            candidate_array(assessment, "candidates")?;
        }
        _ => {}
    }
    Ok(())
}

fn validate_score_output_candidate_binding(
    assessment_kind: &str,
    assessment: &serde_json::Map<String, Value>,
    output: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let Some(value) = output.get("value") else {
        if has_score_output_external_projection(output) {
            return Ok(());
        }
        return Err(invalid_plan(
            "submit_assessments Score.output must carry candidate-bound value or blob/trace output projection",
        ));
    };
    match assessment_kind {
        "independent" => {
            let candidate = candidate_string(assessment, "candidate")?;
            validate_candidate_output_entry(value, candidate)
        }
        "pairwise" | "listwise" => {
            let candidates = candidate_array(assessment, "candidates")?;
            let entries = value.as_array().ok_or_else(|| {
                invalid_plan(
                    "submit_assessments pairwise/listwise Score.output value must be candidate entries",
                )
            })?;
            if entries.len() != candidates.len() {
                return Err(invalid_plan(
                    "submit_assessments Score.output candidate entries must match assessed candidates",
                ));
            }
            for (entry, candidate) in entries.iter().zip(candidates) {
                validate_candidate_output_entry(entry, candidate)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn has_score_output_external_projection(output: &serde_json::Map<String, Value>) -> bool {
    output.get("blob_ref").is_some()
        || output
            .get("trace_refs")
            .and_then(Value::as_array)
            .is_some_and(|trace_refs| !trace_refs.is_empty())
}

fn validate_candidate_output_entry(value: &Value, candidate: &str) -> Result<(), PublicSeamError> {
    let entry = value.as_object().ok_or_else(|| {
        invalid_plan("submit_assessments Score.output value must be a candidate-bound object")
    })?;
    if entry.get("candidate").and_then(Value::as_str) != Some(candidate) {
        return Err(invalid_plan(
            "submit_assessments Score.output candidate binding must match assessed candidate",
        ));
    }
    let carries_output = entry
        .get("output")
        .or_else(|| entry.get("artifact"))
        .is_some_and(has_reportable_content);
    if carries_output {
        Ok(())
    } else {
        Err(invalid_plan(
            "submit_assessments Score.output candidate binding must carry output or artifact content",
        ))
    }
}

fn has_reportable_content(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(object) => !object.is_empty(),
        Value::Bool(_) | Value::Number(_) => true,
    }
}

fn candidate_string<'a>(
    assessment: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, PublicSeamError> {
    assessment
        .get(field)
        .and_then(Value::as_str)
        .filter(|candidate| !candidate.trim().is_empty())
        .ok_or_else(|| {
            invalid_plan(format!(
                "submit_assessments assessment must carry non-empty `{field}`"
            ))
        })
}

fn candidate_array<'a>(
    assessment: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<Vec<&'a str>, PublicSeamError> {
    let values = assessment
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid_plan(format!(
                "submit_assessments assessment must carry `{field}`"
            ))
        })?;
    if values.is_empty() {
        return Err(invalid_plan(format!(
            "submit_assessments assessment `{field}` must not be empty"
        )));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|candidate| !candidate.trim().is_empty())
                .ok_or_else(|| {
                    invalid_plan(format!(
                        "submit_assessments assessment `{field}` entries must be non-empty strings"
                    ))
                })
        })
        .collect()
}

fn validate_score_output_evidence_projection(
    assessment: &serde_json::Map<String, Value>,
    expected_summary: &str,
) -> Result<(), PublicSeamError> {
    let evidence_summary = assessment
        .get("evidence")
        .and_then(Value::as_object)
        .and_then(|evidence| evidence.get("public"))
        .and_then(Value::as_object)
        .and_then(|public| public.get("summary"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            invalid_plan(
                "submit_assessments Score.output must be projected by evidence.public.summary",
            )
        })?;
    if evidence_summary == expected_summary {
        Ok(())
    } else {
        Err(invalid_plan(
            "submit_assessments Score.output must match evidence.public.summary",
        ))
    }
}

#[derive(Default)]
struct DialectUsage {
    pointers: usize,
    jsonpaths: usize,
    templates: usize,
    evaluator: PinnedDialectEvaluator,
}

impl DialectUsage {
    fn inspect_value(&mut self, value: &Value) -> Result<(), PublicSeamError> {
        match value {
            Value::Object(object) => self.inspect_object(object),
            Value::Array(values) => {
                for value in values {
                    self.inspect_value(value)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn inspect_object(
        &mut self,
        object: &serde_json::Map<String, Value>,
    ) -> Result<(), PublicSeamError> {
        if let Some(pointer) = object.get("field").and_then(Value::as_str) {
            self.validate_pointer(pointer)?;
        }
        if object.get("kind").and_then(Value::as_str) == Some("stratified") {
            if let Some(pointer) = object.get("by").and_then(Value::as_str) {
                self.validate_pointer(pointer)?;
            }
        }
        if let Some(fields) = object.get("fields").and_then(Value::as_array) {
            for pointer in fields.iter().filter_map(Value::as_str) {
                self.validate_pointer(pointer)?;
            }
        }
        if object.get("kind").and_then(Value::as_str) == Some("extract") {
            if let Some(path) = object.get("path").and_then(Value::as_str) {
                self.validate_jsonpath(path)?;
            }
        }
        if object.get("kind").and_then(Value::as_str) == Some("template") {
            let dialect = object
                .get("dialect")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_plan("template expression must carry a dialect"))?;
            let template = object
                .get("template")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_plan("template expression must carry a template"))?;
            self.evaluator
                .render_template(dialect, template, &serde_json::json!({}))?;
            self.templates += 1;
        }
        for (key, value) in object {
            if object.get("kind").and_then(Value::as_str) == Some("events") && key == "filter" {
                continue;
            }
            if object.get("kind").and_then(Value::as_str) == Some("schema_valid") && key == "value"
            {
                self.inspect_value(value)?;
                continue;
            }
            if is_arbitrary_json_slot(key) {
                continue;
            }
            self.inspect_value(value)?;
        }
        Ok(())
    }

    fn validate_pointer(&mut self, pointer: &str) -> Result<(), PublicSeamError> {
        match self
            .evaluator
            .resolve_json_pointer(&serde_json::json!({}), pointer)
        {
            Ok(_) => {}
            Err(PublicSeamError::InvalidDialect { message })
                if message.contains("was not present")
                    || message.contains("out of bounds")
                    || message.contains("cannot descend") => {}
            Err(error) => return Err(error),
        }
        self.pointers += 1;
        Ok(())
    }

    fn validate_jsonpath(&mut self, path: &str) -> Result<(), PublicSeamError> {
        self.evaluator
            .extract_json_path(&serde_json::json!({}), path)?;
        self.jsonpaths += 1;
        Ok(())
    }
}

fn is_arbitrary_json_slot(key: &str) -> bool {
    matches!(
        key,
        "value"
            | "values"
            | "payload"
            | "scope"
            | "selector"
            | "provider_hints"
            | "schema"
            | "input_schema"
            | "metadata"
            | "rubric"
            | "causal"
            | "target"
            | "preference"
            | "ranking"
    )
}

fn nested_kind<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .and_then(Value::as_object)
        .and_then(|object| object.get("kind"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("plan `{field}` must carry a kind")))
}

fn required_object_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("plan object must carry string `{field}`")))
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

fn validate_events_revision_sources(
    value: &Value,
    consistency_kind: &str,
    since_revision: Option<&str>,
    until_revision: Option<&str>,
) -> Result<(), PublicSeamError> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    match object.get("kind").and_then(Value::as_str) {
        Some("graph_query") => validate_graph_query_revision_source(
            object,
            consistency_kind,
            since_revision,
            until_revision,
        ),
        Some("project" | "filter") => object
            .get("input")
            .map(|input| {
                validate_events_revision_sources(
                    input,
                    consistency_kind,
                    since_revision,
                    until_revision,
                )
            })
            .unwrap_or(Ok(())),
        _ => Ok(()),
    }
}

fn validate_graph_query_revision_source(
    object: &serde_json::Map<String, Value>,
    consistency_kind: &str,
    since_revision: Option<&str>,
    until_revision: Option<&str>,
) -> Result<(), PublicSeamError> {
    let Some(source) = object.get("source").and_then(Value::as_object) else {
        return Ok(());
    };
    if source.get("kind").and_then(Value::as_str) != Some("events") {
        return Ok(());
    }
    if consistency_kind != "since_revision" {
        return Ok(());
    }
    let Some(since_revision) = since_revision else {
        return Err(invalid_plan(
            "since_revision event queries must carry a plan base revision",
        ));
    };
    if source.get("since_revision").and_then(Value::as_str) != Some(since_revision) {
        return Err(invalid_plan(
            "events source since_revision must match plan consistency base",
        ));
    }
    if let Some(until_revision) = until_revision {
        if source.get("until_revision").and_then(Value::as_str) != Some(until_revision) {
            return Err(invalid_plan(
                "events source until_revision must match plan consistency bound",
            ));
        }
    }
    Ok(())
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
