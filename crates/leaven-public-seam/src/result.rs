use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::PublicSeamError;
use crate::evidence::EvidenceEnvelopeDocument;

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
    receipt_index: &BTreeMap<String, String>,
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

fn validate_value_visibility(
    value_name: &str,
    value: &serde_json::Map<String, Value>,
    value_data_classes: &BTreeSet<String>,
    receipt_index: &BTreeMap<String, String>,
) -> Result<(), PublicSeamError> {
    let mut required = BTreeSet::new();
    collect_score_output_data_classes_from_value(&Value::Object(value.clone()), &mut required)?;
    collect_evidence_data_classes_from_value(
        &Value::Object(value.clone()),
        receipt_index,
        &mut required,
    )?;
    collect_workspace_listing_data_classes(value, &mut required)?;
    for data_class in required {
        if !value_data_classes.contains(&data_class) {
            return Err(invalid_result(format!(
                "result value `{value_name}` data_classes must cover nested evidence data class `{data_class}`"
            )));
        }
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
                required.extend(required_string_set(
                    output.get("data_classes"),
                    "score.output.data_classes",
                )?);
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

fn collect_evidence_data_classes_from_value(
    value: &Value,
    receipt_index: &BTreeMap<String, String>,
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
    receipt_index: &BTreeMap<String, String>,
) -> Result<(), PublicSeamError> {
    validate_evidence_receipts(envelope.read_receipts(), receipt_index, "query", "read")?;
    validate_evidence_receipts(envelope.effect_receipts(), receipt_index, "call", "effect")?;
    validate_evidence_receipts(envelope.write_receipts(), receipt_index, "write", "write")
}

fn validate_evidence_receipts(
    receipts: &[String],
    receipt_index: &BTreeMap<String, String>,
    expected_kind: &str,
    receipt_role: &str,
) -> Result<(), PublicSeamError> {
    for receipt in receipts {
        let Some(kind) = receipt_index.get(receipt) else {
            return Err(invalid_result(format!(
                "evidence {receipt_role} receipt `{receipt}` is missing from plan result receipts"
            )));
        };
        if kind != expected_kind {
            return Err(invalid_result(format!(
                "evidence {receipt_role} receipt `{receipt}` references `{kind}` receipt, expected `{expected_kind}`"
            )));
        }
    }
    Ok(())
}

fn inspect_value_receipt<'a>(
    value: &'a serde_json::Map<String, Value>,
    receipt_index: &BTreeMap<String, String>,
) -> Result<&'a str, PublicSeamError> {
    let value_kind = required_string(value.get("kind"), "value.kind")?;
    if let Some(receipt) = value.get("receipt") {
        let receipt = receipt_id(receipt)?;
        let Some(receipt_kind) = receipt_index.get(receipt) else {
            return Err(invalid_result(format!(
                "value references missing receipt `{receipt}`"
            )));
        };
        if expected_receipt_kind(value_kind).is_some_and(|expected| receipt_kind != expected) {
            return Err(invalid_result(format!(
                "value kind `{value_kind}` cannot reference `{receipt_kind}` receipt"
            )));
        }
    }
    Ok(value_kind)
}

fn inspect_assessment_batch_value(
    batch: &serde_json::Map<String, Value>,
    replayability: &mut Vec<(String, Replayability)>,
    assessment_batches: &mut Vec<AssessmentBatchScope>,
) -> Result<(), PublicSeamError> {
    let batch_rollup = inspect_assessment_batch(batch, replayability)?;
    let value_replayability = required_replayability(batch.get("replayability"))?;
    if value_replayability != batch_rollup {
        return Err(invalid_result(
            "assessment batch replayability must roll up per-assessment replayability",
        ));
    }
    assessment_batches.push(assessment_batch_scope(batch)?);
    Ok(())
}

fn validate_replayability_rollups(
    summary: Replayability,
    value_replayability: &[Replayability],
    assessment_replayability: &[(String, Replayability)],
) -> Result<(), PublicSeamError> {
    if !value_replayability.is_empty() && summary != rollup(value_replayability.iter().copied()) {
        return Err(invalid_result(
            "plan replayability_summary must roll up result value replayability",
        ));
    }
    if !assessment_replayability.is_empty()
        && summary != rollup(assessment_replayability.iter().map(|(_, r)| *r))
    {
        return Err(invalid_result(
            "plan replayability_summary must roll up per-assessment replayability",
        ));
    }
    Ok(())
}

fn inspect_receipts(receipts: &[Value]) -> Result<Vec<String>, PublicSeamError> {
    let mut receipt_kinds = Vec::with_capacity(receipts.len());
    for receipt in receipts {
        let receipt = receipt
            .as_object()
            .ok_or_else(|| invalid_result("plan result receipt must be an object"))?;
        let kind = required_string(receipt.get("kind"), "receipt.kind")?;
        receipt_kinds.push(kind.to_owned());
        required_string(receipt.get("started_at"), "receipt.started_at")?;
        required_string(receipt.get("completed_at"), "receipt.completed_at")?;
        validate_audit_currency_receipt(kind, receipt)?;
    }
    Ok(receipt_kinds)
}

fn validate_audit_currency_receipt(
    kind: &str,
    receipt: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    match kind {
        "query" => {
            required_hash_with_prefix(receipt, "op_hash", "fp_query_sha256_")?;
            required_hash_with_prefix(receipt, "result_hash", "fp_result_sha256_")?;
            required_string(receipt.get("graph_revision"), "receipt.graph_revision")?;
            required_hash_with_prefix(receipt, "read_scope_fingerprint", "fp_scope_sha256_")?;
            required_hash_with_prefix(receipt, "projection_fingerprint", "fp_projection_sha256_")?;
        }
        "call" => {
            required_hash_with_prefix(receipt, "request_hash", "fp_request_sha256_")?;
            required_hash_with_prefix(receipt, "result_hash", "fp_result_sha256_")?;
            required_hash_with_prefix(receipt, "runtime_fingerprint", "fp_runtime_sha256_")?;
        }
        "write" => {
            required_hash_with_prefix(receipt, "request_hash", "fp_request_sha256_")?;
            required_hash_with_prefix(receipt, "result_hash", "fp_result_sha256_")?;
            required_string(receipt.get("base_revision"), "receipt.base_revision")?;
            if receipt.get("write_kind").and_then(Value::as_str) == Some("submit_assessments") {
                validate_submit_assessments_request_hash(receipt)?;
            }
        }
        other => return Err(invalid_result(format!("unknown receipt kind `{other}`"))),
    }
    Ok(())
}

fn validate_submit_assessments_request_hash(
    receipt: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let expected = prefixed_jcs_hash(
        "fp_request_sha256_",
        &json!({
            "schema_version": "leaven.submit_assessments_request.v1",
            "evaluation_request_id": required_string(receipt.get("evaluation_request_id"), "evaluation_request_id")?,
            "assessment_ids": required_string_set(receipt.get("assessment_ids"), "assessment_ids")?
                .into_iter()
                .collect::<Vec<_>>()
        }),
    )?;
    let actual = required_string(receipt.get("request_hash"), "request_hash")?;
    if actual != expected {
        return Err(invalid_result(
            "submit_assessments receipt request_hash does not bind its assessment scope",
        ));
    }
    Ok(())
}

fn validate_result_hash_bindings(
    values: &serde_json::Map<String, Value>,
    receipts: &[Value],
    request_evaluation_policy: RequestEvaluationReceiptPolicy,
) -> Result<(), PublicSeamError> {
    if request_evaluation_policy == RequestEvaluationReceiptPolicy::Reject {
        reject_request_evaluation_receipts_without_context(receipts)?;
    }
    let receipt_objects = receipt_object_index(receipts)?;
    for (name, value) in values {
        let Some(receipt_ref) = value.as_object().and_then(|object| object.get("receipt")) else {
            continue;
        };
        let receipt_id = receipt_id(receipt_ref)?;
        let Some(receipt) = receipt_objects.get(receipt_id) else {
            continue;
        };
        let receipt_kind = required_string(receipt.get("kind"), "receipt.kind")?;
        let op_name = receipt
            .get("op_var")
            .and_then(Value::as_str)
            .unwrap_or(name);
        let Some(schema_version) =
            result_hash_schema(receipt, receipt_id, request_evaluation_policy)?
        else {
            continue;
        };
        let expected = prefixed_jcs_hash(
            "fp_result_sha256_",
            &json!({
                "schema_version": schema_version,
                "name": op_name,
                "value": value
            }),
        )?;
        let actual = required_string(receipt.get("result_hash"), "receipt.result_hash")?;
        if actual != expected {
            return Err(invalid_result(format!(
                "{receipt_kind} receipt `{receipt_id}` result_hash does not bind its result value"
            )));
        }
    }
    Ok(())
}

fn reject_request_evaluation_receipts_without_context(
    receipts: &[Value],
) -> Result<(), PublicSeamError> {
    for receipt in receipts {
        let receipt = receipt
            .as_object()
            .ok_or_else(|| invalid_result("plan result receipt must be an object"))?;
        if receipt.get("kind").and_then(Value::as_str) == Some("write")
            && receipt.get("write_kind").and_then(Value::as_str) == Some("request_evaluation")
        {
            let receipt_id = required_string(receipt.get("receipt"), "receipt.receipt")?;
            return Err(invalid_result(format!(
                "request_evaluation receipt `{receipt_id}` requires evaluation job context"
            )));
        }
    }
    Ok(())
}

fn result_hash_schema(
    receipt: &serde_json::Map<String, Value>,
    receipt_id: &str,
    request_evaluation_policy: RequestEvaluationReceiptPolicy,
) -> Result<Option<&'static str>, PublicSeamError> {
    Ok(
        match required_string(receipt.get("kind"), "receipt.kind")? {
            "query" => Some("leaven.plan_query_result.v1"),
            "call" => Some("leaven.plan_call_result.v1"),
            "write" => match required_string(receipt.get("write_kind"), "receipt.write_kind")? {
                "request_evaluation"
                    if request_evaluation_policy
                        == RequestEvaluationReceiptPolicy::AllowDedicatedValidation =>
                {
                    None
                }
                "request_evaluation" => {
                    return Err(invalid_result(format!(
                        "request_evaluation receipt `{receipt_id}` requires evaluation job context"
                    )));
                }
                _ => Some("leaven.plan_write_result.v1"),
            },
            _ => None,
        },
    )
}

fn required_hash_with_prefix(
    object: &serde_json::Map<String, Value>,
    field: &str,
    prefix: &str,
) -> Result<(), PublicSeamError> {
    let hash = required_string(object.get(field), field)?;
    if !hash.starts_with(prefix) {
        return Err(invalid_result(format!(
            "receipt {field} must use `{prefix}` audit hash role"
        )));
    }
    Ok(())
}

fn validate_submit_assessment_receipts(
    receipts: &[Value],
    assessment_batches: &[AssessmentBatchScope],
) -> Result<(), PublicSeamError> {
    for receipt_scope in submit_assessment_receipts(receipts)? {
        let backed_by_batch = assessment_batches.iter().any(|batch| {
            batch.evaluation_request_id == receipt_scope.evaluation_request_id
                && receipt_scope
                    .assessment_ids
                    .is_subset(&batch.assessment_ids)
        });
        if !backed_by_batch {
            return Err(invalid_result(
                "submit_assessments receipt must be backed by matching assessment batch per-assessment replayability",
            ));
        }
    }
    Ok(())
}

fn validate_failed_call_charges(
    receipts: &[Value],
    charges: &[Value],
) -> Result<(), PublicSeamError> {
    let charge_index = charge_index(charges)?;
    for receipt in receipts {
        let receipt = receipt
            .as_object()
            .ok_or_else(|| invalid_result("plan result receipt must be an object"))?;
        if !is_failed_call_with_cost(receipt) {
            continue;
        }
        let receipt_id = receipt_id(
            receipt
                .get("receipt")
                .ok_or_else(|| invalid_result("call receipt must carry receipt id"))?,
        )?;
        let charge_receipts =
            required_string_set(receipt.get("charge_receipts"), "charge_receipts")?;
        if charge_receipts.is_empty() {
            return Err(invalid_result(
                "failed paid call must carry charge receipts",
            ));
        }
        let mut covered_cost = BTreeMap::new();
        for charge in charge_receipts {
            let Some(charge_record) = charge_index.get(&charge) else {
                return Err(invalid_result(format!(
                    "failed paid call references missing charge receipt `{charge}`"
                )));
            };
            if charge_record.source_receipt != receipt_id {
                return Err(invalid_result(format!(
                    "charge receipt `{charge}` does not point back to call receipt `{receipt_id}`"
                )));
            }
            merge_costs(&mut covered_cost, &charge_record.cost);
        }
        for (field, amount) in numeric_costs(receipt.get("cost")) {
            if covered_cost.get(&field).copied().unwrap_or(0) < amount {
                return Err(invalid_result(format!(
                    "charge receipts do not cover failed call cost `{field}`"
                )));
            }
        }
    }
    Ok(())
}

fn submit_assessment_receipts(
    receipts: &[Value],
) -> Result<Vec<AssessmentBatchScope>, PublicSeamError> {
    let mut submit_assessments = Vec::new();
    for receipt in receipts {
        let receipt = receipt
            .as_object()
            .ok_or_else(|| invalid_result("plan result receipt must be an object"))?;
        if receipt.get("kind").and_then(Value::as_str) == Some("write")
            && receipt.get("write_kind").and_then(Value::as_str) == Some("submit_assessments")
        {
            submit_assessments.push(assessment_batch_scope(receipt)?);
        }
    }
    Ok(submit_assessments)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AssessmentBatchScope {
    evaluation_request_id: String,
    assessment_ids: BTreeSet<String>,
}

fn assessment_batch_scope(
    object: &serde_json::Map<String, Value>,
) -> Result<AssessmentBatchScope, PublicSeamError> {
    Ok(AssessmentBatchScope {
        evaluation_request_id: required_string(
            object.get("evaluation_request_id"),
            "evaluation_request_id",
        )?
        .to_owned(),
        assessment_ids: required_string_set(object.get("assessment_ids"), "assessment_ids")?,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChargeRecord {
    source_receipt: String,
    cost: Value,
}

fn charge_index(charges: &[Value]) -> Result<BTreeMap<String, ChargeRecord>, PublicSeamError> {
    let mut index = BTreeMap::new();
    for charge in charges {
        let charge = charge
            .as_object()
            .ok_or_else(|| invalid_result("charge receipt must be an object"))?;
        let id = required_string(charge.get("receipt"), "charge.receipt")?.to_owned();
        let source = receipt_id(
            charge
                .get("source_receipt")
                .ok_or_else(|| invalid_result("charge receipt must carry source_receipt"))?,
        )?
        .to_owned();
        let cost = charge
            .get("cost")
            .cloned()
            .ok_or_else(|| invalid_result("charge receipt must carry cost"))?;
        if index
            .insert(
                id,
                ChargeRecord {
                    source_receipt: source,
                    cost,
                },
            )
            .is_some()
        {
            return Err(invalid_result("duplicate charge receipt id"));
        }
    }
    Ok(index)
}

fn merge_costs(total: &mut BTreeMap<String, u64>, cost: &Value) {
    for (field, amount) in numeric_costs(Some(cost)) {
        *total.entry(field).or_default() += amount;
    }
}

fn numeric_costs(cost: Option<&Value>) -> BTreeMap<String, u64> {
    let Some(cost) = cost.and_then(Value::as_object) else {
        return BTreeMap::new();
    };
    let mut fields = BTreeMap::new();
    for (field, value) in cost {
        if let Some(amount) = value.as_u64() {
            fields.insert(field.clone(), amount);
        }
    }
    fields
}

fn is_failed_call_with_cost(receipt: &serde_json::Map<String, Value>) -> bool {
    receipt.get("kind").and_then(Value::as_str) == Some("call")
        && receipt.get("status").and_then(Value::as_str) == Some("failed")
        && has_nonzero_cost(receipt.get("cost"))
}

fn has_nonzero_cost(cost: Option<&Value>) -> bool {
    let Some(cost) = cost.and_then(Value::as_object) else {
        return false;
    };
    cost.values()
        .any(|value| value.as_i64().is_some_and(|n| n > 0))
}

fn receipt_index(receipts: &[Value]) -> Result<BTreeMap<String, String>, PublicSeamError> {
    let mut index = BTreeMap::new();
    for receipt in receipts {
        let receipt = receipt
            .as_object()
            .ok_or_else(|| invalid_result("plan result receipt must be an object"))?;
        let id = receipt_id(
            receipt
                .get("receipt")
                .ok_or_else(|| invalid_result("receipt must carry receipt id"))?,
        )?
        .to_owned();
        let kind = required_string(receipt.get("kind"), "receipt.kind")?.to_owned();
        if index.insert(id, kind).is_some() {
            return Err(invalid_result("duplicate operation receipt id"));
        }
    }
    Ok(index)
}

fn receipt_object_index(
    receipts: &[Value],
) -> Result<BTreeMap<String, &serde_json::Map<String, Value>>, PublicSeamError> {
    let mut index = BTreeMap::new();
    for receipt in receipts {
        let receipt = receipt
            .as_object()
            .ok_or_else(|| invalid_result("plan result receipt must be an object"))?;
        let id = receipt_id(
            receipt
                .get("receipt")
                .ok_or_else(|| invalid_result("receipt must carry receipt id"))?,
        )?
        .to_owned();
        if index.insert(id, receipt).is_some() {
            return Err(invalid_result("duplicate operation receipt id"));
        }
    }
    Ok(index)
}

fn receipt_id(value: &Value) -> Result<&str, PublicSeamError> {
    if let Some(receipt) = value.as_str() {
        return Ok(receipt);
    }
    value
        .as_object()
        .and_then(|object| object.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_result("receipt reference must carry a receipt id"))
}

fn expected_receipt_kind(value_kind: &str) -> Option<&'static str> {
    match value_kind {
        "graph_set" | "case_record" | "workspace_file" | "workspace_diff" | "workspace_listing" => {
            Some("query")
        }
        "workspace_handle"
        | "lm_response"
        | "agent_session"
        | "sandbox_exec_result"
        | "human_review_result" => Some("call"),
        "proposal_batch_receipt"
        | "assessment_batch_receipt"
        | "evaluation_request_receipt"
        | "apply_receipt" => Some("write"),
        _ => None,
    }
}

/// Public-seam replayability order used for plan-level roll-up.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Replayability {
    /// Pure graph/case/workspace reads with no external effect.
    PureRead,
    /// Effects are fully managed by Leaven receipts and replay state.
    FullyManaged,
    /// Effects cross a managed external boundary.
    BoundaryManaged,
    /// External effects are declared and auditable but not fully replayable.
    HasDeclaredExternalEffects,
    /// External effects are not fully tracked and dominate the roll-up.
    HasUntrackedExternalEffects,
}

impl Replayability {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "pure_read" => Some(Self::PureRead),
            "fully_managed" => Some(Self::FullyManaged),
            "boundary_managed" => Some(Self::BoundaryManaged),
            "has_declared_external_effects" => Some(Self::HasDeclaredExternalEffects),
            "has_untracked_external_effects" => Some(Self::HasUntrackedExternalEffects),
            _ => None,
        }
    }
}

fn inspect_assessment_batch(
    batch: &serde_json::Map<String, Value>,
    replayability: &mut Vec<(String, Replayability)>,
) -> Result<Replayability, PublicSeamError> {
    let assessment_ids = required_string_set(batch.get("assessment_ids"), "assessment_ids")?;
    let per_assessment = batch
        .get("per_assessment")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid_result("assessment batch result must carry per_assessment replayability")
        })?;
    let mut seen = BTreeSet::new();
    let mut batch_replayability = Vec::with_capacity(per_assessment.len());
    for entry in per_assessment {
        let object = entry
            .as_object()
            .ok_or_else(|| invalid_result("per_assessment entry must be an object"))?;
        let assessment = object
            .get("assessment")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_result("per_assessment entry must carry assessment"))?
            .to_owned();
        let item_replayability = required_replayability(object.get("replayability"))?;
        if !seen.insert(assessment.clone()) {
            return Err(invalid_result("duplicate per_assessment entry"));
        }
        replayability.push((assessment, item_replayability));
        batch_replayability.push(item_replayability);
    }
    if seen != assessment_ids {
        return Err(invalid_result(
            "per_assessment entries must match assessment_ids",
        ));
    }
    Ok(rollup(batch_replayability))
}

fn rollup<I>(items: I) -> Replayability
where
    I: IntoIterator<Item = Replayability>,
{
    items.into_iter().max().unwrap_or(Replayability::PureRead)
}

fn required_replayability(value: Option<&Value>) -> Result<Replayability, PublicSeamError> {
    let raw = required_string(value, "replayability")?;
    Replayability::parse(raw)
        .ok_or_else(|| invalid_result(format!("unknown replayability `{raw}`")))
}

fn required_string<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_result(format!("{field} must be a string")))
}

fn required_string_set(
    value: Option<&Value>,
    field: &str,
) -> Result<BTreeSet<String>, PublicSeamError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_result(format!("{field} must be an array")))?;
    let mut set = BTreeSet::new();
    for value in values {
        let item = value
            .as_str()
            .ok_or_else(|| invalid_result(format!("{field} entries must be strings")))?;
        if !set.insert(item.to_owned()) {
            return Err(invalid_result(format!("{field} entries must be unique")));
        }
    }
    Ok(set)
}

fn optional_string_set(
    value: Option<&Value>,
    field: &str,
) -> Result<BTreeSet<String>, PublicSeamError> {
    match value {
        Some(value) => required_string_set(Some(value), field),
        None => Ok(BTreeSet::new()),
    }
}

fn array_len(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<usize, PublicSeamError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::len)
        .ok_or_else(|| invalid_result(format!("plan result {field} must be an array")))
}

fn invalid_result(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlanResult {
        message: message.into(),
    }
}

fn prefixed_jcs_hash(prefix: &str, value: &Value) -> Result<String, PublicSeamError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value)
        .map_err(|error| invalid_result(format!("plan result hash failed: {error}")))?;
    Ok(format!("{prefix}{digest}"))
}
