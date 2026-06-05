use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Number, Value};

use crate::PublicSeamError;
use crate::evidence::{EvidenceEnvelopeDocument, EvidenceReceiptRef};
use crate::plan_error::{self, PlanErrorDocument};

mod audit;
mod helpers;
mod visibility;

pub use audit::Replayability;
use audit::{
    AssessmentBatchScope, ReceiptAudit, inspect_assessment_batch_value, inspect_receipts,
    inspect_value_receipt, receipt_index, validate_failed_call_charges,
    validate_replayability_rollups, validate_result_hash_bindings,
    validate_submit_assessment_receipts,
};
use helpers::{
    array_len, invalid_result, optional_string_set, required_replayability, required_string,
};
use visibility::{collect_trace_ref_data_classes, validate_value_visibility};

/// Schema-valid public-seam Plan Result classified by replayability facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultDocument {
    plan_id: String,
    base_revision: String,
    final_revision: String,
    replayability_summary: Replayability,
    values: Vec<PlanResultValueFact>,
    receipts: Vec<PlanResultReceiptFact>,
    value_kinds: Vec<String>,
    receipt_kinds: Vec<String>,
    value_data_classes: Vec<(String, Vec<String>)>,
    errors: Vec<PlanErrorDocument>,
    charge_count: usize,
    assessment_batch_replayability: Vec<(String, Replayability)>,
    graph_row_fragments: PlanResultGraphRowFragments,
}

impl PlanResultDocument {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        Self::from_schema_valid_value_with_policy(value, RequestEvaluationReceiptPolicy::Reject)
    }

    pub(crate) fn from_schema_valid_value_allowing_request_evaluation(
        value: &Value,
    ) -> Result<Self, PublicSeamError> {
        Self::from_schema_valid_value_with_policy(
            value,
            RequestEvaluationReceiptPolicy::AllowDedicatedValidation,
        )
    }

    fn from_schema_valid_value_with_policy(
        value: &Value,
        request_evaluation_policy: RequestEvaluationReceiptPolicy,
    ) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_result("plan result must be an object"))?;
        let replayability_summary = required_replayability(object.get("replayability_summary"))?;
        let parts = PlanResultParts::from_object(object)?;
        validate_result_hash_bindings(parts.values, parts.receipts, request_evaluation_policy)?;
        let value_audit = inspect_values(
            parts.values,
            &receipt_index(parts.receipts)?,
            replayability_summary,
        )?;
        let receipt_kinds = inspect_receipts(parts.receipts)?;
        validate_submit_assessment_receipts(parts.receipts, &value_audit.assessment_batches)?;
        validate_failed_call_charges(parts.receipts, parts.charges)?;
        Ok(Self {
            plan_id: parts.plan_id.to_owned(),
            base_revision: parts.base_revision.to_owned(),
            final_revision: parts.final_revision.to_owned(),
            replayability_summary,
            values: value_audit.values,
            receipts: receipt_facts(&receipt_kinds),
            value_kinds: value_audit.value_kinds,
            receipt_kinds: receipt_kinds
                .iter()
                .map(|kind| kind.as_str().to_owned())
                .collect(),
            value_data_classes: value_audit.value_data_classes,
            errors: parts.errors,
            charge_count: parts.charge_count,
            assessment_batch_replayability: value_audit.assessment_batch_replayability,
            graph_row_fragments: value_audit.graph_row_fragments,
        })
    }

    /// Plan identifier this result answers.
    pub fn plan_id(&self) -> &str {
        &self.plan_id
    }

    /// Graph revision used as the plan read base.
    pub fn base_revision(&self) -> &str {
        &self.base_revision
    }

    /// Graph revision after the plan completed.
    pub fn final_revision(&self) -> &str {
        &self.final_revision
    }

    /// Plan-level replayability summary after semantic roll-up validation.
    pub fn replayability_summary(&self) -> Replayability {
        self.replayability_summary
    }

    /// Number of typed result values.
    pub fn value_count(&self) -> usize {
        self.values.len()
    }

    /// Number of operation receipts.
    pub fn receipt_count(&self) -> usize {
        self.receipts.len()
    }

    /// Number of typed plan errors.
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Number of charge receipts.
    pub fn charge_count(&self) -> usize {
        self.charge_count
    }

    /// Typed value kinds present in the result envelope.
    pub fn value_kinds(&self) -> &[String] {
        &self.value_kinds
    }

    /// Operation receipt kinds present in the result envelope.
    pub fn receipt_kinds(&self) -> &[String] {
        &self.receipt_kinds
    }

    /// Typed top-level result values present in the result envelope.
    pub fn values(&self) -> &[PlanResultValueFact] {
        &self.values
    }

    /// Typed operation receipts present in the result envelope.
    pub fn receipts(&self) -> &[PlanResultReceiptFact] {
        &self.receipts
    }

    /// Typed top-level charge section facts.
    pub const fn charges(&self) -> PlanResultChargeFacts {
        PlanResultChargeFacts {
            count: self.charge_count,
        }
    }

    /// Typed top-level error section facts.
    pub fn errors(&self) -> PlanResultErrorFacts {
        PlanResultErrorFacts {
            count: self.errors.len(),
        }
    }

    /// Typed `PlanErrors` carried by the top-level result error section.
    pub fn plan_errors(&self) -> &[PlanErrorDocument] {
        &self.errors
    }

    /// Data classes carried by each typed result value.
    pub fn value_data_classes(&self) -> &[(String, Vec<String>)] {
        &self.value_data_classes
    }

    /// Per-assessment replayability carried by assessment batch result values.
    pub fn assessment_batch_replayability(&self) -> &[(String, Replayability)] {
        &self.assessment_batch_replayability
    }

    /// Typed JSON fragments carried by graph-set summary rows.
    pub const fn graph_row_fragments(&self) -> &PlanResultGraphRowFragments {
        &self.graph_row_fragments
    }
}

fn receipt_facts(receipt_kinds: &[PlanResultReceiptKind]) -> Vec<PlanResultReceiptFact> {
    receipt_kinds
        .iter()
        .copied()
        .map(PlanResultReceiptFact::new)
        .collect()
}

/// Typed facts for one top-level Plan Result value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultValueFact {
    name: String,
    kind: PlanResultValueKind,
    data_classes: Vec<String>,
}

/// Typed JSON fragments carried by graph-set result rows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlanResultGraphRowFragments {
    candidate_scores: Vec<PlanResultCandidateScores>,
    candidate_artifacts: Vec<PlanResultCandidateArtifact>,
    proposal_effects: Vec<PlanResultProposalEffectSummary>,
    event_payloads: Vec<PlanResultGraphEventPayload>,
    extension_payloads: Vec<PlanResultGraphExtensionPayload>,
}

impl PlanResultGraphRowFragments {
    /// Candidate summary score fragments.
    pub fn candidate_scores(&self) -> &[PlanResultCandidateScores] {
        &self.candidate_scores
    }

    /// Candidate summary artifact fragments.
    pub fn candidate_artifacts(&self) -> &[PlanResultCandidateArtifact] {
        &self.candidate_artifacts
    }

    /// Proposal summary effect fragments.
    pub fn proposal_effects(&self) -> &[PlanResultProposalEffectSummary] {
        &self.proposal_effects
    }

    /// Event summary payload fragments.
    pub fn event_payloads(&self) -> &[PlanResultGraphEventPayload] {
        &self.event_payloads
    }

    /// Extension graph-row payload fragments.
    pub fn extension_payloads(&self) -> &[PlanResultGraphExtensionPayload] {
        &self.extension_payloads
    }
}

macro_rules! graph_row_fragment {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $name(Value);

        impl $name {
            fn from_schema_valid_value(value: &Value) -> Self {
                Self(value.clone())
            }

            /// JSON value carried on the wire by this graph-row fragment.
            pub const fn as_json(&self) -> &Value {
                &self.0
            }
        }
    };
}

graph_row_fragment!(
    PlanResultGraphEventPayload,
    "Schema-valid JSON carried by `event_summary.payload`."
);
graph_row_fragment!(
    PlanResultGraphExtensionPayload,
    "Schema-valid JSON carried by an extension graph row payload."
);

/// Closed typed score summary carried by `candidate_summary.scores`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultCandidateScores {
    primary: Option<Number>,
    metrics: Vec<(String, Number)>,
    cases: Vec<PlanResultCandidateCaseScore>,
}

impl PlanResultCandidateScores {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_result("candidate_summary.scores must be an object"))?;
        let primary = optional_number(object.get("primary"), "candidate_summary.scores.primary")?;
        let metrics =
            optional_number_map(object.get("metrics"), "candidate_summary.scores.metrics")?;
        let cases = optional_case_scores(object.get("cases"))?;
        Ok(Self {
            primary,
            metrics,
            cases,
        })
    }

    /// Primary candidate score when present.
    pub const fn primary(&self) -> Option<&Number> {
        self.primary.as_ref()
    }

    /// Numeric metric scores in deterministic key order.
    pub fn metrics(&self) -> &[(String, Number)] {
        &self.metrics
    }

    /// Case-level score summaries.
    pub fn cases(&self) -> &[PlanResultCandidateCaseScore] {
        &self.cases
    }
}

/// One case-level score carried by `candidate_summary.scores`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultCandidateCaseScore {
    case_id: String,
    score: Number,
}

impl PlanResultCandidateCaseScore {
    /// Case id this score belongs to.
    pub fn case_id(&self) -> &str {
        &self.case_id
    }

    /// Numeric score value.
    pub const fn score(&self) -> &Number {
        &self.score
    }
}

/// Closed typed artifact summary carried by `candidate_summary.artifact`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultCandidateArtifact {
    kind: String,
    identity: Option<String>,
    summary: Option<String>,
    body: Option<String>,
    schema_fingerprint: Option<String>,
}

impl PlanResultCandidateArtifact {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_result("candidate_summary.artifact must be an object"))?;
        Ok(Self {
            kind: required_string(object.get("kind"), "candidate_summary.artifact.kind")?
                .to_owned(),
            identity: optional_string(object.get("identity"))?,
            summary: optional_string(object.get("summary"))?,
            body: optional_string(object.get("body"))?,
            schema_fingerprint: optional_string(object.get("schema_fingerprint"))?,
        })
    }

    /// Artifact kind.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Artifact identity, when carried inline.
    pub fn identity(&self) -> Option<&str> {
        self.identity.as_deref()
    }

    /// Human-readable artifact summary, when present.
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    /// Inline artifact body, when present.
    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    /// Schema fingerprint for the artifact body, when present.
    pub fn schema_fingerprint(&self) -> Option<&str> {
        self.schema_fingerprint.as_deref()
    }
}

/// Closed typed summary carried by `proposal_summary.effect`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultProposalEffectSummary {
    kind: PlanResultProposalEffectKind,
    target_candidate_id: Option<String>,
    artifact_type: Option<String>,
    artifact_schema: Option<String>,
    workspace_id: Option<String>,
    agent_receipt_id: Option<String>,
    parser: Option<String>,
    surface_fingerprint: Option<String>,
    change_schema: Option<String>,
}

impl PlanResultProposalEffectSummary {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_result("proposal_summary.effect must be an object"))?;
        let kind = PlanResultProposalEffectKind::parse(required_string(
            object.get("kind"),
            "proposal_summary.effect.kind",
        )?)
        .ok_or_else(|| invalid_result("unknown proposal_summary.effect kind"))?;
        Ok(Self {
            kind,
            target_candidate_id: optional_ref_id(
                object.get("target"),
                "proposal_summary.effect.target",
            )?,
            artifact_type: optional_string(object.get("artifact_type"))?,
            artifact_schema: optional_string(object.get("artifact_schema"))?,
            workspace_id: optional_ref_id(
                object.get("workspace"),
                "proposal_summary.effect.workspace",
            )?,
            agent_receipt_id: optional_ref_id(
                object.get("agent_receipt"),
                "proposal_summary.effect.agent_receipt",
            )?,
            parser: optional_string(object.get("parser"))?,
            surface_fingerprint: optional_string(object.get("surface_fingerprint"))?,
            change_schema: optional_string(object.get("change_schema"))?,
        })
    }

    /// Proposal effect summary kind.
    pub const fn kind(&self) -> PlanResultProposalEffectKind {
        self.kind
    }

    /// Candidate id referenced by change-like summary effects, when present.
    pub fn target_candidate_id(&self) -> Option<&str> {
        self.target_candidate_id.as_deref()
    }

    /// Artifact type referenced by create summary effects, when present.
    pub fn artifact_type(&self) -> Option<&str> {
        self.artifact_type.as_deref()
    }

    /// Artifact schema fingerprint referenced by create summary effects, when present.
    pub fn artifact_schema(&self) -> Option<&str> {
        self.artifact_schema.as_deref()
    }

    /// Workspace id referenced by workspace-diff summary effects, when present.
    pub fn workspace_id(&self) -> Option<&str> {
        self.workspace_id.as_deref()
    }

    /// Agent receipt id referenced by agent-session summary effects, when present.
    pub fn agent_receipt_id(&self) -> Option<&str> {
        self.agent_receipt_id.as_deref()
    }

    /// Parser selected for imported summary effects, when present.
    pub fn parser(&self) -> Option<&str> {
        self.parser.as_deref()
    }

    /// Surface fingerprint referenced by change-like summary effects, when present.
    pub fn surface_fingerprint(&self) -> Option<&str> {
        self.surface_fingerprint.as_deref()
    }

    /// Change schema fingerprint referenced by change-like summary effects, when present.
    pub fn change_schema(&self) -> Option<&str> {
        self.change_schema.as_deref()
    }
}

/// Closed proposal effect kinds carried by graph-row summaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanResultProposalEffectKind {
    Create,
    Change,
    ChangeFromWorkspaceDiff,
    ChangeFromAgentSession,
}

impl PlanResultProposalEffectKind {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "create" => Self::Create,
            "change" => Self::Change,
            "change_from_workspace_diff" => Self::ChangeFromWorkspaceDiff,
            "change_from_agent_session" => Self::ChangeFromAgentSession,
            _ => return None,
        })
    }

    /// Wire spelling for this proposal effect summary kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Change => "change",
            Self::ChangeFromWorkspaceDiff => "change_from_workspace_diff",
            Self::ChangeFromAgentSession => "change_from_agent_session",
        }
    }
}

impl PlanResultValueFact {
    fn new(name: impl Into<String>, kind: PlanResultValueKind, data_classes: Vec<String>) -> Self {
        Self {
            name: name.into(),
            kind,
            data_classes,
        }
    }

    /// Result value binding name in the top-level `values` map.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Typed result value kind.
    pub const fn kind(&self) -> PlanResultValueKind {
        self.kind
    }

    /// Data classes carried by this value.
    pub fn data_classes(&self) -> &[String] {
        &self.data_classes
    }
}

/// Locked top-level Plan Result value kinds currently accepted by the public seam.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PlanResultValueKind {
    GraphSet,
    CaseRecord,
    WorkspaceFile,
    WorkspaceDiff,
    WorkspaceListing,
    WorkspaceSnapshot,
    WorkspaceHandle,
    LmResponse,
    AgentSession,
    SandboxExec,
    ProposalBatchReceipt,
    AssessmentBatchReceipt,
    EvaluationRequestReceipt,
    EmitRunEvent,
    ApplyReceipt,
}

impl PlanResultValueKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "graph_set" => Self::GraphSet,
            "case_record" => Self::CaseRecord,
            "workspace_file" => Self::WorkspaceFile,
            "workspace_diff" => Self::WorkspaceDiff,
            "workspace_listing" => Self::WorkspaceListing,
            "workspace_snapshot" => Self::WorkspaceSnapshot,
            "workspace_handle" => Self::WorkspaceHandle,
            "lm_response" => Self::LmResponse,
            "agent_session" => Self::AgentSession,
            "sandbox_exec" => Self::SandboxExec,
            "proposal_batch_receipt" => Self::ProposalBatchReceipt,
            "assessment_batch_receipt" => Self::AssessmentBatchReceipt,
            "evaluation_request_receipt" => Self::EvaluationRequestReceipt,
            "emit_run_event" => Self::EmitRunEvent,
            "apply_receipt" => Self::ApplyReceipt,
            _ => return None,
        })
    }

    /// Wire spelling for this value kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GraphSet => "graph_set",
            Self::CaseRecord => "case_record",
            Self::WorkspaceFile => "workspace_file",
            Self::WorkspaceDiff => "workspace_diff",
            Self::WorkspaceListing => "workspace_listing",
            Self::WorkspaceSnapshot => "workspace_snapshot",
            Self::WorkspaceHandle => "workspace_handle",
            Self::LmResponse => "lm_response",
            Self::AgentSession => "agent_session",
            Self::SandboxExec => "sandbox_exec",
            Self::ProposalBatchReceipt => "proposal_batch_receipt",
            Self::AssessmentBatchReceipt => "assessment_batch_receipt",
            Self::EvaluationRequestReceipt => "evaluation_request_receipt",
            Self::EmitRunEvent => "emit_run_event",
            Self::ApplyReceipt => "apply_receipt",
        }
    }
}

/// Typed facts for one top-level operation receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanResultReceiptFact {
    kind: PlanResultReceiptKind,
}

impl PlanResultReceiptFact {
    pub(crate) const fn new(kind: PlanResultReceiptKind) -> Self {
        Self { kind }
    }

    /// Typed operation receipt kind.
    pub const fn kind(self) -> PlanResultReceiptKind {
        self.kind
    }
}

/// Locked operation receipt kinds accepted by Plan Result validation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PlanResultReceiptKind {
    Query,
    Call,
    Write,
}

impl PlanResultReceiptKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "query" => Self::Query,
            "call" => Self::Call,
            "write" => Self::Write,
            _ => return None,
        })
    }

    /// Wire spelling for this receipt kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Call => "call",
            Self::Write => "write",
        }
    }
}

/// Typed facts for the top-level Plan Result charge section.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanResultChargeFacts {
    count: usize,
}

impl PlanResultChargeFacts {
    /// Number of charge receipts carried by the result.
    pub const fn count(self) -> usize {
        self.count
    }

    /// Whether the result has no charge receipts.
    pub const fn is_empty(self) -> bool {
        self.count == 0
    }
}

/// Typed facts for the top-level Plan Result error section.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanResultErrorFacts {
    count: usize,
}

impl PlanResultErrorFacts {
    /// Number of typed plan errors carried by the result.
    pub const fn count(self) -> usize {
        self.count
    }

    /// Whether the result has no typed plan errors.
    pub const fn is_empty(self) -> bool {
        self.count == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestEvaluationReceiptPolicy {
    Reject,
    AllowDedicatedValidation,
}

struct PlanResultParts<'a> {
    plan_id: &'a str,
    base_revision: &'a str,
    final_revision: &'a str,
    values: &'a serde_json::Map<String, Value>,
    receipts: &'a [Value],
    charges: &'a [Value],
    errors: Vec<PlanErrorDocument>,
    charge_count: usize,
}

impl<'a> PlanResultParts<'a> {
    fn from_object(object: &'a serde_json::Map<String, Value>) -> Result<Self, PublicSeamError> {
        Ok(Self {
            plan_id: required_string(object.get("plan_id"), "plan_id")?,
            base_revision: required_string(object.get("base_revision"), "base_revision")?,
            final_revision: required_string(object.get("final_revision"), "final_revision")?,
            values: object
                .get("values")
                .and_then(Value::as_object)
                .ok_or_else(|| invalid_result("plan result values must be an object"))?,
            receipts: object
                .get("receipts")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .ok_or_else(|| invalid_result("plan result receipts must be an array"))?,
            charges: object
                .get("charges")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .ok_or_else(|| invalid_result("plan result charges must be an array"))?,
            errors: plan_errors_from_object(object)?,
            charge_count: array_len(object, "charges")?,
        })
    }
}

fn plan_errors_from_object(
    object: &serde_json::Map<String, Value>,
) -> Result<Vec<PlanErrorDocument>, PublicSeamError> {
    let errors = object
        .get("errors")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid_result("plan result errors must be an array"))?;
    plan_error::closed_plan_errors(errors, "plan result error").map_err(invalid_result)
}

struct ValueAudit {
    values: Vec<PlanResultValueFact>,
    value_kinds: Vec<String>,
    value_data_classes: Vec<(String, Vec<String>)>,
    assessment_batch_replayability: Vec<(String, Replayability)>,
    assessment_batches: Vec<AssessmentBatchScope>,
    graph_row_fragments: PlanResultGraphRowFragments,
}

fn inspect_values(
    values: &serde_json::Map<String, Value>,
    receipt_index: &BTreeMap<String, ReceiptAudit>,
    replayability_summary: Replayability,
) -> Result<ValueAudit, PublicSeamError> {
    let mut value_facts = Vec::with_capacity(values.len());
    let mut value_kinds = Vec::with_capacity(values.len());
    let mut value_data_classes = Vec::with_capacity(values.len());
    let mut value_replayability = Vec::with_capacity(values.len());
    let mut assessment_batch_replayability = Vec::new();
    let mut assessment_batches = Vec::new();
    let mut graph_row_fragments = PlanResultGraphRowFragments::default();
    for (name, value) in values {
        let value_object = value
            .as_object()
            .ok_or_else(|| invalid_result("plan result value must be an object"))?;
        let value_kind = inspect_value_receipt(value_object, receipt_index)?;
        let typed_value_kind = PlanResultValueKind::parse(value_kind).ok_or_else(|| {
            invalid_result(format!("unknown plan result value kind `{value_kind}`"))
        })?;
        let data_classes = optional_string_set(value_object.get("data_classes"), "data_classes")?;
        validate_value_visibility(name, value_object, &data_classes, receipt_index)?;
        validate_graph_set_assessment_summaries(value_object, receipt_index)?;
        collect_graph_row_fragments(value_object, &mut graph_row_fragments)?;
        let data_classes = data_classes.into_iter().collect::<Vec<_>>();
        value_facts.push(PlanResultValueFact::new(
            name.to_owned(),
            typed_value_kind,
            data_classes.clone(),
        ));
        value_kinds.push(value_kind.to_owned());
        value_data_classes.push((name.to_owned(), data_classes));
        value_replayability.push(required_replayability(value_object.get("replayability"))?);
        if value_kind == "assessment_batch_receipt" {
            inspect_assessment_batch_value(
                value_object,
                &mut assessment_batch_replayability,
                &mut assessment_batches,
            )?;
        }
    }
    validate_replayability_rollups(
        replayability_summary,
        &value_replayability,
        &assessment_batch_replayability,
    )?;
    Ok(ValueAudit {
        values: value_facts,
        value_kinds,
        value_data_classes,
        assessment_batch_replayability,
        assessment_batches,
        graph_row_fragments,
    })
}

fn collect_graph_row_fragments(
    value: &serde_json::Map<String, Value>,
    fragments: &mut PlanResultGraphRowFragments,
) -> Result<(), PublicSeamError> {
    if value.get("kind").and_then(Value::as_str) != Some("graph_set") {
        return Ok(());
    }
    let Some(items) = value.get("items").and_then(Value::as_array) else {
        return Ok(());
    };
    for item in items {
        let Some(item_object) = item.as_object() else {
            continue;
        };
        match item_object.get("kind").and_then(Value::as_str) {
            Some("candidate_summary") => {
                if let Some(scores) = item_object.get("scores") {
                    fragments
                        .candidate_scores
                        .push(PlanResultCandidateScores::from_schema_valid_value(scores)?);
                }
                if let Some(artifact) = item_object.get("artifact") {
                    fragments.candidate_artifacts.push(
                        PlanResultCandidateArtifact::from_schema_valid_value(artifact)?,
                    );
                }
            }
            Some("proposal_summary") => {
                if let Some(effect) = item_object.get("effect") {
                    fragments.proposal_effects.push(
                        PlanResultProposalEffectSummary::from_schema_valid_value(effect)?,
                    );
                }
            }
            Some("event_summary") => {
                if let Some(payload) = item_object.get("payload") {
                    fragments.event_payloads.push(
                        PlanResultGraphEventPayload::from_schema_valid_value(payload),
                    );
                }
            }
            Some("extension") => {
                let payload = item_object
                    .get("payload")
                    .ok_or_else(|| invalid_result("extension graph row must carry payload"))?;
                fragments.extension_payloads.push(
                    PlanResultGraphExtensionPayload::from_schema_valid_value(payload),
                );
            }
            _ => {}
        }
    }
    Ok(())
}

fn optional_string(value: Option<&Value>) -> Result<Option<String>, PublicSeamError> {
    value
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                invalid_result("optional proposal effect summary field must be a string")
            })
        })
        .transpose()
}

fn optional_number(value: Option<&Value>, field: &str) -> Result<Option<Number>, PublicSeamError> {
    value
        .map(|value| {
            value
                .as_number()
                .cloned()
                .ok_or_else(|| invalid_result(format!("{field} must be a number")))
        })
        .transpose()
}

fn optional_number_map(
    value: Option<&Value>,
    field: &str,
) -> Result<Vec<(String, Number)>, PublicSeamError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| invalid_result(format!("{field} must be an object")))?;
    object
        .iter()
        .map(|(key, value)| {
            Ok((
                key.to_owned(),
                value
                    .as_number()
                    .cloned()
                    .ok_or_else(|| invalid_result(format!("{field}.{key} must be a number")))?,
            ))
        })
        .collect()
}

fn optional_case_scores(
    value: Option<&Value>,
) -> Result<Vec<PlanResultCandidateCaseScore>, PublicSeamError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .ok_or_else(|| invalid_result("candidate_summary.scores.cases must be an array"))?;
    items
        .iter()
        .map(|item| {
            let object = item.as_object().ok_or_else(|| {
                invalid_result("candidate_summary.scores.cases item must be an object")
            })?;
            Ok(PlanResultCandidateCaseScore {
                case_id: ref_id(
                    object.get("case").ok_or_else(|| {
                        invalid_result("candidate_summary.scores.cases.case missing")
                    })?,
                    "candidate_summary.scores.cases.case",
                )?
                .to_owned(),
                score: object
                    .get("score")
                    .and_then(Value::as_number)
                    .cloned()
                    .ok_or_else(|| {
                        invalid_result("candidate_summary.scores.cases.score must be a number")
                    })?,
            })
        })
        .collect()
}

fn optional_ref_id(value: Option<&Value>, field: &str) -> Result<Option<String>, PublicSeamError> {
    value
        .map(|value| ref_id(value, field).map(ToOwned::to_owned))
        .transpose()
}

fn ref_id<'a>(value: &'a Value, field: &str) -> Result<&'a str, PublicSeamError> {
    if let Some(id) = value.as_str() {
        return Ok(id);
    }
    value
        .as_object()
        .and_then(|object| object.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_result(format!("{field} must be a string ref or object ref")))
}

fn validate_graph_set_assessment_summaries(
    value: &serde_json::Map<String, Value>,
    receipt_index: &BTreeMap<String, ReceiptAudit>,
) -> Result<(), PublicSeamError> {
    if value.get("kind").and_then(Value::as_str) != Some("graph_set") {
        return Ok(());
    }
    let Some(items) = value.get("items").and_then(Value::as_array) else {
        return Ok(());
    };
    for item in items {
        let Some(item_object) = item.as_object() else {
            continue;
        };
        if item_object.get("kind").and_then(Value::as_str) == Some("assessment_summary") {
            validate_assessment_summary(item_object, receipt_index)?;
        }
    }
    Ok(())
}

fn validate_assessment_summary(
    item: &serde_json::Map<String, Value>,
    receipt_index: &BTreeMap<String, ReceiptAudit>,
) -> Result<(), PublicSeamError> {
    let score = item
        .get("score")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_result("assessment_summary must carry score"))?;
    let output = score
        .get("output")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_result("assessment_summary score must carry Score.output"))?;
    validate_assessment_summary_output(output)?;

    let evidence = item
        .get("evidence")
        .ok_or_else(|| invalid_result("assessment_summary must carry evidence"))?;
    let envelope =
        EvidenceEnvelopeDocument::from_schema_valid_value(evidence).map_err(|source| {
            invalid_result(format!("assessment_summary evidence invalid: {source}"))
        })?;
    validate_evidence_source_receipts(&envelope, receipt_index)?;
    if let Some(summary) = reportable_output_summary(output) {
        validate_optional_assessment_summary_evidence_projection(evidence, summary)?;
    }
    Ok(())
}

fn validate_assessment_summary_output(
    output: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let data_classes =
        optional_string_set(output.get("data_classes"), "Score.output.data_classes")?;
    let carries_assessed_output = data_classes
        .iter()
        .any(|class| matches!(class.as_str(), "candidate.output" | "candidate.artifact"));
    if !carries_assessed_output {
        return Err(invalid_result(
            "assessment_summary Score.output must carry candidate.output or candidate.artifact data class",
        ));
    }
    if output.get("value").is_some_and(is_non_string_value)
        && reportable_output_summary(output).is_none()
    {
        return Err(invalid_result(
            "assessment_summary structured/json Score.output value must carry a non-empty summary for evidence projection",
        ));
    }
    if output
        .get("summary")
        .and_then(Value::as_str)
        .is_some_and(|summary| !summary.trim().is_empty())
        || output.get("value").is_some_and(has_reportable_content)
        || output.get("blob_ref").is_some()
        || output
            .get("trace_refs")
            .and_then(Value::as_array)
            .is_some_and(|trace_refs| !trace_refs.is_empty())
    {
        return Ok(());
    }
    Err(invalid_result(
        "assessment_summary Score.output must carry reportable output content",
    ))
}

fn is_non_string_value(value: &Value) -> bool {
    !matches!(value, Value::String(_))
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

fn reportable_output_summary(output: &serde_json::Map<String, Value>) -> Option<&str> {
    output
        .get("summary")
        .and_then(Value::as_str)
        .filter(|summary| !summary.trim().is_empty())
        .or_else(|| {
            output
                .get("value")
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty())
        })
}

fn validate_optional_assessment_summary_evidence_projection(
    evidence: &Value,
    expected_summary: &str,
) -> Result<(), PublicSeamError> {
    let Some(evidence_summary) = evidence
        .get("public")
        .and_then(Value::as_object)
        .and_then(|public| public.get("summary"))
        .and_then(Value::as_str)
    else {
        return Err(invalid_result(
            "assessment_summary evidence.public.summary must project Score.output summary",
        ));
    };
    if evidence_summary == expected_summary {
        Ok(())
    } else {
        Err(invalid_result(
            "assessment_summary Score.output must match evidence.public.summary",
        ))
    }
}

fn validate_evidence_source_receipts(
    envelope: &EvidenceEnvelopeDocument,
    receipt_index: &BTreeMap<String, ReceiptAudit>,
) -> Result<(), PublicSeamError> {
    let envelope_data_classes = evidence_data_class_set(envelope);
    validate_evidence_receipts(
        envelope.read_receipt_refs(),
        receipt_index,
        "query",
        "read",
        &envelope_data_classes,
        envelope.is_target_derived(),
    )?;
    validate_evidence_receipts(
        envelope.effect_receipt_refs(),
        receipt_index,
        "call",
        "effect",
        &envelope_data_classes,
        false,
    )?;
    validate_evidence_receipts(
        envelope.write_receipt_refs(),
        receipt_index,
        "write",
        "write",
        &envelope_data_classes,
        false,
    )
}

fn validate_evidence_receipts(
    receipts: &[EvidenceReceiptRef],
    receipt_index: &BTreeMap<String, ReceiptAudit>,
    expected_kind: &str,
    receipt_role: &str,
    envelope_data_classes: &BTreeSet<String>,
    require_receipt_visibility: bool,
) -> Result<(), PublicSeamError> {
    for receipt in receipts {
        let Some(audit) = receipt_index.get(receipt.id()) else {
            return Err(invalid_result(format!(
                "evidence {receipt_role} receipt `{}` is missing from plan result receipts",
                receipt.id()
            )));
        };
        if audit.kind != expected_kind {
            return Err(invalid_result(format!(
                "evidence {receipt_role} receipt `{}` references `{}` receipt, expected `{expected_kind}`",
                receipt.id(),
                audit.kind
            )));
        }
        if let Some(fingerprint) = receipt.fingerprint()
            && fingerprint != audit.fingerprint
        {
            return Err(invalid_result(format!(
                "evidence {receipt_role} receipt `{}` fingerprint does not match plan result receipt",
                receipt.id()
            )));
        }
        if require_receipt_visibility && audit.trace_data_classes.is_empty() {
            return Err(invalid_result(format!(
                "target-derived evidence {receipt_role} receipt `{}` must carry receipt trace data classes",
                receipt.id()
            )));
        }
        if require_receipt_visibility && !audit.trace_data_classes.contains("case.target") {
            return Err(invalid_result(format!(
                "target-derived evidence {receipt_role} receipt `{}` must carry case.target receipt trace data class",
                receipt.id()
            )));
        }
        for data_class in &audit.trace_data_classes {
            if !envelope_data_classes.contains(data_class) {
                return Err(invalid_result(format!(
                    "evidence {receipt_role} receipt `{}` trace data class `{data_class}` is not covered by evidence data_classes",
                    receipt.id()
                )));
            }
        }
    }
    Ok(())
}

fn evidence_data_class_set(envelope: &EvidenceEnvelopeDocument) -> BTreeSet<String> {
    let mut data_classes = BTreeSet::new();
    data_classes.extend(envelope.data_classes().iter().cloned());
    data_classes.extend(envelope.public_data_classes().iter().cloned());
    if let Some(private) = envelope.private_data_classes() {
        data_classes.extend(private.iter().cloned());
    }
    data_classes.extend(envelope.trace_data_classes().iter().cloned());
    data_classes
}
