use std::collections::BTreeSet;

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
        let plan_id = required_string(object.get("plan_id"), "plan_id")?.to_owned();
        let base_revision =
            required_string(object.get("base_revision"), "base_revision")?.to_owned();
        let final_revision =
            required_string(object.get("final_revision"), "final_revision")?.to_owned();
        let values = object
            .get("values")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_result("plan result values must be an object"))?;
        let receipts = object
            .get("receipts")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_result("plan result receipts must be an array"))?;
        let mut value_kinds = Vec::with_capacity(values.len());
        let mut value_replayability = Vec::with_capacity(values.len());
        let mut assessment_batch_replayability = Vec::new();
        let mut assessment_batch_ids = BTreeSet::new();
        for value in values.values() {
            let value_object = value
                .as_object()
                .ok_or_else(|| invalid_result("plan result value must be an object"))?;
            value_kinds.push(required_string(value_object.get("kind"), "value.kind")?.to_owned());
            value_replayability.push(required_replayability(value_object.get("replayability"))?);
            let Some(batch) = value.as_object().filter(|object| {
                object.get("kind").and_then(Value::as_str) == Some("assessment_batch_receipt")
            }) else {
                continue;
            };
            let batch_rollup =
                inspect_assessment_batch(batch, &mut assessment_batch_replayability)?;
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
        }
        if !value_replayability.is_empty() && replayability_summary != rollup(value_replayability) {
            return Err(invalid_result(
                "plan replayability_summary must roll up result value replayability",
            ));
        }
        if !assessment_batch_replayability.is_empty()
            && replayability_summary
                != rollup(assessment_batch_replayability.iter().map(|(_, r)| *r))
        {
            return Err(invalid_result(
                "plan replayability_summary must roll up per-assessment replayability",
            ));
        }
        let mut receipt_kinds = Vec::with_capacity(receipts.len());
        for receipt in receipts {
            let receipt = receipt
                .as_object()
                .ok_or_else(|| invalid_result("plan result receipt must be an object"))?;
            receipt_kinds.push(required_string(receipt.get("kind"), "receipt.kind")?.to_owned());
            required_string(receipt.get("started_at"), "receipt.started_at")?;
            required_string(receipt.get("completed_at"), "receipt.completed_at")?;
        }
        let error_count = object
            .get("errors")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_result("plan result errors must be an array"))?
            .len();
        let charge_count = object
            .get("charges")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_result("plan result charges must be an array"))?
            .len();
        let submit_assessment_receipts = submit_assessment_receipts(receipts)?;
        for receipt_assessments in submit_assessment_receipts {
            if !receipt_assessments.is_subset(&assessment_batch_ids) {
                return Err(invalid_result(
                    "submit_assessments receipt must be backed by assessment batch per-assessment replayability",
                ));
            }
        }
        Ok(Self {
            plan_id,
            base_revision,
            final_revision,
            replayability_summary,
            value_kinds,
            receipt_kinds,
            error_count,
            charge_count,
            assessment_batch_replayability,
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

fn invalid_result(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlanResult {
        message: message.into(),
    }
}
