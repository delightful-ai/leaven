use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::PublicSeamError;
use crate::evidence::{EvidenceEnvelopeDocument, EvidenceReceiptRef};

mod helpers;

use helpers::{
    array_len, invalid_result, optional_string_set, prefixed_jcs_hash, required_replayability,
    required_string, required_string_set,
};

/// Schema-valid public-seam Plan Result classified by replayability facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultDocument {
    plan_id: String,
    base_revision: String,
    final_revision: String,
    replayability_summary: Replayability,
    value_kinds: Vec<String>,
    receipt_kinds: Vec<String>,
    value_data_classes: Vec<(String, Vec<String>)>,
    error_count: usize,
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
            value_kinds: value_audit.value_kinds,
            receipt_kinds,
            value_data_classes: value_audit.value_data_classes,
            error_count: parts.error_count,
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
        self.value_kinds.len()
    }

    /// Number of operation receipts.
    pub fn receipt_count(&self) -> usize {
        self.receipt_kinds.len()
    }

    /// Number of typed plan errors.
    pub fn error_count(&self) -> usize {
        self.error_count
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

    /// Data classes carried by each typed result value.
    pub fn value_data_classes(&self) -> &[(String, Vec<String>)] {
        &self.value_data_classes
    }

    /// Per-assessment replayability carried by assessment batch result values.
    pub fn assessment_batch_replayability(&self) -> &[(String, Replayability)] {
        &self.assessment_batch_replayability
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
    error_count: usize,
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
            error_count: array_len(object, "errors")?,
            charge_count: array_len(object, "charges")?,
        })
    }
}

struct ValueAudit {
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
        let data_classes = optional_string_set(value_object.get("data_classes"), "data_classes")?;
        validate_value_visibility(name, value_object, &data_classes, receipt_index)?;
        validate_graph_set_assessment_summaries(value_object, receipt_index)?;
        value_kinds.push(value_kind.to_owned());
        value_data_classes.push((name.to_owned(), data_classes.into_iter().collect()));
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

fn validate_value_visibility(
    value_name: &str,
    value: &serde_json::Map<String, Value>,
    value_data_classes: &BTreeSet<String>,
    receipt_index: &BTreeMap<String, ReceiptAudit>,
) -> Result<(), PublicSeamError> {
    let mut required = BTreeSet::new();
    collect_score_output_data_classes_from_value(&Value::Object(value.clone()), &mut required)?;
    collect_evidence_data_classes_from_value(
        &Value::Object(value.clone()),
        receipt_index,
        &mut required,
    )?;
    collect_value_trace_data_classes(value, &mut required)?;
    collect_value_blob_ref_data_classes(value, &mut required)?;
    collect_workspace_listing_data_classes(value, &mut required)?;
    for data_class in required {
        if !value_data_classes.contains(&data_class) {
            return Err(invalid_result(format!(
                "result value `{value_name}` data_classes must cover nested visibility data class `{data_class}`"
            )));
        }
    }
    Ok(())
}

fn collect_value_trace_data_classes(
    value: &serde_json::Map<String, Value>,
    required: &mut BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    if let Some(trace_refs) = value.get("trace_refs") {
        collect_trace_ref_data_classes(trace_refs, "value.trace_refs", required)?;
    }
    Ok(())
}

fn collect_value_blob_ref_data_classes(
    value: &serde_json::Map<String, Value>,
    required: &mut BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    collect_optional_blob_ref_data_classes(value.get("blob_ref"), "value.blob_ref", required)?;
    collect_optional_blob_ref_data_classes(
        value.get("transcript_ref"),
        "value.transcript_ref",
        required,
    )?;
    collect_optional_blob_ref_data_classes(value.get("stdout_ref"), "value.stdout_ref", required)?;
    collect_optional_blob_ref_data_classes(value.get("stderr_ref"), "value.stderr_ref", required)?;
    if let Some(files) = value.get("files") {
        let files = files
            .as_object()
            .ok_or_else(|| invalid_result("value.files must be an object"))?;
        for (path, blob_ref) in files {
            collect_optional_blob_ref_data_classes(
                Some(blob_ref),
                &format!("value.files[{path}]"),
                required,
            )?;
        }
    }
    if let Some(commands) = value.get("commands") {
        let commands = commands
            .as_array()
            .ok_or_else(|| invalid_result("value.commands must be an array"))?;
        for (index, command) in commands.iter().enumerate() {
            let command = command
                .as_object()
                .ok_or_else(|| invalid_result("value.commands entries must be objects"))?;
            collect_optional_blob_ref_data_classes(
                command.get("stdout_ref"),
                &format!("value.commands[{index}].stdout_ref"),
                required,
            )?;
            collect_optional_blob_ref_data_classes(
                command.get("stderr_ref"),
                &format!("value.commands[{index}].stderr_ref"),
                required,
            )?;
            if let Some(files) = command.get("files") {
                let files = files.as_object().ok_or_else(|| {
                    invalid_result(format!("value.commands[{index}].files must be an object"))
                })?;
                for (path, blob_ref) in files {
                    collect_optional_blob_ref_data_classes(
                        Some(blob_ref),
                        &format!("value.commands[{index}].files[{path}]"),
                        required,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn collect_optional_blob_ref_data_classes(
    blob_ref: Option<&Value>,
    field: &str,
    required: &mut BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    let Some(blob_ref) = blob_ref else {
        return Ok(());
    };
    let blob_ref = blob_ref
        .as_object()
        .ok_or_else(|| invalid_result(format!("{field} must be an object")))?;
    if let Some(data_classes) = blob_ref.get("data_classes") {
        required.extend(required_string_set(
            Some(data_classes),
            &format!("{field}.data_classes"),
        )?);
    }
    Ok(())
}

fn collect_workspace_listing_data_classes(
    value: &serde_json::Map<String, Value>,
    required: &mut BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    if value.get("kind").and_then(Value::as_str) != Some("workspace_listing") {
        return Ok(());
    }
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_result("workspace_listing entries must be an array"))?;
    for entry in entries {
        let entry = entry
            .as_object()
            .ok_or_else(|| invalid_result("workspace_listing entries must be objects"))?;
        if let Some(data_classes) = entry.get("data_classes") {
            required.extend(required_string_set(
                Some(data_classes),
                "workspace_listing.entries.data_classes",
            )?);
        }
    }
    Ok(())
}

fn collect_score_output_data_classes_from_value(
    value: &Value,
    required: &mut BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    match value {
        Value::Object(object) => {
            if let Some(output) = object
                .get("score")
                .and_then(Value::as_object)
                .and_then(|score| score.get("output"))
                .and_then(Value::as_object)
            {
                collect_output_record_data_classes(output, required)?;
            }
            for value in object.values() {
                collect_score_output_data_classes_from_value(value, required)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for value in values {
                collect_score_output_data_classes_from_value(value, required)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn collect_output_record_data_classes(
    output: &serde_json::Map<String, Value>,
    required: &mut BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    required.extend(required_string_set(
        output.get("data_classes"),
        "score.output.data_classes",
    )?);
    if let Some(blob_ref) = output.get("blob_ref").and_then(Value::as_object)
        && let Some(data_classes) = blob_ref.get("data_classes")
    {
        required.extend(required_string_set(
            Some(data_classes),
            "score.output.blob_ref.data_classes",
        )?);
    }
    if let Some(trace_refs) = output.get("trace_refs") {
        collect_trace_ref_data_classes(trace_refs, "score.output.trace_refs", required)?;
    }
    Ok(())
}

fn collect_trace_ref_data_classes(
    trace_refs: &Value,
    field: &str,
    required: &mut BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    let trace_refs = trace_refs
        .as_array()
        .ok_or_else(|| invalid_result(format!("{field} must be an array")))?;
    for trace_ref in trace_refs {
        let trace_ref = trace_ref
            .as_object()
            .ok_or_else(|| invalid_result(format!("{field} entries must be objects")))?;
        if let Some(data_classes) = trace_ref.get("data_classes") {
            required.extend(required_string_set(
                Some(data_classes),
                &format!("{field}.data_classes"),
            )?);
        }
    }
    Ok(())
}

fn collect_evidence_data_classes_from_value(
    value: &Value,
    receipt_index: &BTreeMap<String, ReceiptAudit>,
    required: &mut BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    match value {
        Value::Object(object)
            if object.get("schema_version").and_then(Value::as_str)
                == Some("leaven.evidence_envelope.v1") =>
        {
            let envelope = EvidenceEnvelopeDocument::from_schema_valid_value(value)?;
            required.extend(envelope.data_classes().iter().cloned());
            required.extend(envelope.public_data_classes().iter().cloned());
            if let Some(private) = envelope.private_data_classes() {
                required.extend(private.iter().cloned());
            }
            required.extend(envelope.trace_data_classes().iter().cloned());
            validate_evidence_source_receipts(&envelope, receipt_index)
        }
        Value::Object(object) => {
            for value in object.values() {
                collect_evidence_data_classes_from_value(value, receipt_index, required)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for value in values {
                collect_evidence_data_classes_from_value(value, receipt_index, required)?;
            }
            Ok(())
        }
        _ => Ok(()),
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

fn inspect_value_receipt<'a>(
    value: &'a serde_json::Map<String, Value>,
    receipt_index: &BTreeMap<String, ReceiptAudit>,
) -> Result<&'a str, PublicSeamError> {
    let value_kind = required_string(value.get("kind"), "value.kind")?;
    if let Some(receipt) = value.get("receipt") {
        let receipt = receipt_id(receipt)?;
        let Some(receipt_kind) = receipt_index.get(receipt) else {
            return Err(invalid_result(format!(
                "value references missing receipt `{receipt}`"
            )));
        };
        if expected_receipt_kind(value_kind).is_some_and(|expected| receipt_kind.kind != expected) {
            return Err(invalid_result(format!(
                "value kind `{value_kind}` cannot reference `{}` receipt",
                receipt_kind.kind
            )));
        }
    }
    Ok(value_kind)
}

include!("result/audit.rs");
