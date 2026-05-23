use serde_json::Value;

use crate::PublicSeamError;

/// Schema-valid public-seam evaluation job with semantic identity checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluationJobDocument {
    request_id: String,
    evaluator_id: String,
    evaluator_fingerprint: String,
    capability_fingerprint: String,
    base_revision: String,
    deadline_at: String,
    resolved_set_id: String,
    case_ids: Vec<String>,
    candidate_ids: Vec<String>,
    case_count: u64,
    candidate_count: usize,
    pair_count: usize,
    kind: EvaluationJobKind,
}

impl EvaluationJobDocument {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_evaluation_job("evaluation job must be an object"))?;
        let kind = object
            .get("kind")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_evaluation_job("evaluation job kind must be an object"))?;
        let resolved_set = object
            .get("resolved_set")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                invalid_evaluation_job("evaluation job resolved_set must be an object")
            })?;
        let deadline_at = required_string(object.get("deadline_at"), "deadline_at")?.to_owned();
        let evaluator_fingerprint =
            required_string(object.get("evaluator_fingerprint"), "evaluator_fingerprint")?
                .to_owned();
        let case_count = resolved_set
            .get("case_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid_evaluation_job("resolved_set.case_count must be an integer"))?;
        validate_resolved_case_set(resolved_set, case_count)?;
        let kind_name = required_string(kind.get("kind"), "kind.kind")?;
        let (kind, candidate_ids, pair_count) = match kind_name {
            "independent" => {
                let candidates = required_array(kind.get("candidates"), "kind.candidates")?
                    .iter()
                    .map(candidate_id)
                    .collect::<Result<Vec<_>, _>>()?;
                (EvaluationJobKind::Independent, candidates, 0)
            }
            "pairwise" => {
                let pairs = required_array(kind.get("pairs"), "kind.pairs")?;
                let mut candidates = Vec::with_capacity(pairs.len().saturating_mul(2));
                for pair in pairs {
                    let pair = pair.as_object().ok_or_else(|| {
                        invalid_evaluation_job("pairwise job pairs must be objects")
                    })?;
                    let left = candidate_id(pair.get("left").ok_or_else(|| {
                        invalid_evaluation_job("pairwise job pair must carry left candidate")
                    })?)?;
                    let right = candidate_id(pair.get("right").ok_or_else(|| {
                        invalid_evaluation_job("pairwise job pair must carry right candidate")
                    })?)?;
                    if left == right {
                        return Err(invalid_evaluation_job(
                            "pairwise job pairs must compare distinct candidates",
                        ));
                    }
                    candidates.push(left);
                    candidates.push(right);
                }
                (EvaluationJobKind::Pairwise, candidates, pairs.len())
            }
            "listwise" => {
                let candidates = required_array(kind.get("candidates"), "kind.candidates")?
                    .iter()
                    .map(candidate_id)
                    .collect::<Result<Vec<_>, _>>()?;
                (EvaluationJobKind::Listwise, candidates, 0)
            }
            other => {
                return Err(invalid_evaluation_job(format!(
                    "unknown evaluation job kind `{other}`"
                )));
            }
        };

        Ok(Self {
            request_id: required_string(
                object.get("evaluation_request_id"),
                "evaluation_request_id",
            )?
            .to_owned(),
            evaluator_id: required_string(object.get("evaluator_id"), "evaluator_id")?.to_owned(),
            evaluator_fingerprint,
            capability_fingerprint: required_string(
                object.get("capability_fingerprint"),
                "capability_fingerprint",
            )?
            .to_owned(),
            base_revision: required_string(object.get("base_revision"), "base_revision")?
                .to_owned(),
            deadline_at,
            resolved_set_id: required_string(resolved_set.get("id"), "resolved_set.id")?.to_owned(),
            case_ids: case_ids(resolved_set)?,
            candidate_count: candidate_ids.len(),
            candidate_ids,
            case_count,
            pair_count,
            kind,
        })
    }

    /// Evaluation request id bound to this job.
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    /// Evaluator id selected for this job.
    pub fn evaluator_id(&self) -> &str {
        &self.evaluator_id
    }

    /// Evaluator runtime/schema fingerprint.
    pub fn evaluator_fingerprint(&self) -> &str {
        &self.evaluator_fingerprint
    }

    /// Capability fingerprint authorizing the evaluator job.
    pub fn capability_fingerprint(&self) -> &str {
        &self.capability_fingerprint
    }

    /// Graph revision used as the evaluation base.
    pub fn base_revision(&self) -> &str {
        &self.base_revision
    }

    /// Deadline timestamp for this job.
    pub fn deadline_at(&self) -> &str {
        &self.deadline_at
    }

    /// Resolved case-set id.
    pub fn resolved_set_id(&self) -> &str {
        &self.resolved_set_id
    }

    /// Case ids in the resolved case set.
    pub fn case_ids(&self) -> &[String] {
        &self.case_ids
    }

    /// Candidate ids carried by this job in request order.
    pub fn candidate_ids(&self) -> &[String] {
        &self.candidate_ids
    }

    /// Number of cases in the resolved set.
    pub const fn case_count(&self) -> u64 {
        self.case_count
    }

    /// Number of candidate slots carried by this job.
    pub const fn candidate_count(&self) -> usize {
        self.candidate_count
    }

    /// Number of pairwise comparisons carried by this job.
    pub const fn pair_count(&self) -> usize {
        self.pair_count
    }

    /// Job request shape.
    pub const fn kind(&self) -> EvaluationJobKind {
        self.kind
    }

    fn kind_name(&self) -> &'static str {
        self.kind.as_str()
    }
}

/// Evaluation job request shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvaluationJobKind {
    /// Independent candidate assessment.
    Independent,
    /// Pairwise candidate comparison.
    Pairwise,
    /// Listwise candidate ranking.
    Listwise,
}

impl EvaluationJobKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Independent => "independent",
            Self::Pairwise => "pairwise",
            Self::Listwise => "listwise",
        }
    }
}

/// Plan-result evaluation-request receipt bound to a validated evaluation job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluationRequestReceiptDocument {
    request_id: String,
    receipt_id: String,
    base_revision: String,
    request_hash: String,
    result_hash: String,
    candidate_ids: Vec<String>,
    case_ids: Vec<String>,
}

impl EvaluationRequestReceiptDocument {
    pub(crate) fn from_plan_result(
        job: &EvaluationJobDocument,
        result: &Value,
    ) -> Result<Self, PublicSeamError> {
        let result = result
            .as_object()
            .ok_or_else(|| invalid_evaluation_job("plan result must be an object"))?;
        let values = result
            .get("values")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_evaluation_job("plan result values must be an object"))?;
        let receipts = result
            .get("receipts")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_evaluation_job("plan result receipts must be an array"))?;
        let value = matching_evaluation_request_value(values, job.request_id())?;
        let receipt_id = required_string(value.get("receipt"), "evaluation_request.receipt")?;
        let receipt = matching_evaluation_request_receipt(receipts, receipt_id)?;
        validate_receipt_matches_job(job, value, receipt)?;
        Ok(Self {
            request_id: job.request_id.clone(),
            receipt_id: receipt_id.to_owned(),
            base_revision: job.base_revision.clone(),
            request_hash: required_string(receipt.get("request_hash"), "request_hash")?.to_owned(),
            result_hash: required_string(receipt.get("result_hash"), "result_hash")?.to_owned(),
            candidate_ids: job.candidate_ids.clone(),
            case_ids: job.case_ids.clone(),
        })
    }

    /// Evaluation request id bound to this receipt.
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    /// Operation receipt id for the `request_evaluation` write.
    pub fn receipt_id(&self) -> &str {
        &self.receipt_id
    }

    /// Graph revision used as the request write base.
    pub fn base_revision(&self) -> &str {
        &self.base_revision
    }

    /// Hash of the request identity, including candidate and case-set bindings.
    pub fn request_hash(&self) -> &str {
        &self.request_hash
    }

    /// Hash of the recorded request result.
    pub fn result_hash(&self) -> &str {
        &self.result_hash
    }

    /// Candidate ids bound into the receipted request hash.
    pub fn candidate_ids(&self) -> &[String] {
        &self.candidate_ids
    }

    /// Case ids bound into the receipted request hash.
    pub fn case_ids(&self) -> &[String] {
        &self.case_ids
    }
}

fn validate_resolved_case_set(
    resolved_set: &serde_json::Map<String, Value>,
    case_count: u64,
) -> Result<(), PublicSeamError> {
    let explicit_case_ids = resolved_set.get("case_ids").and_then(Value::as_array);
    if let Some(case_ids) = explicit_case_ids {
        if case_ids.len() as u64 != case_count {
            return Err(invalid_evaluation_job(
                "resolved_set.case_count must match explicit case_ids length",
            ));
        }
        if resolved_set
            .get("case_set_version")
            .and_then(Value::as_str)
            .is_none_or(|version| version.trim().is_empty())
        {
            return Err(invalid_evaluation_job(
                "resolved_set explicit case_ids must carry case_set_version",
            ));
        }
        if resolved_set
            .get("partition_summary")
            .and_then(Value::as_object)
            .is_none_or(serde_json::Map::is_empty)
        {
            return Err(invalid_evaluation_job(
                "resolved_set explicit case_ids must carry partition_summary",
            ));
        }
        return Ok(());
    }
    if case_count > 0 {
        return Err(invalid_evaluation_job(
            "resolved_set with cases must carry partition-resolved explicit case_ids",
        ));
    }
    Ok(())
}

fn matching_evaluation_request_value<'a>(
    values: &'a serde_json::Map<String, Value>,
    request_id: &str,
) -> Result<&'a serde_json::Map<String, Value>, PublicSeamError> {
    let mut matched = None;
    for value in values.values() {
        let value = value
            .as_object()
            .ok_or_else(|| invalid_evaluation_job("plan result value must be an object"))?;
        if value.get("kind").and_then(Value::as_str) == Some("evaluation_request_receipt")
            && value.get("evaluation_request_id").and_then(Value::as_str) == Some(request_id)
            && matched.replace(value).is_some()
        {
            return Err(invalid_evaluation_job(
                "plan result carries multiple matching evaluation request values",
            ));
        }
    }
    matched.ok_or_else(|| {
        invalid_evaluation_job("plan result lacks evaluation request value for job request id")
    })
}

fn matching_evaluation_request_receipt<'a>(
    receipts: &'a [Value],
    receipt_id: &str,
) -> Result<&'a serde_json::Map<String, Value>, PublicSeamError> {
    let mut matched = None;
    for receipt in receipts {
        let receipt = receipt
            .as_object()
            .ok_or_else(|| invalid_evaluation_job("plan result receipt must be an object"))?;
        if receipt.get("receipt").and_then(Value::as_str) == Some(receipt_id)
            && matched.replace(receipt).is_some()
        {
            return Err(invalid_evaluation_job(
                "plan result carries duplicate evaluation request receipt ids",
            ));
        }
    }
    matched.ok_or_else(|| {
        invalid_evaluation_job("plan result lacks write receipt for evaluation request value")
    })
}

fn validate_receipt_matches_job(
    job: &EvaluationJobDocument,
    value: &serde_json::Map<String, Value>,
    receipt: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    require_field_value(value, "status", "recorded")?;
    require_field_value(value, "graph_revision", job.base_revision())?;
    require_field_value(receipt, "kind", "write")?;
    require_field_value(receipt, "write_kind", "request_evaluation")?;
    require_field_value(receipt, "status", "succeeded")?;
    require_field_value(receipt, "evaluation_request_id", job.request_id())?;
    require_field_value(receipt, "base_revision", job.base_revision())?;
    if let Some(committed) = receipt.get("committed_revision").and_then(Value::as_str)
        && committed != job.base_revision()
    {
        return Err(invalid_evaluation_job(
            "evaluation request receipt committed_revision must match job base_revision",
        ));
    }
    let expected_request_hash = evaluation_request_hash(job)?;
    let expected_result_hash = evaluation_request_result_hash(job)?;
    require_field_value(receipt, "request_hash", &expected_request_hash)?;
    require_field_value(receipt, "result_hash", &expected_result_hash)?;
    Ok(())
}

fn evaluation_request_hash(job: &EvaluationJobDocument) -> Result<String, PublicSeamError> {
    prefixed_jcs_hash(
        "fp_request_sha256_",
        &serde_json::json!({
            "schema_version": "leaven.evaluation_request_identity.v1",
            "evaluation_request_id": job.request_id(),
            "kind": job.kind_name(),
            "candidate_ids": job.candidate_ids(),
            "resolved_set_id": job.resolved_set_id(),
            "case_ids": job.case_ids(),
            "case_count": job.case_count(),
            "base_revision": job.base_revision(),
            "deadline_at": job.deadline_at(),
            "evaluator_id": job.evaluator_id(),
            "evaluator_fingerprint": job.evaluator_fingerprint(),
            "capability_fingerprint": job.capability_fingerprint()
        }),
    )
}

fn evaluation_request_result_hash(job: &EvaluationJobDocument) -> Result<String, PublicSeamError> {
    prefixed_jcs_hash(
        "fp_result_sha256_",
        &serde_json::json!({
            "schema_version": "leaven.evaluation_request_receipt_result.v1",
            "evaluation_request_id": job.request_id(),
            "status": "recorded",
            "resolved_set_id": job.resolved_set_id(),
            "case_ids": job.case_ids(),
            "candidate_ids": job.candidate_ids()
        }),
    )
}

fn prefixed_jcs_hash(prefix: &str, value: &Value) -> Result<String, PublicSeamError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value).map_err(|error| {
        invalid_evaluation_job(format!("evaluation request receipt hash failed: {error}"))
    })?;
    Ok(format!("{prefix}{digest}"))
}

fn require_field_value(
    object: &serde_json::Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), PublicSeamError> {
    let actual = required_string(object.get(field), field)?;
    if actual != expected {
        return Err(invalid_evaluation_job(format!(
            "evaluation request receipt {field} must be `{expected}`"
        )));
    }
    Ok(())
}

fn case_ids(resolved_set: &serde_json::Map<String, Value>) -> Result<Vec<String>, PublicSeamError> {
    required_array(resolved_set.get("case_ids"), "resolved_set.case_ids")?
        .iter()
        .map(case_id)
        .collect()
}

fn candidate_id(value: &Value) -> Result<String, PublicSeamError> {
    match value {
        Value::String(id) if !id.trim().is_empty() => Ok(id.to_owned()),
        Value::Object(object) => {
            required_string(object.get("id"), "candidate.id").map(str::to_owned)
        }
        _ => Err(invalid_evaluation_job(
            "evaluation job candidate ref must carry an id",
        )),
    }
}

fn case_id(value: &Value) -> Result<String, PublicSeamError> {
    match value {
        Value::String(id) if !id.trim().is_empty() => Ok(id.to_owned()),
        Value::Object(object) => required_string(object.get("id"), "case.id").map(str::to_owned),
        _ => Err(invalid_evaluation_job(
            "evaluation job case ref must carry an id",
        )),
    }
}

fn required_array<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<&'a Vec<Value>, PublicSeamError> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_evaluation_job(format!("evaluation job {field} must be an array")))
}

fn required_string<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid_evaluation_job(format!("evaluation job {field} must be a string")))
}

fn invalid_evaluation_job(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidEvaluationJob {
        message: message.into(),
    }
}
