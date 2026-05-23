use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::PublicSeamError;

/// Schema-valid public-seam Plan Result classified by replayability facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResultDocument {
    plan_id: String,
    base_revision: String,
    final_revision: String,
    replayability_summary: Replayability,
    value_kinds: Vec<String>,
    receipt_kinds: Vec<String>,
    error_count: usize,
    charge_count: usize,
    assessment_batch_replayability: Vec<(String, Replayability)>,
}

impl PlanResultDocument {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_result("plan result must be an object"))?;
        let replayability_summary = required_replayability(object.get("replayability_summary"))?;
        let parts = PlanResultParts::from_object(object)?;
        let value_audit = inspect_values(
            parts.values,
            &receipt_index(parts.receipts)?,
            replayability_summary,
        )?;
        let receipt_kinds = inspect_receipts(parts.receipts)?;
        validate_submit_assessment_receipts(parts.receipts, &value_audit.assessment_batch_ids)?;
        Ok(Self {
            plan_id: parts.plan_id.to_owned(),
            base_revision: parts.base_revision.to_owned(),
            final_revision: parts.final_revision.to_owned(),
            replayability_summary,
            value_kinds: value_audit.value_kinds,
            receipt_kinds,
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

    /// Per-assessment replayability carried by assessment batch result values.
    pub fn assessment_batch_replayability(&self) -> &[(String, Replayability)] {
        &self.assessment_batch_replayability
    }
}

struct PlanResultParts<'a> {
    plan_id: &'a str,
    base_revision: &'a str,
    final_revision: &'a str,
    values: &'a serde_json::Map<String, Value>,
    receipts: &'a [Value],
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
            error_count: array_len(object, "errors")?,
            charge_count: array_len(object, "charges")?,
        })
    }
}

struct ValueAudit {
    value_kinds: Vec<String>,
    assessment_batch_replayability: Vec<(String, Replayability)>,
    assessment_batch_ids: BTreeSet<String>,
}

fn inspect_values(
    values: &serde_json::Map<String, Value>,
    receipt_index: &BTreeMap<String, String>,
    replayability_summary: Replayability,
) -> Result<ValueAudit, PublicSeamError> {
    let mut value_kinds = Vec::with_capacity(values.len());
    let mut value_replayability = Vec::with_capacity(values.len());
    let mut assessment_batch_replayability = Vec::new();
    let mut assessment_batch_ids = BTreeSet::new();
    for value in values.values() {
        let value_object = value
            .as_object()
            .ok_or_else(|| invalid_result("plan result value must be an object"))?;
        let value_kind = inspect_value_receipt(value_object, receipt_index)?;
        value_kinds.push(value_kind.to_owned());
        value_replayability.push(required_replayability(value_object.get("replayability"))?);
        if value_kind == "assessment_batch_receipt" {
            inspect_assessment_batch_value(
                value_object,
                &mut assessment_batch_replayability,
                &mut assessment_batch_ids,
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
        assessment_batch_replayability,
        assessment_batch_ids,
    })
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
    assessment_batch_ids: &mut BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    let batch_rollup = inspect_assessment_batch(batch, replayability)?;
    let value_replayability = required_replayability(batch.get("replayability"))?;
    if value_replayability != batch_rollup {
        return Err(invalid_result(
            "assessment batch replayability must roll up per-assessment replayability",
        ));
    }
    assessment_batch_ids.extend(required_string_set(
        batch.get("assessment_ids"),
        "assessment_ids",
    )?);
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
        receipt_kinds.push(required_string(receipt.get("kind"), "receipt.kind")?.to_owned());
        required_string(receipt.get("started_at"), "receipt.started_at")?;
        required_string(receipt.get("completed_at"), "receipt.completed_at")?;
    }
    Ok(receipt_kinds)
}

fn validate_submit_assessment_receipts(
    receipts: &[Value],
    assessment_batch_ids: &BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    for receipt_assessments in submit_assessment_receipts(receipts)? {
        if !receipt_assessments.is_subset(assessment_batch_ids) {
            return Err(invalid_result(
                "submit_assessments receipt must be backed by assessment batch per-assessment replayability",
            ));
        }
    }
    Ok(())
}

fn submit_assessment_receipts(
    receipts: &[Value],
) -> Result<Vec<BTreeSet<String>>, PublicSeamError> {
    let mut submit_assessments = Vec::new();
    for receipt in receipts {
        let receipt = receipt
            .as_object()
            .ok_or_else(|| invalid_result("plan result receipt must be an object"))?;
        if receipt.get("kind").and_then(Value::as_str) == Some("write")
            && receipt.get("write_kind").and_then(Value::as_str) == Some("submit_assessments")
        {
            submit_assessments.push(required_string_set(
                receipt.get("assessment_ids"),
                "assessment_ids",
            )?);
        }
    }
    Ok(submit_assessments)
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
