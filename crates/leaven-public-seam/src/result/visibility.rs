use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::PublicSeamError;
use crate::evidence::{EvidenceEnvelopeDocument, EvidenceReceiptRef};
use crate::result::audit::ReceiptAudit;
use crate::result::helpers::{invalid_result, required_string_set};

pub(super) fn validate_value_visibility(
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
        collect_optional_blob_ref_data_classes(
            entry.get("blob_ref"),
            "workspace_listing.entries.blob_ref",
            required,
        )?;
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

pub(super) fn collect_trace_ref_data_classes(
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

pub(super) fn validate_evidence_source_receipts(
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
