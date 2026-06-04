use leaven_engine::ExternalEventPayload;
use leaven_kernel::{EvaluationRequestId, ProposalBatchId};
use serde_json::Value;
use uuid::Uuid;

use super::RunBoundGraphEffectError;

pub(crate) struct ProposalApplyParams<'a> {
    pub(crate) plan_id: &'a str,
    pub(crate) write: ProposalApplyWrite,
}

pub(crate) struct ProposalApplyWrite {
    pub(crate) proposal_batch_id: ProposalBatchId,
}

/// Parsed `leaven/evaluation.request` callback params.
pub struct EvaluationRequestParams<'a> {
    pub(crate) plan_id: &'a str,
    pub(crate) write: EvaluationRequestWrite<'a>,
}

pub(crate) struct EvaluationRequestWrite<'a> {
    pub(crate) name: &'a str,
    pub(crate) request: &'a Value,
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

    /// Host-domain evaluation request payload.
    #[must_use]
    pub fn request_payload(&self) -> &Value {
        self.write.request
    }
}

/// Parsed `leaven/assessment.submit` callback params.
pub struct AssessmentSubmitParams<'a> {
    pub(crate) plan_id: &'a str,
    pub(crate) write: AssessmentSubmitWrite<'a>,
}

pub(crate) struct AssessmentSubmitWrite<'a> {
    pub(crate) name: &'a str,
    pub(crate) evaluation_request_id: EvaluationRequestId,
    pub(crate) assessments: &'a Value,
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
        self.write.evaluation_request_id
    }

    /// Host-domain assessment payloads.
    #[must_use]
    pub fn assessments_payload(&self) -> &Value {
        self.write.assessments
    }
}

pub(crate) struct EventEmitParams<'a> {
    pub(crate) plan_id: &'a str,
    pub(crate) write: EventEmitWrite<'a>,
    pub(crate) return_values: Option<&'a Value>,
}

pub(crate) struct EventEmitWrite<'a> {
    pub(crate) name: &'a str,
    pub(crate) event_kind: &'a str,
    pub(crate) payload_schema: &'a str,
    pub(crate) payload: ExternalEventPayload,
    pub(crate) visibility: &'a str,
}

pub(super) fn proposal_apply_params(
    params: &Value,
) -> Result<ProposalApplyParams<'_>, RunBoundGraphEffectError> {
    let plan_id = string_field(params, "plan_id")?;
    let op = write_op(params, "apply_proposal_batch", || {
        RunBoundGraphEffectError::MissingApplyWrite
    })?;
    let public_ref = op
        .write
        .get("proposal_batch")
        .and_then(Value::as_str)
        .ok_or(RunBoundGraphEffectError::InvalidProposalBatchRef)?;
    let uuid = public_ref
        .strip_prefix("pb_")
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(RunBoundGraphEffectError::InvalidProposalBatchRef)?;
    Ok(ProposalApplyParams {
        plan_id,
        write: ProposalApplyWrite {
            proposal_batch_id: ProposalBatchId::from_uuid(uuid),
        },
    })
}

pub(super) fn evaluation_request_params(
    params: &Value,
) -> Result<EvaluationRequestParams<'_>, RunBoundGraphEffectError> {
    let plan_id = string_field(params, "plan_id")?;
    let op = write_op(params, "request_evaluation", || {
        RunBoundGraphEffectError::MissingEvaluationRequestWrite
    })?;
    let request = op
        .write
        .get("request")
        .ok_or(RunBoundGraphEffectError::MissingValue { field: "request" })?;
    Ok(EvaluationRequestParams {
        plan_id,
        write: EvaluationRequestWrite {
            name: op.name,
            request,
        },
    })
}

pub(super) fn assessment_submit_params(
    params: &Value,
) -> Result<AssessmentSubmitParams<'_>, RunBoundGraphEffectError> {
    let plan_id = string_field(params, "plan_id")?;
    let op = write_op(params, "submit_assessments", || {
        RunBoundGraphEffectError::MissingAssessmentWrite
    })?;
    let public_ref = op
        .write
        .get("evaluation_request_id")
        .and_then(Value::as_str)
        .ok_or(RunBoundGraphEffectError::InvalidEvaluationRequestRef)?;
    let uuid = public_ref
        .strip_prefix("evalreq_")
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(RunBoundGraphEffectError::InvalidEvaluationRequestRef)?;
    let assessments =
        op.write
            .get("assessments")
            .ok_or(RunBoundGraphEffectError::MissingValue {
                field: "assessments",
            })?;
    Ok(AssessmentSubmitParams {
        plan_id,
        write: AssessmentSubmitWrite {
            name: op.name,
            evaluation_request_id: EvaluationRequestId::from_uuid(uuid),
            assessments,
        },
    })
}

pub(super) fn event_emit_params(
    params: &Value,
) -> Result<EventEmitParams<'_>, RunBoundGraphEffectError> {
    let plan_id = string_field(params, "plan_id")?;
    let op = write_op(params, "emit_run_event", || {
        RunBoundGraphEffectError::MissingEventWrite
    })?;
    Ok(EventEmitParams {
        plan_id,
        write: EventEmitWrite {
            name: op.name,
            event_kind: string_field(op.write, "event_kind")?,
            payload_schema: string_field(op.write, "payload_schema")?,
            payload: external_event_payload(
                op.write
                    .get("payload")
                    .ok_or(RunBoundGraphEffectError::MissingValue { field: "payload" })?,
            )?,
            visibility: string_field(op.write, "visibility")?,
        },
        return_values: params.get("return"),
    })
}

fn external_event_payload(value: &Value) -> Result<ExternalEventPayload, RunBoundGraphEffectError> {
    serde_json::from_value(value.clone())
        .map_err(|error| RunBoundGraphEffectError::InvalidEventPayload(error.to_string()))
}

struct WriteOp<'a> {
    name: &'a str,
    write: &'a Value,
}

fn write_op<'a>(
    params: &'a Value,
    kind: &'static str,
    missing: impl Fn() -> RunBoundGraphEffectError,
) -> Result<WriteOp<'a>, RunBoundGraphEffectError> {
    let ops = params
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(&missing)?;
    for op in ops {
        let Some(write) = op.get("write") else {
            continue;
        };
        if write.get("kind").and_then(Value::as_str) == Some(kind) {
            return Ok(WriteOp {
                name: string_field(op, "name")?,
                write,
            });
        }
    }
    Err(missing())
}

pub(super) fn string_field<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<&'a str, RunBoundGraphEffectError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(RunBoundGraphEffectError::MissingString { field })
}
