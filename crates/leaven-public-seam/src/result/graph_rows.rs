use std::collections::BTreeMap;

use serde_json::{Number, Value};

use crate::PublicSeamError;
use crate::evidence::EvidenceEnvelopeDocument;

use super::audit::ReceiptAudit;
use super::helpers::optional_string_set;
use super::helpers::{invalid_result, required_string};

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

/// Closed payload carried by an extension graph row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanResultGraphExtensionPayload {
    Summary(PlanResultGraphExtensionSummaryPayload),
    BlobRef(PlanResultGraphExtensionBlobRefPayload),
}

impl PlanResultGraphExtensionPayload {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_result("extension graph row payload must be an object"))?;
        match required_string(object.get("kind"), "extension graph row payload.kind")? {
            "summary" => Ok(Self::Summary(
                PlanResultGraphExtensionSummaryPayload::from_object(object)?,
            )),
            "blob_ref" => Ok(Self::BlobRef(
                PlanResultGraphExtensionBlobRefPayload::from_object(object)?,
            )),
            kind => Err(invalid_result(format!(
                "extension graph row payload kind `{kind}` is not locked in V1"
            ))),
        }
    }
}

/// Inline summary payload for an extension graph row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultGraphExtensionSummaryPayload {
    summary: String,
    data_classes: Vec<String>,
    source_ref: Option<String>,
}

impl PlanResultGraphExtensionSummaryPayload {
    fn from_object(object: &serde_json::Map<String, Value>) -> Result<Self, PublicSeamError> {
        Ok(Self {
            summary: required_string(object.get("summary"), "extension graph row payload.summary")?
                .to_owned(),
            data_classes: optional_string_vec(object, "data_classes")?,
            source_ref: optional_ref_id(
                object.get("source_ref"),
                "extension graph row payload.source_ref",
            )?,
        })
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn data_classes(&self) -> &[String] {
        &self.data_classes
    }

    pub fn source_ref(&self) -> Option<&str> {
        self.source_ref.as_deref()
    }
}

/// Durable blob-reference payload for an extension graph row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultGraphExtensionBlobRefPayload {
    blob_id: String,
    summary: Option<String>,
    data_classes: Vec<String>,
}

impl PlanResultGraphExtensionBlobRefPayload {
    fn from_object(object: &serde_json::Map<String, Value>) -> Result<Self, PublicSeamError> {
        let blob = object
            .get("blob")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_result("extension graph row payload.blob must be an object"))?;
        Ok(Self {
            blob_id: required_string(blob.get("id"), "extension graph row payload.blob.id")?
                .to_owned(),
            summary: optional_string(object, "summary")?,
            data_classes: optional_string_vec(object, "data_classes")?,
        })
    }

    pub fn blob_id(&self) -> &str {
        &self.blob_id
    }

    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    pub fn data_classes(&self) -> &[String] {
        &self.data_classes
    }
}

/// Typed payload carried by `event_summary.payload`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanResultGraphEventPayload {
    ExternalEvent(PlanResultExternalEventPayload),
    EventEmitted(PlanResultEventEmittedPayload),
    ProposalBatchSubmitted(PlanResultProposalBatchSubmittedPayload),
    ProposalBatchApplied(PlanResultProposalBatchAppliedPayload),
    AssessmentsSubmitted(PlanResultAssessmentsSubmittedPayload),
    EvaluationRequested(PlanResultEvaluationRequestedPayload),
    RunContextSummary(PlanResultRunContextSummaryPayload),
}

impl PlanResultGraphEventPayload {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_result("event_summary.payload must be an object"))?;
        match required_string(object.get("kind"), "event_summary.payload.kind")? {
            "external_event" => Ok(Self::ExternalEvent(
                PlanResultExternalEventPayload::from_object(object)?,
            )),
            "event_emitted" => Ok(Self::EventEmitted(
                PlanResultEventEmittedPayload::from_object(object)?,
            )),
            "proposal_batch_submitted" => Ok(Self::ProposalBatchSubmitted(
                PlanResultProposalBatchSubmittedPayload::from_object(object)?,
            )),
            "proposal_batch_applied" => Ok(Self::ProposalBatchApplied(
                PlanResultProposalBatchAppliedPayload::from_object(object)?,
            )),
            "assessments_submitted" => Ok(Self::AssessmentsSubmitted(
                PlanResultAssessmentsSubmittedPayload::from_object(object)?,
            )),
            "evaluation_requested" => Ok(Self::EvaluationRequested(
                PlanResultEvaluationRequestedPayload::from_object(object)?,
            )),
            "run_context_summary" => Ok(Self::RunContextSummary(
                PlanResultRunContextSummaryPayload::from_object(object)?,
            )),
            kind => Err(invalid_result(format!(
                "event_summary.payload kind `{kind}` is not locked in V1"
            ))),
        }
    }
}

/// External event payload details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultExternalEventPayload {
    ok: bool,
    stage_call_id: Option<String>,
}

impl PlanResultExternalEventPayload {
    fn from_object(object: &serde_json::Map<String, Value>) -> Result<Self, PublicSeamError> {
        Ok(Self {
            ok: object
                .get("ok")
                .and_then(Value::as_bool)
                .ok_or_else(|| invalid_result("external_event payload must carry bool `ok`"))?,
            stage_call_id: optional_string(object, "stage_call_id")?,
        })
    }

    pub const fn ok(&self) -> bool {
        self.ok
    }

    pub fn stage_call_id(&self) -> Option<&str> {
        self.stage_call_id.as_deref()
    }
}

/// Event-emitted graph summary payload details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultEventEmittedPayload {
    event_id: String,
    event_kind: String,
    payload_schema: String,
    value: PlanResultExternalEventPayload,
    visibility: String,
}

impl PlanResultEventEmittedPayload {
    fn from_object(object: &serde_json::Map<String, Value>) -> Result<Self, PublicSeamError> {
        let value = object
            .get("value")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_result("event_emitted payload must carry object `value`"))?;
        Ok(Self {
            event_id: required_string(object.get("event_id"), "event_id")?.to_owned(),
            event_kind: required_string(object.get("event_kind"), "event_kind")?.to_owned(),
            payload_schema: required_string(object.get("payload_schema"), "payload_schema")?
                .to_owned(),
            value: PlanResultExternalEventPayload::from_object(value)?,
            visibility: required_string(object.get("visibility"), "visibility")?.to_owned(),
        })
    }

    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn event_kind(&self) -> &str {
        &self.event_kind
    }

    pub fn payload_schema(&self) -> &str {
        &self.payload_schema
    }

    pub const fn value(&self) -> &PlanResultExternalEventPayload {
        &self.value
    }

    pub fn visibility(&self) -> &str {
        &self.visibility
    }
}

/// Proposal-batch-submitted event summary payload details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultProposalBatchSubmittedPayload {
    proposal_batch: String,
    proposal_ids: Vec<String>,
}

impl PlanResultProposalBatchSubmittedPayload {
    fn from_object(object: &serde_json::Map<String, Value>) -> Result<Self, PublicSeamError> {
        Ok(Self {
            proposal_batch: required_string(object.get("proposal_batch"), "proposal_batch")?
                .to_owned(),
            proposal_ids: required_string_vec(object, "proposal_ids")?,
        })
    }
}

/// Proposal-batch-applied event summary payload details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultProposalBatchAppliedPayload {
    proposal_batch: String,
    created_candidates: Vec<String>,
}

impl PlanResultProposalBatchAppliedPayload {
    fn from_object(object: &serde_json::Map<String, Value>) -> Result<Self, PublicSeamError> {
        Ok(Self {
            proposal_batch: required_string(object.get("proposal_batch"), "proposal_batch")?
                .to_owned(),
            created_candidates: required_string_vec(object, "created_candidates")?,
        })
    }
}

/// Assessments-submitted event summary payload details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultAssessmentsSubmittedPayload {
    evaluation_request_id: String,
    assessment_ids: Vec<String>,
}

impl PlanResultAssessmentsSubmittedPayload {
    fn from_object(object: &serde_json::Map<String, Value>) -> Result<Self, PublicSeamError> {
        Ok(Self {
            evaluation_request_id: required_string(
                object.get("evaluation_request_id"),
                "evaluation_request_id",
            )?
            .to_owned(),
            assessment_ids: required_string_vec(object, "assessment_ids")?,
        })
    }
}

/// Evaluation-requested event summary payload details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultEvaluationRequestedPayload {
    name: String,
    evaluation_request_id: String,
    evaluator_id: String,
}

impl PlanResultEvaluationRequestedPayload {
    fn from_object(object: &serde_json::Map<String, Value>) -> Result<Self, PublicSeamError> {
        Ok(Self {
            name: required_string(object.get("name"), "name")?.to_owned(),
            evaluation_request_id: required_string(
                object.get("evaluation_request_id"),
                "evaluation_request_id",
            )?
            .to_owned(),
            evaluator_id: required_string(object.get("evaluator_id"), "evaluator_id")?.to_owned(),
        })
    }
}

/// `RunContext` graph summary payload details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultRunContextSummaryPayload {
    source: String,
    candidate_count: u64,
    proposal_batch: String,
    created_candidates: Vec<String>,
    event_count: u64,
    emitted_events: Vec<PlanResultEventEmittedPayload>,
    evaluation_request_id: Option<String>,
    assessment_ids: Vec<String>,
    applied: bool,
}

impl PlanResultRunContextSummaryPayload {
    fn from_object(object: &serde_json::Map<String, Value>) -> Result<Self, PublicSeamError> {
        let emitted_events = object
            .get("emitted_events")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_result("run_context_summary.emitted_events must be an array"))?
            .iter()
            .map(|value| {
                let event = value.as_object().ok_or_else(|| {
                    invalid_result("run_context_summary.emitted_events entries must be objects")
                })?;
                if required_string(event.get("kind"), "emitted_events.kind")? != "event_emitted" {
                    return Err(invalid_result(
                        "run_context_summary emitted events must use `event_emitted` payloads",
                    ));
                }
                PlanResultEventEmittedPayload::from_object(event)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            source: required_string(object.get("source"), "source")?.to_owned(),
            candidate_count: required_u64(object, "candidate_count")?,
            proposal_batch: required_string(object.get("proposal_batch"), "proposal_batch")?
                .to_owned(),
            created_candidates: required_string_vec(object, "created_candidates")?,
            event_count: required_u64(object, "event_count")?,
            emitted_events,
            evaluation_request_id: optional_nullable_string(object, "evaluation_request_id")?,
            assessment_ids: required_string_vec(object, "assessment_ids")?,
            applied: required_bool(object, "applied")?,
        })
    }
}

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
            identity: optional_string(object, "identity")?,
            summary: optional_string(object, "summary")?,
            body: optional_string(object, "body")?,
            schema_fingerprint: optional_string(object, "schema_fingerprint")?,
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
            artifact_type: optional_string(object, "artifact_type")?,
            artifact_schema: optional_string(object, "artifact_schema")?,
            workspace_id: optional_ref_id(
                object.get("workspace"),
                "proposal_summary.effect.workspace",
            )?,
            agent_receipt_id: optional_ref_id(
                object.get("agent_receipt"),
                "proposal_summary.effect.agent_receipt",
            )?,
            parser: optional_string(object, "parser")?,
            surface_fingerprint: optional_string(object, "surface_fingerprint")?,
            change_schema: optional_string(object, "change_schema")?,
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

pub(super) fn collect_graph_row_fragments(
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
                        PlanResultGraphEventPayload::from_schema_valid_value(payload)?,
                    );
                }
            }
            Some("extension") => {
                let payload = item_object
                    .get("payload")
                    .ok_or_else(|| invalid_result("extension graph row must carry payload"))?;
                fragments.extension_payloads.push(
                    PlanResultGraphExtensionPayload::from_schema_valid_value(payload)?,
                );
            }
            _ => {}
        }
    }
    Ok(())
}

fn optional_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<String>, PublicSeamError> {
    object
        .get(field)
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                invalid_result(format!("optional result field `{field}` must be a string"))
            })
        })
        .transpose()
}

fn optional_nullable_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<String>, PublicSeamError> {
    match object.get(field) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| {
                invalid_result(format!(
                    "optional result field `{field}` must be a string or null"
                ))
            }),
    }
}

fn required_string_vec(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Vec<String>, PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_result(format!("{field} must be an array")))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid_result(format!("{field} entries must be strings")))
        })
        .collect()
}

fn optional_string_vec(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Vec<String>, PublicSeamError> {
    let Some(value) = object.get(field) else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .ok_or_else(|| invalid_result(format!("{field} must be an array")))?;
    items
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid_result(format!("{field} entries must be strings")))
        })
        .collect()
}

fn required_u64(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<u64, PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_result(format!("{field} must be an unsigned integer")))
}

fn required_bool(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<bool, PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid_result(format!("{field} must be a bool")))
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

pub(super) fn validate_graph_set_assessment_summaries(
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
    super::visibility::validate_evidence_source_receipts(&envelope, receipt_index)?;
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
