//! RunContext-backed host effects for worker-initiated graph writes.
//!
//! This module is intentionally narrow: it handles graph-write callbacks by
//! routing worker requests through `RunContext`, then returning receipt-bound
//! public-seam extension results. It does not mutate `RunGraph` directly and
//! does not make the bridge crate a general engine facade.

use std::cell::RefCell;
use std::collections::BTreeMap;

use leaven_acp::{AcpEffectHost, AcpTransportError, AcpTransportResult};
use leaven_core::{Assessment, EvaluationRequest, OptimizationProblem};
use leaven_engine::{ProposalBatchReport, RunContext, RunEvent};
use leaven_kernel::{EvaluationRequestId, EvaluatorId, Fingerprint, Metered, ProposalBatchId};
use leaven_public_seam::LockedMethod;
use leaven_run::{
    PublicAssessmentWriteReceiptContext, PublicAssessmentWriteReceiptProjectionError,
    PublicEvaluationJobContext, PublicEvaluationJobProjectionError,
    PublicProposalWriteReceiptContext, PublicProposalWriteReceiptProjectionError,
};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

/// Host-side effect handler for worker-initiated graph-write callbacks.
pub struct RunContextGraphEffectHost<'context, 'run, P: OptimizationProblem> {
    context: RefCell<&'context mut RunContext<'run, P>>,
    batches: BTreeMap<ProposalBatchId, ProposalBatchReport>,
    assessment_submitter: Option<Box<AssessmentSubmitter<'context, P>>>,
    evaluation_requester: Option<Box<EvaluationRequester<'context>>>,
    capability_fingerprint: String,
    policy_fingerprint: String,
    base_revision: String,
    final_revision: String,
    started_at: String,
    completed_at: String,
}

type AssessmentSubmitter<'context, P> =
    dyn Fn(&Value) -> Result<Metered<Vec<Assessment<P>>>, String> + 'context;
type EvaluationRequester<'context> =
    dyn Fn(&Value) -> Result<ExternalEvaluationRequest, String> + 'context;

/// Typed evaluation request produced by a host-owned public payload lowerer.
pub struct ExternalEvaluationRequest {
    /// Evaluator identity that will own the later assessments.
    pub evaluator: EvaluatorId,
    /// Runtime fingerprint for the evaluator/job identity.
    pub evaluator_fingerprint: Fingerprint,
    /// Typed engine evaluation request.
    pub request: EvaluationRequest,
}

impl<'context, 'run, P: OptimizationProblem> RunContextGraphEffectHost<'context, 'run, P> {
    /// Binds a mutable RunContext and the proposal batches workers may apply.
    pub fn new(
        context: &'context mut RunContext<'run, P>,
        batches: impl IntoIterator<Item = ProposalBatchReport>,
        capability_fingerprint: impl Into<String>,
        policy_fingerprint: impl Into<String>,
        base_revision: impl Into<String>,
        final_revision: impl Into<String>,
    ) -> Self {
        Self {
            context: RefCell::new(context),
            batches: batches
                .into_iter()
                .map(|batch| (batch.batch_id, batch))
                .collect(),
            assessment_submitter: None,
            evaluation_requester: None,
            capability_fingerprint: capability_fingerprint.into(),
            policy_fingerprint: policy_fingerprint.into(),
            base_revision: base_revision.into(),
            final_revision: final_revision.into(),
            started_at: "2026-06-03T00:00:00Z".to_owned(),
            completed_at: "2026-06-03T00:00:01Z".to_owned(),
        }
    }

    /// Installs host-side lowering for public assessment payloads.
    ///
    /// The closure is deliberately typed in terms of the problem's
    /// `Assessment<P>` so the bridge cannot turn public JSON into graph state
    /// without a host-owned parser for the concrete evidence type.
    #[must_use]
    pub fn with_assessment_submitter(
        mut self,
        submitter: impl Fn(&Value) -> Result<Metered<Vec<Assessment<P>>>, String> + 'context,
    ) -> Self {
        self.assessment_submitter = Some(Box::new(submitter));
        self
    }

    /// Installs host-side lowering for public evaluation request payloads.
    ///
    /// The closure returns typed engine request identity. The bridge records it
    /// through `RunContext`; it does not treat public JSON ids as graph state.
    #[must_use]
    pub fn with_evaluation_requester(
        mut self,
        requester: impl Fn(&Value) -> Result<ExternalEvaluationRequest, String> + 'context,
    ) -> Self {
        self.evaluation_requester = Some(Box::new(requester));
        self
    }

    fn proposal_apply(&self, params: &Value) -> Result<Value, RunContextGraphEffectHostError> {
        let callback = ProposalApplyCallback::parse(params)?;
        let batch_id = callback.write.batch_id;
        let batch = self
            .batches
            .get(&batch_id)
            .ok_or(RunContextGraphEffectHostError::UnknownBatch(batch_id))?
            .clone();
        let mut context = self.context.borrow_mut();
        let apply = context.apply_batch(batch_id)?;
        let graph = context.graph();
        let plan_result = PublicProposalWriteReceiptContext::new(
            callback.plan_id,
            &self.base_revision,
            &self.final_revision,
            &self.capability_fingerprint,
            &self.policy_fingerprint,
        )
        .with_submit_timing(&self.started_at, &self.started_at)
        .with_apply_timing(&self.started_at, &self.completed_at)
        .proposal_apply_plan_result(&graph, &batch, &apply)?;
        proposal_apply_extension_result(&plan_result)
    }

    fn event_emit(&self, params: &Value) -> Result<Value, RunContextGraphEffectHostError> {
        let callback = EventEmitCallback::parse(params)?;
        let event_id = format!("event_{}", callback.write.name);
        self.context
            .borrow_mut()
            .emit(RunEvent::ExternalEventEmitted {
                event_id: event_id.clone(),
                event_kind: callback.write.event_kind.to_owned(),
                payload_schema: callback.write.payload_schema.to_owned(),
                payload: callback.write.payload.clone(),
                visibility: callback.write.visibility.to_owned(),
            });
        event_emit_extension_result(EventEmitExtensionContext {
            plan_id: callback.plan_id,
            name: callback.write.name,
            write: callback.write.raw,
            event_id: &event_id,
            base_revision: &self.base_revision,
            final_revision: &self.final_revision,
            capability_fingerprint: &self.capability_fingerprint,
            policy_fingerprint: &self.policy_fingerprint,
            started_at: &self.started_at,
            completed_at: &self.completed_at,
            returned: callback.returned,
        })
    }

    fn assessment_submit(&self, params: &Value) -> Result<Value, RunContextGraphEffectHostError> {
        let callback = AssessmentSubmitCallback::parse(params)?;
        let submitter = self
            .assessment_submitter
            .as_ref()
            .ok_or(RunContextGraphEffectHostError::MissingAssessmentSubmitter)?;
        let metered =
            submitter(params).map_err(RunContextGraphEffectHostError::AssessmentSubmit)?;
        let mut context = self.context.borrow_mut();
        let report = context.submit_assessments(callback.write.request_id, metered)?;
        let graph = context.graph();
        let plan_result = PublicAssessmentWriteReceiptContext::new(
            callback.plan_id,
            &self.base_revision,
            &self.final_revision,
            &self.capability_fingerprint,
            &self.policy_fingerprint,
        )
        .with_timing(&self.started_at, &self.completed_at)
        .submit_assessments_plan_result(&graph, &report)?;
        assessment_submit_extension_result(&plan_result)
    }

    fn evaluation_request(&self, params: &Value) -> Result<Value, RunContextGraphEffectHostError> {
        let _callback = EvaluationRequestCallback::parse(params)?;
        let requester = self
            .evaluation_requester
            .as_ref()
            .ok_or(RunContextGraphEffectHostError::MissingEvaluationRequester)?;
        let external =
            requester(params).map_err(RunContextGraphEffectHostError::EvaluationRequest)?;
        let mut context = self.context.borrow_mut();
        let request_id = context.request_evaluation(
            external.evaluator,
            external.evaluator_fingerprint,
            external.request,
        )?;
        let graph = context.graph();
        let request = graph
            .evaluation_request(request_id)
            .ok_or(RunContextGraphEffectHostError::RecordedRequestMissing)?;
        let job_context = PublicEvaluationJobContext::new(
            "sc_stage_bridge_evaluation_request",
            &self.base_revision,
            &self.capability_fingerprint,
            &self.policy_fingerprint,
            "2026-06-03T00:10:00Z",
        )
        .with_evaluation_request_receipt_timing(&self.started_at, &self.completed_at);
        let job = job_context.evaluation_job_document(&graph, &request)?;
        let plan_result = job_context.evaluation_request_receipt_plan_result(&job)?;
        evaluation_request_extension_result(&plan_result)
    }
}

impl<P: OptimizationProblem> AcpEffectHost for RunContextGraphEffectHost<'_, '_, P> {
    fn lm_complete(&self, _params: &Value) -> AcpTransportResult<Value> {
        Err(AcpTransportError::EffectUnimplemented {
            method: "leaven/lm.complete".to_owned(),
        })
    }

    fn service(&self, method: LockedMethod, params: &Value) -> AcpTransportResult<Value> {
        match method {
            LockedMethod::ProposalApply => self.proposal_apply(params).map_err(protocol),
            LockedMethod::AssessmentSubmit => self.assessment_submit(params).map_err(protocol),
            LockedMethod::EvaluationRequest => self.evaluation_request(params).map_err(protocol),
            LockedMethod::EventEmit => self.event_emit(params).map_err(protocol),
            LockedMethod::LmComplete => self.lm_complete(params),
            other => Err(AcpTransportError::EffectUnimplemented {
                method: other.as_str().to_owned(),
            }),
        }
    }
}

/// Errors from RunContext-backed graph-effect callback handling.
#[derive(Debug, Error)]
pub enum RunContextGraphEffectHostError {
    /// A required string field is missing.
    #[error("{field} must be a string")]
    MissingString {
        /// Field name.
        field: &'static str,
    },
    /// The callback did not carry an apply proposal write.
    #[error("leaven/proposal.apply callback must carry an apply_proposal_batch write")]
    MissingApplyWrite,
    /// The callback did not carry an event write.
    #[error("leaven/event.emit callback must carry an emit_run_event write")]
    MissingEventWrite,
    /// The callback did not carry an assessment write.
    #[error("leaven/assessment.submit callback must carry a submit_assessments write")]
    MissingAssessmentWrite,
    /// The callback did not carry an evaluation request write.
    #[error("leaven/evaluation.request callback must carry a request_evaluation write")]
    MissingEvaluationRequestWrite,
    /// The host has no typed assessment lowerer installed.
    #[error("leaven/assessment.submit callback requires a typed host assessment lowerer")]
    MissingAssessmentSubmitter,
    /// The host has no typed evaluation request lowerer installed.
    #[error("leaven/evaluation.request callback requires a typed host evaluation request lowerer")]
    MissingEvaluationRequester,
    /// Host-side typed assessment lowering refused the payload.
    #[error("assessment submit payload refused by host lowerer: {0}")]
    AssessmentSubmit(String),
    /// Host-side typed evaluation request lowering refused the payload.
    #[error("evaluation request payload refused by host lowerer: {0}")]
    EvaluationRequest(String),
    /// A request was recorded but could not be read back from the graph.
    #[error("recorded evaluation request was not visible after RunContext mutation")]
    RecordedRequestMissing,
    /// The public evaluation request ref is malformed.
    #[error("evaluation_request_id must be an evalreq_<uuid> ref")]
    InvalidEvaluationRequestRef,
    /// A required JSON value is missing.
    #[error("{field} must be present")]
    MissingValue {
        /// Field name.
        field: &'static str,
    },
    /// The public batch ref is malformed.
    #[error("proposal_batch must be a pb_<uuid> ref")]
    InvalidProposalBatchRef,
    /// The batch is not one of the batches registered with the host.
    #[error("proposal batch `{0}` is not registered with the RunContext effect host")]
    UnknownBatch(ProposalBatchId),
    /// RunContext rejected the apply.
    #[error(transparent)]
    RunContext(#[from] leaven_engine::RunContextError),
    /// The graph-backed report failed public-seam projection.
    #[error(transparent)]
    Projection(#[from] PublicProposalWriteReceiptProjectionError),
    /// The graph-backed assessment report failed public-seam projection.
    #[error(transparent)]
    AssessmentProjection(#[from] PublicAssessmentWriteReceiptProjectionError),
    /// The graph-backed evaluation request failed public-seam projection.
    #[error(transparent)]
    EvaluationProjection(#[from] PublicEvaluationJobProjectionError),
    /// Canonical JSON hashing failed.
    #[error("failed to hash public seam receipt preimage: {0}")]
    Hash(String),
}

struct ProposalApplyCallback<'a> {
    plan_id: &'a str,
    write: ProposalApplyWrite,
}

struct ProposalApplyWrite {
    batch_id: ProposalBatchId,
}

impl<'a> ProposalApplyCallback<'a> {
    fn parse(params: &'a Value) -> Result<Self, RunContextGraphEffectHostError> {
        let op = plan_write_op(params, "apply_proposal_batch", || {
            RunContextGraphEffectHostError::MissingApplyWrite
        })?;
        Ok(Self {
            plan_id: op.plan_id,
            write: ProposalApplyWrite {
                batch_id: proposal_batch_id(op.write)?,
            },
        })
    }
}

struct AssessmentSubmitCallback<'a> {
    plan_id: &'a str,
    write: AssessmentSubmitWrite,
}

struct AssessmentSubmitWrite {
    request_id: EvaluationRequestId,
}

impl<'a> AssessmentSubmitCallback<'a> {
    fn parse(params: &'a Value) -> Result<Self, RunContextGraphEffectHostError> {
        let op = plan_write_op(params, "submit_assessments", || {
            RunContextGraphEffectHostError::MissingAssessmentWrite
        })?;
        Ok(Self {
            plan_id: op.plan_id,
            write: AssessmentSubmitWrite {
                request_id: evaluation_request_id(op.write)?,
            },
        })
    }
}

struct EvaluationRequestCallback<'a> {
    _plan_id: &'a str,
    _write: EvaluationRequestWrite,
}

struct EvaluationRequestWrite;

impl<'a> EvaluationRequestCallback<'a> {
    fn parse(params: &'a Value) -> Result<Self, RunContextGraphEffectHostError> {
        let op = plan_write_op(params, "request_evaluation", || {
            RunContextGraphEffectHostError::MissingEvaluationRequestWrite
        })?;
        Ok(Self {
            _plan_id: op.plan_id,
            _write: EvaluationRequestWrite,
        })
    }
}

struct PlanWriteOp<'a> {
    plan_id: &'a str,
    name: &'a str,
    write: &'a Value,
    returned: Option<&'a Value>,
}

fn plan_write_op<'a>(
    params: &'a Value,
    expected_kind: &'static str,
    missing: impl Fn() -> RunContextGraphEffectHostError,
) -> Result<PlanWriteOp<'a>, RunContextGraphEffectHostError> {
    let plan_id = string_field(params, "plan_id")?;
    let ops = params
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(&missing)?;
    for op in ops {
        let Some(write) = op.get("write") else {
            continue;
        };
        if write.get("kind").and_then(Value::as_str) == Some(expected_kind) {
            return Ok(PlanWriteOp {
                plan_id,
                name: string_field(op, "name")?,
                write,
                returned: params.get("return"),
            });
        }
    }
    Err(missing())
}

fn proposal_batch_id(write: &Value) -> Result<ProposalBatchId, RunContextGraphEffectHostError> {
    let public_ref = write
        .get("proposal_batch")
        .and_then(Value::as_str)
        .ok_or(RunContextGraphEffectHostError::InvalidProposalBatchRef)?;
    let uuid = public_ref
        .strip_prefix("pb_")
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(RunContextGraphEffectHostError::InvalidProposalBatchRef)?;
    Ok(ProposalBatchId::from_uuid(uuid))
}

fn evaluation_request_id(
    write: &Value,
) -> Result<EvaluationRequestId, RunContextGraphEffectHostError> {
    let public_ref = write
        .get("evaluation_request_id")
        .and_then(Value::as_str)
        .ok_or(RunContextGraphEffectHostError::InvalidEvaluationRequestRef)?;
    let uuid = public_ref
        .strip_prefix("evalreq_")
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(RunContextGraphEffectHostError::InvalidEvaluationRequestRef)?;
    Ok(EvaluationRequestId::from_uuid(uuid))
}

fn proposal_apply_extension_result(
    plan_result: &Value,
) -> Result<Value, RunContextGraphEffectHostError> {
    let primary = plan_result
        .pointer("/values/apply")
        .cloned()
        .ok_or(RunContextGraphEffectHostError::MissingApplyWrite)?;
    let receipts = plan_result
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or(RunContextGraphEffectHostError::MissingApplyWrite)?
        .iter()
        .filter(|receipt| {
            receipt.get("write_kind").and_then(Value::as_str) == Some("apply_proposal_batch")
        })
        .cloned()
        .collect::<Vec<_>>();
    Ok(json!({
        "method": "leaven/proposal.apply",
        "primary": primary,
        "receipts": receipts,
        "redactions": plan_result.get("redactions").cloned().unwrap_or_else(|| json!([])),
        "capability_fingerprint": plan_result.get("capability_fingerprint").cloned().unwrap_or_else(|| json!("fp_cap_sha256_stage_bridge")),
        "policy_fingerprint": plan_result.get("policy_fingerprint").cloned().unwrap_or_else(|| json!("fp_policy_sha256_stage_bridge")),
        "data_classes": ["public"]
    }))
}

fn assessment_submit_extension_result(
    plan_result: &Value,
) -> Result<Value, RunContextGraphEffectHostError> {
    let primary = plan_result
        .pointer("/values/assessment_batch")
        .cloned()
        .ok_or(RunContextGraphEffectHostError::MissingAssessmentWrite)?;
    let receipts = plan_result
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or(RunContextGraphEffectHostError::MissingAssessmentWrite)?
        .iter()
        .filter(|receipt| {
            receipt.get("write_kind").and_then(Value::as_str) == Some("submit_assessments")
        })
        .cloned()
        .collect::<Vec<_>>();
    Ok(json!({
        "method": "leaven/assessment.submit",
        "primary": primary,
        "receipts": receipts,
        "redactions": plan_result.get("redactions").cloned().unwrap_or_else(|| json!([])),
        "capability_fingerprint": plan_result.get("capability_fingerprint").cloned().unwrap_or_else(|| json!("fp_cap_sha256_stage_bridge")),
        "policy_fingerprint": plan_result.get("policy_fingerprint").cloned().unwrap_or_else(|| json!("fp_policy_sha256_stage_bridge")),
        "data_classes": ["public"]
    }))
}

fn evaluation_request_extension_result(
    plan_result: &Value,
) -> Result<Value, RunContextGraphEffectHostError> {
    let primary = plan_result
        .pointer("/values/evaluation_request")
        .cloned()
        .ok_or(RunContextGraphEffectHostError::MissingEvaluationRequestWrite)?;
    let mut receipts = plan_result
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or(RunContextGraphEffectHostError::MissingEvaluationRequestWrite)?
        .iter()
        .filter(|receipt| {
            receipt.get("write_kind").and_then(Value::as_str) == Some("request_evaluation")
        })
        .cloned()
        .collect::<Vec<_>>();
    for receipt in &mut receipts {
        if receipt.get("write_kind").and_then(Value::as_str) == Some("request_evaluation") {
            let op_name = receipt
                .get("op_var")
                .and_then(Value::as_str)
                .unwrap_or("primary");
            receipt["result_hash"] = json!(prefixed_jcs_hash(
                "fp_result_sha256_",
                &json!({
                    "schema_version": "leaven.plan_write_result.v1",
                    "name": op_name,
                    "value": primary
                }),
            )?);
        }
    }
    Ok(json!({
        "method": "leaven/evaluation.request",
        "primary": primary,
        "receipts": receipts,
        "redactions": plan_result.get("redactions").cloned().unwrap_or_else(|| json!([])),
        "capability_fingerprint": plan_result.get("capability_fingerprint").cloned().unwrap_or_else(|| json!("fp_cap_sha256_stage_bridge")),
        "policy_fingerprint": plan_result.get("policy_fingerprint").cloned().unwrap_or_else(|| json!("fp_policy_sha256_stage_bridge")),
        "data_classes": ["public"]
    }))
}

struct EventEmitCallback<'a> {
    plan_id: &'a str,
    write: EventEmitWrite<'a>,
    returned: Option<&'a Value>,
}

struct EventEmitWrite<'a> {
    name: &'a str,
    raw: &'a Value,
    event_kind: &'a str,
    payload_schema: &'a str,
    payload: &'a Value,
    visibility: &'a str,
}

struct EventEmitExtensionContext<'a> {
    plan_id: &'a str,
    name: &'a str,
    write: &'a Value,
    event_id: &'a str,
    base_revision: &'a str,
    final_revision: &'a str,
    capability_fingerprint: &'a str,
    policy_fingerprint: &'a str,
    started_at: &'a str,
    completed_at: &'a str,
    returned: Option<&'a Value>,
}

impl<'a> EventEmitCallback<'a> {
    fn parse(params: &'a Value) -> Result<Self, RunContextGraphEffectHostError> {
        let op = plan_write_op(params, "emit_run_event", || {
            RunContextGraphEffectHostError::MissingEventWrite
        })?;
        Ok(Self {
            plan_id: op.plan_id,
            write: EventEmitWrite {
                name: op.name,
                raw: op.write,
                event_kind: string_field(op.write, "event_kind")?,
                payload_schema: string_field(op.write, "payload_schema")?,
                payload: op
                    .write
                    .get("payload")
                    .ok_or(RunContextGraphEffectHostError::MissingValue { field: "payload" })?,
                visibility: string_field(op.write, "visibility")?,
            },
            returned: op.returned,
        })
    }
}

fn event_emit_extension_result(
    context: EventEmitExtensionContext<'_>,
) -> Result<Value, RunContextGraphEffectHostError> {
    let receipt_id = format!("wrec_{}", context.name);
    let request_hash = prefixed_jcs_hash(
        "fp_request_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_request.v1",
            "name": context.name,
            "kind": "emit_run_event",
            "write": context.write,
            "deps": {},
            "dependency_data_classes": [],
            "base_revision": context.base_revision
        }),
    )?;
    let primary = json!({
        "kind": "emit_run_event",
        "event_id": context.event_id,
        "receipt": receipt_id,
        "data_classes": ["public"],
        "replayability": "fully_managed"
    });
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_result.v1",
            "name": context.name,
            "value": primary
        }),
    )?;
    let receipt = json!({
        "kind": "write",
        "receipt": primary["receipt"],
        "op_var": context.name,
        "started_at": context.started_at,
        "completed_at": context.completed_at,
        "write_kind": "emit_run_event",
        "request_hash": request_hash,
        "result_hash": result_hash,
        "base_revision": context.base_revision,
        "committed_revision": context.final_revision,
        "status": "succeeded",
        "event_id": context.event_id
    });
    Ok(json!({
        "method": "leaven/event.emit",
        "primary": primary,
        "receipts": [receipt],
        "redactions": [],
        "capability_fingerprint": context.capability_fingerprint,
        "policy_fingerprint": context.policy_fingerprint,
        "data_classes": ["public"],
        "plan_id": context.plan_id,
        "return": context.returned.cloned().unwrap_or_else(|| json!([]))
    }))
}

fn prefixed_jcs_hash(
    prefix: &str,
    value: &Value,
) -> Result<String, RunContextGraphEffectHostError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value)
        .map_err(|error| RunContextGraphEffectHostError::Hash(error.to_string()))?;
    Ok(format!("{prefix}{digest}"))
}

fn string_field<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<&'a str, RunContextGraphEffectHostError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(RunContextGraphEffectHostError::MissingString { field })
}

fn protocol(error: RunContextGraphEffectHostError) -> AcpTransportError {
    AcpTransportError::Protocol {
        message: error.to_string(),
    }
}
