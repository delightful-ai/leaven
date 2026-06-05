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
use leaven_engine::{ExternalEventPayload, ProposalBatchReport, RunContext, RunEvent};
use leaven_kernel::{EvaluationRequestId, EvaluatorId, Fingerprint, Metered, ProposalBatchId};
use leaven_public_seam::LockedMethod;
use leaven_run::{
    PublicAssessmentWriteReceiptContext, PublicAssessmentWriteReceiptProjectionError,
    PublicEvaluationJobContext, PublicEvaluationJobProjectionError,
    PublicProposalWriteReceiptContext, PublicProposalWriteReceiptProjectionError,
};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::graph_host_projection::{
    assessment_submit_extension_result, evaluation_request_extension_result,
    proposal_apply_extension_result,
};

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

type AssessmentSubmitter<'context, P> = dyn for<'params> Fn(&AssessmentSubmitParams<'params>) -> Result<Metered<Vec<Assessment<P>>>, String>
    + 'context;
type EvaluationRequester<'context> = dyn for<'params> Fn(&EvaluationRequestParams<'params>) -> Result<ExternalEvaluationRequest, String>
    + 'context;

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
    /// Binds a mutable `RunContext` and the proposal batches workers may apply.
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
        submitter: impl for<'params> Fn(
            &AssessmentSubmitParams<'params>,
        ) -> Result<Metered<Vec<Assessment<P>>>, String>
        + 'context,
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
        requester: impl for<'params> Fn(
            &EvaluationRequestParams<'params>,
        ) -> Result<ExternalEvaluationRequest, String>
        + 'context,
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
        event_emit_extension_result(&EventEmitExtensionContext {
            plan_id: callback.plan_id,
            name: callback.write.name,
            event_kind: callback.write.event_kind,
            payload_schema: callback.write.payload_schema,
            payload: &callback.write.payload,
            visibility: callback.write.visibility,
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
        let callback = AssessmentSubmitParams::parse(params)?;
        let submitter = self
            .assessment_submitter
            .as_ref()
            .ok_or(RunContextGraphEffectHostError::MissingAssessmentSubmitter)?;
        let metered =
            submitter(&callback).map_err(RunContextGraphEffectHostError::AssessmentSubmit)?;
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
        let callback = EvaluationRequestParams::parse(params)?;
        let requester = self
            .evaluation_requester
            .as_ref()
            .ok_or(RunContextGraphEffectHostError::MissingEvaluationRequester)?;
        let external =
            requester(&callback).map_err(RunContextGraphEffectHostError::EvaluationRequest)?;
        let mut context = self.context.borrow_mut();
        let request_id = context.request_evaluation(
            &external.evaluator,
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
            LockedMethod::ProposalApply => self
                .proposal_apply(params)
                .map_err(|error| protocol(&error)),
            LockedMethod::AssessmentSubmit => self
                .assessment_submit(params)
                .map_err(|error| protocol(&error)),
            LockedMethod::EvaluationRequest => self
                .evaluation_request(params)
                .map_err(|error| protocol(&error)),
            LockedMethod::EventEmit => self.event_emit(params).map_err(|error| protocol(&error)),
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
    /// The event payload did not match the engine-owned external event payload.
    #[error("leaven/event.emit callback payload is not typed: {0}")]
    InvalidEventPayload(String),
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
    /// A graph-backed public-seam projection had an unexpected shape.
    #[error("graph-backed public-seam projection field `{field}` had invalid shape: {reason}")]
    InvalidProjection {
        /// Field path or semantic field name.
        field: &'static str,
        /// Expected shape or violated invariant.
        reason: &'static str,
    },
    /// The public batch ref is malformed.
    #[error("proposal_batch must be a pb_<uuid> ref")]
    InvalidProposalBatchRef,
    /// The batch is not one of the batches registered with the host.
    #[error("proposal batch `{0}` is not registered with the RunContext effect host")]
    UnknownBatch(ProposalBatchId),
    /// `RunContext` rejected the apply.
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

/// Parsed `leaven/assessment.submit` callback params for host-owned lowering.
pub struct AssessmentSubmitParams<'a> {
    plan_id: &'a str,
    write: AssessmentSubmitWrite<'a>,
}

struct AssessmentSubmitWrite<'a> {
    name: &'a str,
    request_id: EvaluationRequestId,
    assessments: &'a Value,
}

impl AssessmentSubmitParams<'_> {
    /// Plan identity carried by the public-seam callback.
    #[must_use]
    pub fn plan_id(&self) -> &str {
        self.plan_id
    }

    /// Operation name for the assessment submit write.
    #[must_use]
    pub fn op_name(&self) -> &str {
        self.write.name
    }

    /// Typed evaluation request identity receiving the assessments.
    #[must_use]
    pub fn evaluation_request_id(&self) -> EvaluationRequestId {
        self.write.request_id
    }

    /// Host-domain assessment payloads for the owning lowerer to parse.
    #[must_use]
    pub fn assessments_payload(&self) -> &Value {
        self.write.assessments
    }
}

impl<'a> AssessmentSubmitParams<'a> {
    fn parse(params: &'a Value) -> Result<Self, RunContextGraphEffectHostError> {
        let op = plan_write_op(params, "submit_assessments", || {
            RunContextGraphEffectHostError::MissingAssessmentWrite
        })?;
        let assessments =
            op.write
                .get("assessments")
                .ok_or(RunContextGraphEffectHostError::MissingValue {
                    field: "assessments",
                })?;
        Ok(Self {
            plan_id: op.plan_id,
            write: AssessmentSubmitWrite {
                name: op.name,
                request_id: evaluation_request_id(op.write)?,
                assessments,
            },
        })
    }
}

/// Parsed `leaven/evaluation.request` callback params for host-owned lowering.
pub struct EvaluationRequestParams<'a> {
    plan_id: &'a str,
    write: EvaluationRequestWrite<'a>,
}

struct EvaluationRequestWrite<'a> {
    name: &'a str,
    request: &'a Value,
}

impl EvaluationRequestParams<'_> {
    /// Plan identity carried by the public-seam callback.
    #[must_use]
    pub fn plan_id(&self) -> &str {
        self.plan_id
    }

    /// Operation name for the evaluation request write.
    #[must_use]
    pub fn op_name(&self) -> &str {
        self.write.name
    }

    /// Host-domain evaluation request payload for the owning lowerer to parse.
    #[must_use]
    pub fn request_payload(&self) -> &Value {
        self.write.request
    }
}

impl<'a> EvaluationRequestParams<'a> {
    fn parse(params: &'a Value) -> Result<Self, RunContextGraphEffectHostError> {
        let op = plan_write_op(params, "request_evaluation", || {
            RunContextGraphEffectHostError::MissingEvaluationRequestWrite
        })?;
        let request = op
            .write
            .get("request")
            .ok_or(RunContextGraphEffectHostError::MissingValue { field: "request" })?;
        Ok(Self {
            plan_id: op.plan_id,
            write: EvaluationRequestWrite {
                name: op.name,
                request,
            },
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

struct EventEmitCallback<'a> {
    plan_id: &'a str,
    write: EventEmitWrite<'a>,
    returned: Option<&'a Value>,
}

struct EventEmitWrite<'a> {
    name: &'a str,
    event_kind: &'a str,
    payload_schema: &'a str,
    payload: ExternalEventPayload,
    visibility: &'a str,
}

struct EventEmitExtensionContext<'a> {
    plan_id: &'a str,
    name: &'a str,
    event_kind: &'a str,
    payload_schema: &'a str,
    payload: &'a ExternalEventPayload,
    visibility: &'a str,
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
                event_kind: string_field(op.write, "event_kind")?,
                payload_schema: string_field(op.write, "payload_schema")?,
                payload: external_event_payload(
                    op.write
                        .get("payload")
                        .ok_or(RunContextGraphEffectHostError::MissingValue { field: "payload" })?,
                )?,
                visibility: string_field(op.write, "visibility")?,
            },
            returned: op.returned,
        })
    }
}

fn external_event_payload(
    value: &Value,
) -> Result<ExternalEventPayload, RunContextGraphEffectHostError> {
    serde_json::from_value(value.clone())
        .map_err(|error| RunContextGraphEffectHostError::InvalidEventPayload(error.to_string()))
}

fn event_emit_extension_result(
    context: &EventEmitExtensionContext<'_>,
) -> Result<Value, RunContextGraphEffectHostError> {
    let receipt_id = format!("wrec_{}", context.name);
    let request_hash = prefixed_jcs_hash(
        "fp_request_sha256_",
        &EventEmitRequestPreimage {
            schema_version: "leaven.plan_write_request.v1",
            name: context.name,
            kind: "emit_run_event",
            write: EventEmitWriteProjection {
                kind: "emit_run_event",
                event_kind: context.event_kind,
                payload_schema: context.payload_schema,
                payload: context.payload,
                visibility: context.visibility,
            },
            deps: EmptyObject {},
            dependency_data_classes: &[],
            base_revision: context.base_revision,
        },
    )?;
    let primary = EventEmitPrimary {
        kind: "emit_run_event",
        event_id: context.event_id,
        receipt: &receipt_id,
        data_classes: &["public"],
        replayability: "fully_managed",
    };
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &EventEmitResultPreimage {
            schema_version: "leaven.plan_write_result.v1",
            name: context.name,
            value: &primary,
        },
    )?;
    let result = EventEmitExtensionResult {
        method: "leaven/event.emit",
        primary,
        receipts: vec![EventEmitReceipt {
            kind: "write",
            receipt: &receipt_id,
            op_var: context.name,
            started_at: context.started_at,
            completed_at: context.completed_at,
            write_kind: "emit_run_event",
            request_hash: &request_hash,
            result_hash: &result_hash,
            base_revision: context.base_revision,
            committed_revision: context.final_revision,
            status: "succeeded",
            event_id: context.event_id,
        }],
        redactions: &[],
        capability_fingerprint: context.capability_fingerprint,
        policy_fingerprint: context.policy_fingerprint,
        data_classes: &["public"],
        plan_id: context.plan_id,
        returned: EventEmitReturnValues::from(context.returned),
    };
    serde_json::to_value(result)
        .map_err(|error| RunContextGraphEffectHostError::Hash(error.to_string()))
}

fn prefixed_jcs_hash(
    prefix: &str,
    value: &(impl Serialize + ?Sized),
) -> Result<String, RunContextGraphEffectHostError> {
    let value = serde_json::to_value(value)
        .map_err(|error| RunContextGraphEffectHostError::Hash(error.to_string()))?;
    let digest = jcs_canonicalize::sha256_jcs_hex(&value)
        .map_err(|error| RunContextGraphEffectHostError::Hash(error.to_string()))?;
    Ok(format!("{prefix}{digest}"))
}

#[derive(Serialize)]
struct EmptyObject {}

#[derive(Serialize)]
struct EventEmitWriteProjection<'a> {
    kind: &'static str,
    event_kind: &'a str,
    payload_schema: &'a str,
    payload: &'a ExternalEventPayload,
    visibility: &'a str,
}

#[derive(Serialize)]
struct EventEmitRequestPreimage<'a> {
    schema_version: &'static str,
    name: &'a str,
    kind: &'static str,
    write: EventEmitWriteProjection<'a>,
    deps: EmptyObject,
    dependency_data_classes: &'static [&'static str],
    base_revision: &'a str,
}

#[derive(Serialize)]
struct EventEmitPrimary<'a> {
    kind: &'static str,
    event_id: &'a str,
    receipt: &'a str,
    data_classes: &'static [&'static str],
    replayability: &'static str,
}

#[derive(Serialize)]
struct EventEmitResultPreimage<'a> {
    schema_version: &'static str,
    name: &'a str,
    value: &'a EventEmitPrimary<'a>,
}

#[derive(Serialize)]
struct EventEmitReceipt<'a> {
    kind: &'static str,
    receipt: &'a str,
    op_var: &'a str,
    started_at: &'a str,
    completed_at: &'a str,
    write_kind: &'static str,
    request_hash: &'a str,
    result_hash: &'a str,
    base_revision: &'a str,
    committed_revision: &'a str,
    status: &'static str,
    event_id: &'a str,
}

#[derive(Serialize)]
struct EventEmitExtensionResult<'a> {
    method: &'static str,
    primary: EventEmitPrimary<'a>,
    receipts: Vec<EventEmitReceipt<'a>>,
    redactions: &'static [&'static str],
    capability_fingerprint: &'a str,
    policy_fingerprint: &'a str,
    data_classes: &'static [&'static str],
    plan_id: &'a str,
    #[serde(rename = "return")]
    returned: EventEmitReturnValues<'a>,
}

enum EventEmitReturnValues<'a> {
    Empty,
    Values(&'a Value),
}

impl<'a> EventEmitReturnValues<'a> {
    fn from(values: Option<&'a Value>) -> Self {
        values.map_or(Self::Empty, Self::Values)
    }
}

impl Serialize for EventEmitReturnValues<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Empty => <[&str; 0]>::default().serialize(serializer),
            Self::Values(values) => values.serialize(serializer),
        }
    }
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

fn protocol(error: &RunContextGraphEffectHostError) -> AcpTransportError {
    AcpTransportError::Protocol {
        message: error.to_string(),
    }
}
