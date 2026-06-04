use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

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

    /// Typed PlanErrors carried by the top-level result error section.
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
    })
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
