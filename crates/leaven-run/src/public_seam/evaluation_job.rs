use leaven_core::{
    AssessmentGranularity, EvaluationPurpose, EvaluationRequest, OptimizationProblem,
};
use leaven_engine::{EvaluationRequestView, RunGraphView};
use leaven_kernel::{
    CandidateId, CaseId, EvaluationRequestId, Fingerprint, ResolvedEvaluationSetId, RunId,
};
use serde_json::{Map, Value, json};
use thiserror::Error;

/// Public-seam fields supplied by the external-worker dispatch layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicEvaluationJobContext {
    stage_call_id: String,
    base_revision: String,
    capability_fingerprint: String,
    policy_fingerprint: String,
    deadline_at: String,
    receipt_started_at: Option<String>,
    receipt_completed_at: Option<String>,
    evaluation_attempt_id: Option<String>,
    target_egress_policy_ref: Option<String>,
}

impl PublicEvaluationJobContext {
    /// Creates dispatch-layer identity for a public-seam evaluation job.
    #[must_use]
    pub fn new(
        stage_call_id: impl Into<String>,
        base_revision: impl Into<String>,
        capability_fingerprint: impl Into<String>,
        policy_fingerprint: impl Into<String>,
        deadline_at: impl Into<String>,
    ) -> Self {
        Self {
            stage_call_id: stage_call_id.into(),
            base_revision: base_revision.into(),
            capability_fingerprint: capability_fingerprint.into(),
            policy_fingerprint: policy_fingerprint.into(),
            deadline_at: deadline_at.into(),
            receipt_started_at: None,
            receipt_completed_at: None,
            evaluation_attempt_id: None,
            target_egress_policy_ref: None,
        }
    }

    /// Adds a public evaluation-attempt id.
    #[must_use]
    pub fn with_evaluation_attempt_id(mut self, id: impl Into<String>) -> Self {
        self.evaluation_attempt_id = Some(id.into());
        self
    }

    /// Adds audit timing for the `request_evaluation` receipt projection.
    #[must_use]
    pub fn with_evaluation_request_receipt_timing(
        mut self,
        started_at: impl Into<String>,
        completed_at: impl Into<String>,
    ) -> Self {
        self.receipt_started_at = Some(started_at.into());
        self.receipt_completed_at = Some(completed_at.into());
        self
    }

    /// Adds a target-egress policy fingerprint for evaluator target reads.
    #[must_use]
    pub fn with_target_egress_policy_ref(mut self, policy: impl Into<String>) -> Self {
        self.target_egress_policy_ref = Some(policy.into());
        self
    }

    /// Projects a recorded engine evaluation request into the locked public-seam job document.
    ///
    /// The returned value still needs validation by `leaven-public-seam`; this
    /// helper only lowers runtime-owned identity into the active wire shape.
    pub fn evaluation_job_document<P>(
        &self,
        graph: &RunGraphView<'_, P>,
        request: &EvaluationRequestView<'_>,
    ) -> Result<Value, PublicEvaluationJobProjectionError>
    where
        P: OptimizationProblem,
    {
        let mut document = Map::from_iter([
            (
                "schema_version".to_owned(),
                json!("leaven.evaluation_job.v1"),
            ),
            ("run".to_owned(), json!(run_ref(graph.run_id()))),
            ("stage_call_id".to_owned(), json!(self.stage_call_id)),
            (
                "evaluation_request_id".to_owned(),
                json!(evaluation_request_ref(request.id())),
            ),
            ("base_revision".to_owned(), json!(self.base_revision)),
            ("deadline_at".to_owned(), json!(self.deadline_at)),
            ("kind".to_owned(), kind_value(request.request())),
            (
                "resolved_set".to_owned(),
                resolved_set_value(request.resolved_set()),
            ),
            (
                "granularity".to_owned(),
                json!(granularity_value(request.request())?),
            ),
            (
                "purpose".to_owned(),
                json!(purpose_value(request.request())),
            ),
            (
                "capability_fingerprint".to_owned(),
                json!(self.capability_fingerprint),
            ),
            (
                "evaluator_id".to_owned(),
                json!(request.evaluator().as_str()),
            ),
            (
                "evaluator_fingerprint".to_owned(),
                json!(runtime_fingerprint(request.evaluator_fingerprint())),
            ),
        ]);
        if let Some(id) = &self.evaluation_attempt_id {
            document.insert("evaluation_attempt_id".to_owned(), json!(id));
        }
        if let Some(policy) = &self.target_egress_policy_ref {
            document.insert("target_egress_policy_ref".to_owned(), json!(policy));
        }
        Ok(Value::Object(document))
    }

    /// Projects a public-seam evaluation job into an evaluation-request receipt Plan Result.
    pub fn evaluation_request_receipt_plan_result(
        &self,
        job: &Value,
    ) -> Result<Value, PublicEvaluationJobProjectionError> {
        let binding = EvaluationRequestBinding::from_job(job)?;
        let started_at = self
            .receipt_started_at
            .as_deref()
            .ok_or_else(|| invalid_job("evaluation request receipt must carry started_at"))?;
        let completed_at = self
            .receipt_completed_at
            .as_deref()
            .ok_or_else(|| invalid_job("evaluation request receipt must carry completed_at"))?;
        let receipt_id = format!("wrec_{}", binding.evaluation_request_id);
        let request_hash = prefixed_jcs_hash("fp_request_sha256_", &binding.request_hash_value())?;
        let result_hash = prefixed_jcs_hash("fp_result_sha256_", &binding.result_hash_value())?;
        Ok(json!({
            "schema_version": "leaven.plan_result.v1",
            "plan_id": format!("evaljob_{}", self.stage_call_id),
            "capability_fingerprint": self.capability_fingerprint,
            "policy_fingerprint": self.policy_fingerprint,
            "base_revision": binding.base_revision,
            "final_revision": binding.base_revision,
            "replayability_summary": "fully_managed",
            "values": {
                "evaluation_request": {
                    "kind": "evaluation_request_receipt",
                    "receipt": receipt_id,
                    "evaluation_request_id": binding.evaluation_request_id,
                    "status": "recorded",
                    "graph_revision": binding.base_revision,
                    "data_classes": ["public"],
                    "replayability": "fully_managed"
                }
            },
            "receipts": [
                {
                    "kind": "write",
                    "write_kind": "request_evaluation",
                    "receipt": receipt_id,
                    "started_at": started_at,
                    "completed_at": completed_at,
                    "request_hash": request_hash,
                    "result_hash": result_hash,
                    "base_revision": binding.base_revision,
                    "committed_revision": binding.base_revision,
                    "status": "succeeded",
                    "evaluation_request_id": binding.evaluation_request_id
                }
            ],
            "redactions": [],
            "charges": [],
            "errors": []
        }))
    }
}

/// Errors raised while projecting runtime evaluation identity into the V1 wire shape.
#[derive(Debug, Error)]
pub enum PublicEvaluationJobProjectionError {
    /// The public evaluation-job schema has no representation for this engine granularity.
    #[error("public-seam evaluation jobs do not support `{granularity}` granularity")]
    UnsupportedGranularity {
        /// Engine granularity name.
        granularity: &'static str,
    },
    /// The supplied public evaluation-job document is missing fields required for receipt binding.
    #[error("invalid public-seam evaluation job document: {message}")]
    InvalidJobDocument {
        /// Human-readable reason.
        message: String,
    },
}

struct EvaluationRequestBinding {
    evaluation_request_id: String,
    kind: String,
    candidate_ids: Vec<String>,
    resolved_set_id: String,
    case_ids: Vec<String>,
    case_count: u64,
    base_revision: String,
    deadline_at: String,
    evaluator_id: String,
    evaluator_fingerprint: String,
    capability_fingerprint: String,
}

impl EvaluationRequestBinding {
    fn from_job(job: &Value) -> Result<Self, PublicEvaluationJobProjectionError> {
        let object = job_object(job)?;
        let kind = object_value(object, "kind")?;
        let resolved_set = object_value(object, "resolved_set")?;
        Ok(Self {
            evaluation_request_id: string_value(object, "evaluation_request_id")?.to_owned(),
            kind: string_value(kind, "kind")?.to_owned(),
            candidate_ids: candidate_ids(kind)?,
            resolved_set_id: string_value(resolved_set, "id")?.to_owned(),
            case_ids: array_values(resolved_set, "case_ids")?
                .iter()
                .map(candidate_or_case_id)
                .collect::<Result<Vec<_>, _>>()?,
            case_count: resolved_set
                .get("case_count")
                .and_then(Value::as_u64)
                .ok_or_else(|| invalid_job("resolved_set.case_count must be an integer"))?,
            base_revision: string_value(object, "base_revision")?.to_owned(),
            deadline_at: string_value(object, "deadline_at")?.to_owned(),
            evaluator_id: string_value(object, "evaluator_id")?.to_owned(),
            evaluator_fingerprint: string_value(object, "evaluator_fingerprint")?.to_owned(),
            capability_fingerprint: string_value(object, "capability_fingerprint")?.to_owned(),
        })
    }

    fn request_hash_value(&self) -> Value {
        json!({
            "schema_version": "leaven.evaluation_request_identity.v1",
            "evaluation_request_id": self.evaluation_request_id,
            "kind": self.kind,
            "candidate_ids": self.candidate_ids,
            "resolved_set_id": self.resolved_set_id,
            "case_ids": self.case_ids,
            "case_count": self.case_count,
            "base_revision": self.base_revision,
            "deadline_at": self.deadline_at,
            "evaluator_id": self.evaluator_id,
            "evaluator_fingerprint": self.evaluator_fingerprint,
            "capability_fingerprint": self.capability_fingerprint
        })
    }

    fn result_hash_value(&self) -> Value {
        json!({
            "schema_version": "leaven.evaluation_request_receipt_result.v1",
            "evaluation_request_id": self.evaluation_request_id,
            "status": "recorded",
            "resolved_set_id": self.resolved_set_id,
            "case_ids": self.case_ids,
            "candidate_ids": self.candidate_ids
        })
    }
}

fn kind_value(request: &EvaluationRequest) -> Value {
    match request {
        EvaluationRequest::Independent { candidates, .. } => json!({
            "kind": "independent",
            "candidates": candidates.iter().copied().map(candidate_ref).collect::<Vec<_>>()
        }),
        EvaluationRequest::Pairwise { left, right, .. } => json!({
            "kind": "pairwise",
            "pairs": [{
                "left": candidate_ref(*left),
                "right": candidate_ref(*right)
            }]
        }),
        EvaluationRequest::Listwise { candidates, .. } => json!({
            "kind": "listwise",
            "candidates": candidates.iter().copied().map(candidate_ref).collect::<Vec<_>>()
        }),
    }
}

fn resolved_set_value(set: &leaven_core::ResolvedEvaluationSet) -> Value {
    let case_ids = set
        .case_ids
        .iter()
        .copied()
        .map(case_ref)
        .collect::<Vec<_>>();
    json!({
        "id": resolved_set_ref(set.id),
        "case_ids": case_ids,
        "case_count": set.case_ids.len(),
        "case_set_version": set.case_set_version.0,
        "partition_summary": {
            "resolved": set.case_ids.len()
        }
    })
}

fn granularity_value(
    request: &EvaluationRequest,
) -> Result<&'static str, PublicEvaluationJobProjectionError> {
    let granularity = match request {
        EvaluationRequest::Independent { granularity, .. }
        | EvaluationRequest::Pairwise { granularity, .. }
        | EvaluationRequest::Listwise { granularity, .. } => granularity,
    };
    match granularity {
        AssessmentGranularity::Aggregate => Ok("aggregate"),
        AssessmentGranularity::PerCase => Ok("per_case"),
        AssessmentGranularity::Both => {
            Err(PublicEvaluationJobProjectionError::UnsupportedGranularity {
                granularity: "both",
            })
        }
    }
}

fn purpose_value(request: &EvaluationRequest) -> &'static str {
    let purpose = match request {
        EvaluationRequest::Independent { purpose, .. }
        | EvaluationRequest::Pairwise { purpose, .. }
        | EvaluationRequest::Listwise { purpose, .. } => purpose,
    };
    match purpose {
        EvaluationPurpose::SeedBaseline
        | EvaluationPurpose::Feedback
        | EvaluationPurpose::Screening
        | EvaluationPurpose::Search => "train",
        EvaluationPurpose::Validation | EvaluationPurpose::Selection => "validation",
        EvaluationPurpose::FinalTest => "test",
        EvaluationPurpose::Probe => "diagnostic",
        EvaluationPurpose::Custom(_) => "custom",
    }
}

fn job_object(value: &Value) -> Result<&Map<String, Value>, PublicEvaluationJobProjectionError> {
    value
        .as_object()
        .ok_or_else(|| invalid_job("evaluation job must be an object"))
}

fn object_value<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Map<String, Value>, PublicEvaluationJobProjectionError> {
    object
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_job(format!("evaluation job {field} must be an object")))
}

fn string_value<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, PublicEvaluationJobProjectionError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid_job(format!("evaluation job {field} must be a string")))
}

fn array_values<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Vec<Value>, PublicEvaluationJobProjectionError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_job(format!("evaluation job {field} must be an array")))
}

fn candidate_ids(
    kind: &Map<String, Value>,
) -> Result<Vec<String>, PublicEvaluationJobProjectionError> {
    match string_value(kind, "kind")? {
        "independent" | "listwise" => array_values(kind, "candidates")?
            .iter()
            .map(candidate_or_case_id)
            .collect(),
        "pairwise" => {
            let pairs = array_values(kind, "pairs")?;
            let mut ids = Vec::with_capacity(pairs.len().saturating_mul(2));
            for pair in pairs {
                let pair = pair
                    .as_object()
                    .ok_or_else(|| invalid_job("pairwise job pair must be an object"))?;
                ids.push(candidate_or_case_id(pair.get("left").ok_or_else(
                    || invalid_job("pairwise job pair must carry left candidate"),
                )?)?);
                ids.push(candidate_or_case_id(pair.get("right").ok_or_else(
                    || invalid_job("pairwise job pair must carry right candidate"),
                )?)?);
            }
            Ok(ids)
        }
        other => Err(invalid_job(format!(
            "unknown evaluation job kind `{other}`"
        ))),
    }
}

fn candidate_or_case_id(value: &Value) -> Result<String, PublicEvaluationJobProjectionError> {
    match value {
        Value::String(id) if !id.trim().is_empty() => Ok(id.to_owned()),
        Value::Object(object) => string_value(object, "id").map(str::to_owned),
        _ => Err(invalid_job("evaluation job ref must carry an id")),
    }
}

fn prefixed_jcs_hash(
    prefix: &str,
    value: &Value,
) -> Result<String, PublicEvaluationJobProjectionError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value)
        .map_err(|error| invalid_job(format!("evaluation request receipt hash failed: {error}")))?;
    Ok(format!("{prefix}{digest}"))
}

fn invalid_job(message: impl Into<String>) -> PublicEvaluationJobProjectionError {
    PublicEvaluationJobProjectionError::InvalidJobDocument {
        message: message.into(),
    }
}

fn run_ref(id: RunId) -> String {
    uuid_ref("run", id.as_uuid())
}

fn candidate_ref(id: CandidateId) -> String {
    uuid_ref("cand", id.as_uuid())
}

fn evaluation_request_ref(id: EvaluationRequestId) -> String {
    uuid_ref("evalreq", id.as_uuid())
}

fn resolved_set_ref(id: ResolvedEvaluationSetId) -> String {
    uuid_ref("rset", id.as_uuid())
}

fn case_ref(id: CaseId) -> String {
    format!("case_{}", id.0)
}

fn uuid_ref(prefix: &str, id: uuid::Uuid) -> String {
    format!("{prefix}_{id}")
}

fn runtime_fingerprint(fingerprint: Fingerprint) -> String {
    let encoded = fingerprint.to_hex();
    format!("fp_runtime_blake3_{encoded}")
}
