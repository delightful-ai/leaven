use std::fmt::Write as _;

use leaven_core::{
    AssessmentGranularity, EvaluationPurpose, EvaluationRequest, OptimizationProblem,
};
use leaven_engine::{EvaluationRequestView, RunEvent, RunGraphView};
use leaven_kernel::{
    CandidateId, CaseId, Cost, EvaluationRequestId, Fingerprint, ResolvedEvaluationSetId, RunId,
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

/// Public-seam call receipt kind for runtime failures projected from engine cost events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicFailedCallKind {
    /// A failed `lm_complete` call.
    LmComplete,
    /// A failed `agent_run` call.
    AgentRun,
    /// A failed `sandbox_exec` call.
    SandboxExec,
}

impl PublicFailedCallKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::LmComplete => "lm_complete",
            Self::AgentRun => "agent_run",
            Self::SandboxExec => "sandbox_exec",
        }
    }

    fn receipt_prefix(self) -> &'static str {
        match self {
            Self::LmComplete => "lmrec",
            Self::AgentRun => "agentrec",
            Self::SandboxExec => "execrec",
        }
    }

    fn error_code(self) -> &'static str {
        match self {
            Self::LmComplete | Self::AgentRun => "provider_error",
            Self::SandboxExec => "stage_runtime_error",
        }
    }
}

/// Public-seam fields supplied while lowering a failed paid runtime call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicFailedCallReceiptContext {
    plan_id: String,
    base_revision: String,
    final_revision: String,
    capability_fingerprint: String,
    policy_fingerprint: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    charged_at: Option<String>,
}

impl PublicFailedCallReceiptContext {
    /// Creates a public-seam failed-call receipt context.
    #[must_use]
    pub fn new(
        plan_id: impl Into<String>,
        base_revision: impl Into<String>,
        capability_fingerprint: impl Into<String>,
        policy_fingerprint: impl Into<String>,
    ) -> Self {
        let base_revision = base_revision.into();
        Self {
            plan_id: plan_id.into(),
            final_revision: base_revision.clone(),
            base_revision,
            capability_fingerprint: capability_fingerprint.into(),
            policy_fingerprint: policy_fingerprint.into(),
            started_at: None,
            completed_at: None,
            charged_at: None,
        }
    }

    /// Overrides the final graph revision when a failed call changed durable state.
    #[must_use]
    pub fn with_final_revision(mut self, final_revision: impl Into<String>) -> Self {
        self.final_revision = final_revision.into();
        self
    }

    /// Adds audit timing for the failed call and matching charge receipt.
    #[must_use]
    pub fn with_timing(
        mut self,
        started_at: impl Into<String>,
        completed_at: impl Into<String>,
        charged_at: impl Into<String>,
    ) -> Self {
        self.started_at = Some(started_at.into());
        self.completed_at = Some(completed_at.into());
        self.charged_at = Some(charged_at.into());
        self
    }

    /// Projects an engine `BudgetCharged` event into a failed call plus charge receipt.
    ///
    /// This is a lowering helper only: engine budget mutation must already have
    /// happened through `RunContext`, and `leaven-public-seam` remains the owner
    /// that validates the returned wire document.
    pub fn failed_paid_call_plan_result(
        &self,
        charge_event: &RunEvent,
        failure_event: &RunEvent,
        kind: PublicFailedCallKind,
        op_var: impl AsRef<str>,
        request: &Value,
        runtime_fingerprint: impl AsRef<str>,
    ) -> Result<Value, PublicFailedCallReceiptProjectionError> {
        let RunEvent::BudgetCharged { stage, cost, .. } = charge_event else {
            return Err(PublicFailedCallReceiptProjectionError::NotBudgetChargeEvent);
        };
        let RunEvent::Error {
            stage: Some(error_stage),
            error: engine_error,
            ..
        } = failure_event
        else {
            return Err(PublicFailedCallReceiptProjectionError::NotFailureEvent);
        };
        if error_stage != stage {
            return Err(PublicFailedCallReceiptProjectionError::StageMismatch);
        }
        let started_at = self
            .started_at
            .as_deref()
            .ok_or(PublicFailedCallReceiptProjectionError::MissingTiming)?;
        let completed_at = self
            .completed_at
            .as_deref()
            .ok_or(PublicFailedCallReceiptProjectionError::MissingTiming)?;
        let charged_at = self
            .charged_at
            .as_deref()
            .ok_or(PublicFailedCallReceiptProjectionError::MissingTiming)?;
        let cost = public_cost(cost)?;
        let op_var = validated_receipt_suffix(op_var.as_ref())?;
        let runtime_fingerprint = runtime_fingerprint.as_ref();
        if runtime_fingerprint.trim().is_empty() {
            return Err(PublicFailedCallReceiptProjectionError::InvalidRuntimeFingerprint);
        }
        let receipt_id = format!("{}_{}", kind.receipt_prefix(), op_var);
        let charge_id = format!("chargerec_{op_var}");
        let error = json!({
            "code": kind.error_code(),
            "message": engine_error.message,
            "op": op_var,
            "receipt": receipt_id,
            "retryable": true,
            "details": {
                "engine_error_kind": format!("{:?}", engine_error.kind),
                "engine_source_chain": engine_error.source_chain
            }
        });
        let charge_receipts = vec![charge_id.clone()];
        let request_hash = prefixed_jcs_hash_for_failed_call("fp_request_sha256_", request)?;
        let result_hash = prefixed_jcs_hash_for_failed_call(
            "fp_result_sha256_",
            &json!({
                "schema_version": "leaven.plan_call_result.v1",
                "name": op_var,
                "error": error,
                "cost": cost,
                "charge_receipts": charge_receipts
            }),
        )?;
        Ok(json!({
            "schema_version": "leaven.plan_result.v1",
            "plan_id": self.plan_id,
            "capability_fingerprint": self.capability_fingerprint,
            "policy_fingerprint": self.policy_fingerprint,
            "base_revision": self.base_revision,
            "final_revision": self.final_revision,
            "replayability_summary": "has_declared_external_effects",
            "values": {},
            "receipts": [
                {
                    "kind": "call",
                    "receipt": receipt_id,
                    "op_var": op_var,
                    "started_at": started_at,
                    "completed_at": completed_at,
                    "call_kind": kind.as_str(),
                    "request_hash": request_hash,
                    "result_hash": result_hash,
                    "runtime_fingerprint": runtime_fingerprint,
                    "status": "failed",
                    "error": error,
                    "cost": cost,
                    "charge_receipts": charge_receipts
                }
            ],
            "redactions": [],
            "charges": [
                {
                    "receipt": charge_id,
                    "source_receipt": receipt_id,
                    "cost": cost,
                    "ledger_scope": format!("engine:{stage}"),
                    "charged_at": charged_at
                }
            ],
            "errors": [error]
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

/// Errors raised while projecting engine cost events into V1 failed-call receipts.
#[derive(Debug, Error)]
pub enum PublicFailedCallReceiptProjectionError {
    /// The context did not include receipt timing.
    #[error("failed paid call projection requires receipt timing")]
    MissingTiming,
    /// The supplied event was not an engine budget charge.
    #[error("failed paid call projection requires a RunEvent::BudgetCharged event")]
    NotBudgetChargeEvent,
    /// The supplied event was not an engine failure event.
    #[error("failed paid call projection requires a RunEvent::Error event")]
    NotFailureEvent,
    /// The budget charge and failure event came from different stages.
    #[error("failed paid call budget charge and failure stage must match")]
    StageMismatch,
    /// The engine charge carried no public-seam-representable cost.
    #[error("failed paid call projection requires non-zero public-seam cost")]
    EmptyCost,
    /// The engine charge used a cost axis that the locked V1 cost schema cannot represent.
    #[error("engine cost axis `{axis}` is not representable in public-seam V1 cost")]
    UnsupportedCostAxis {
        /// Unsupported cost axis.
        axis: String,
    },
    /// The operation variable cannot be used as a receipt suffix.
    #[error("failed paid call operation name is not a valid receipt suffix")]
    InvalidReceiptSuffix,
    /// The runtime fingerprint was empty.
    #[error("failed paid call runtime fingerprint must be non-empty")]
    InvalidRuntimeFingerprint,
    /// JCS/SHA-256 fingerprint computation failed.
    #[error("failed paid call fingerprinting failed: {message}")]
    Fingerprint {
        /// Human-readable fingerprinting error.
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

fn prefixed_jcs_hash_for_failed_call(
    prefix: &str,
    value: &Value,
) -> Result<String, PublicFailedCallReceiptProjectionError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value).map_err(|error| {
        PublicFailedCallReceiptProjectionError::Fingerprint {
            message: error.to_string(),
        }
    })?;
    Ok(format!("{prefix}{digest}"))
}

fn public_cost(cost: &Cost) -> Result<Value, PublicFailedCallReceiptProjectionError> {
    if !cost.seconds.is_zero() {
        return Err(
            PublicFailedCallReceiptProjectionError::UnsupportedCostAxis {
                axis: "seconds".to_owned(),
            },
        );
    }
    if let Some(axis) = cost
        .other
        .iter()
        .find_map(|(axis, amount)| (!amount.is_zero()).then_some(axis.clone()))
    {
        return Err(PublicFailedCallReceiptProjectionError::UnsupportedCostAxis { axis });
    }
    let mut value = Map::new();
    insert_u64_cost(&mut value, "metric_calls", cost.metric_calls);
    insert_u64_cost(&mut value, "lm_calls", cost.llm_calls);
    insert_u64_cost(&mut value, "input_tokens", cost.prompt_tokens);
    insert_u64_cost(&mut value, "output_tokens", cost.completion_tokens);
    if value.is_empty() {
        return Err(PublicFailedCallReceiptProjectionError::EmptyCost);
    }
    Ok(Value::Object(value))
}

fn insert_u64_cost(value: &mut Map<String, Value>, key: &'static str, amount: u64) {
    if amount > 0 {
        value.insert(key.to_owned(), json!(amount));
    }
}

fn validated_receipt_suffix(value: &str) -> Result<&str, PublicFailedCallReceiptProjectionError> {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
    {
        Ok(value)
    } else {
        Err(PublicFailedCallReceiptProjectionError::InvalidReceiptSuffix)
    }
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
    let mut encoded = String::with_capacity(64);
    for byte in fingerprint.0 {
        write!(&mut encoded, "{byte:02x}").expect("writing to string cannot fail");
    }
    format!("fp_runtime_blake3_{encoded}")
}
