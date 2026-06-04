use leaven_kernel::{EvaluationRequestId, ProposalBatchId};
use serde_json::Value;
use uuid::Uuid;

use super::RunBoundGraphEffectError;

pub(super) struct EventEmitWrite<'a> {
    pub(super) name: &'a str,
    pub(super) write: &'a Value,
    pub(super) event_kind: &'a str,
    pub(super) payload_schema: &'a str,
    pub(super) payload: &'a Value,
    pub(super) visibility: &'a str,
}

pub(super) fn request_evaluation_write(params: &Value) -> Result<(), RunBoundGraphEffectError> {
    let ops = params
        .get("ops")
        .and_then(Value::as_array)
        .ok_or(RunBoundGraphEffectError::MissingEvaluationRequestWrite)?;
    for op in ops {
        let Some(write) = op.get("write") else {
            continue;
        };
        if write.get("kind").and_then(Value::as_str) == Some("request_evaluation") {
            return Ok(());
        }
    }
    Err(RunBoundGraphEffectError::MissingEvaluationRequestWrite)
}

pub(super) fn proposal_batch_id(
    params: &Value,
) -> Result<ProposalBatchId, RunBoundGraphEffectError> {
    let ops = params
        .get("ops")
        .and_then(Value::as_array)
        .ok_or(RunBoundGraphEffectError::MissingApplyWrite)?;
    for op in ops {
        let Some(write) = op.get("write") else {
            continue;
        };
        if write.get("kind").and_then(Value::as_str) == Some("apply_proposal_batch") {
            let public_ref = write
                .get("proposal_batch")
                .and_then(Value::as_str)
                .ok_or(RunBoundGraphEffectError::InvalidProposalBatchRef)?;
            let uuid = public_ref
                .strip_prefix("pb_")
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or(RunBoundGraphEffectError::InvalidProposalBatchRef)?;
            return Ok(ProposalBatchId::from_uuid(uuid));
        }
    }
    Err(RunBoundGraphEffectError::MissingApplyWrite)
}

pub(super) fn evaluation_request_id(
    params: &Value,
) -> Result<EvaluationRequestId, RunBoundGraphEffectError> {
    let ops = params
        .get("ops")
        .and_then(Value::as_array)
        .ok_or(RunBoundGraphEffectError::MissingAssessmentWrite)?;
    for op in ops {
        let Some(write) = op.get("write") else {
            continue;
        };
        if write.get("kind").and_then(Value::as_str) == Some("submit_assessments") {
            let public_ref = write
                .get("evaluation_request_id")
                .and_then(Value::as_str)
                .ok_or(RunBoundGraphEffectError::InvalidEvaluationRequestRef)?;
            let uuid = public_ref
                .strip_prefix("evalreq_")
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or(RunBoundGraphEffectError::InvalidEvaluationRequestRef)?;
            return Ok(EvaluationRequestId::from_uuid(uuid));
        }
    }
    Err(RunBoundGraphEffectError::MissingAssessmentWrite)
}

pub(super) fn event_emit_write(
    params: &Value,
) -> Result<EventEmitWrite<'_>, RunBoundGraphEffectError> {
    let ops = params
        .get("ops")
        .and_then(Value::as_array)
        .ok_or(RunBoundGraphEffectError::MissingEventWrite)?;
    for op in ops {
        let Some(write) = op.get("write") else {
            continue;
        };
        if write.get("kind").and_then(Value::as_str) == Some("emit_run_event") {
            return Ok(EventEmitWrite {
                name: string_field(op, "name")?,
                write,
                event_kind: string_field(write, "event_kind")?,
                payload_schema: string_field(write, "payload_schema")?,
                payload: write
                    .get("payload")
                    .ok_or(RunBoundGraphEffectError::MissingValue { field: "payload" })?,
                visibility: string_field(write, "visibility")?,
            });
        }
    }
    Err(RunBoundGraphEffectError::MissingEventWrite)
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
