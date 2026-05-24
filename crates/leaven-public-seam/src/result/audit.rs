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

struct ReceiptAudit {
    kind: String,
    fingerprint: String,
    trace_data_classes: BTreeSet<String>,
}

fn receipt_index(receipts: &[Value]) -> Result<BTreeMap<String, ReceiptAudit>, PublicSeamError> {
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
        let fingerprint = prefixed_jcs_hash("fp_receipt_sha256_", &Value::Object(receipt.clone()))?;
        let mut trace_data_classes = BTreeSet::new();
        if let Some(trace_refs) = receipt.get("trace_refs") {
            collect_trace_ref_data_classes(
                trace_refs,
                "receipt.trace_refs",
                &mut trace_data_classes,
            )?;
        }
        if index
            .insert(
                id,
                ReceiptAudit {
                    kind,
                    fingerprint,
                    trace_data_classes,
                },
            )
            .is_some()
        {
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
        | "sandbox_exec"
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
