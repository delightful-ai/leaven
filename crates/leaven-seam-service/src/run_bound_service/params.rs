use leaven_engine::ExternalEventPayload;
use leaven_kernel::{EvaluationRequestId, ProposalBatchId};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::RunBoundGraphEffectError;

pub(super) struct ProposalApplyParams {
    pub(crate) plan_id: String,
    pub(crate) write: ProposalApplyWrite,
}

/// Parsed `leaven/proposal.submit_batch` callback params.
pub struct ProposalSubmitParams {
    pub(crate) plan_id: String,
    pub(crate) write: ProposalSubmitWrite,
}

pub(crate) struct ProposalSubmitWrite {
    pub(crate) name: String,
    pub(crate) proposals: Value,
}

impl ProposalSubmitParams {
    /// Plan identity carried by the public-seam callback.
    #[must_use]
    pub fn plan_id(&self) -> &str {
        &self.plan_id
    }

    /// Operation name for the proposal submit write.
    #[must_use]
    pub fn op_name(&self) -> &str {
        &self.write.name
    }

    /// Host-domain proposal payloads.
    #[must_use]
    pub fn proposals_payload(&self) -> &Value {
        &self.write.proposals
    }
}

pub(crate) struct ProposalApplyWrite {
    pub(crate) proposal_batch_id: ProposalBatchId,
}

/// Parsed `leaven/evaluation.request` callback params.
pub struct EvaluationRequestParams {
    pub(crate) plan_id: String,
    pub(crate) write: EvaluationRequestWrite,
}

pub(crate) struct EvaluationRequestWrite {
    pub(crate) name: String,
    pub(crate) request: Value,
}

impl EvaluationRequestParams {
    /// Plan identity carried by the public-seam callback.
    #[must_use]
    pub fn plan_id(&self) -> &str {
        &self.plan_id
    }

    /// Operation name for the evaluation request write.
    #[must_use]
    pub fn op_name(&self) -> &str {
        &self.write.name
    }

    /// Host-domain evaluation request payload.
    #[must_use]
    pub fn request_payload(&self) -> &Value {
        &self.write.request
    }
}

/// Parsed `leaven/assessment.submit` callback params.
pub struct AssessmentSubmitParams {
    pub(crate) plan_id: String,
    pub(crate) write: AssessmentSubmitWrite,
}

pub(crate) struct AssessmentSubmitWrite {
    pub(crate) name: String,
    pub(crate) evaluation_request_id: EvaluationRequestId,
    pub(crate) assessments: Value,
}

impl AssessmentSubmitParams {
    /// Plan identity carried by the public-seam callback.
    #[must_use]
    pub fn plan_id(&self) -> &str {
        &self.plan_id
    }

    /// Operation name for the assessment submit write.
    #[must_use]
    pub fn op_name(&self) -> &str {
        &self.write.name
    }

    /// Typed evaluation request identity receiving the assessments.
    #[must_use]
    pub fn evaluation_request_id(&self) -> EvaluationRequestId {
        self.write.evaluation_request_id
    }

    /// Host-domain assessment payloads.
    #[must_use]
    pub fn assessments_payload(&self) -> &Value {
        &self.write.assessments
    }
}

pub(super) struct EventEmitParams {
    pub(crate) plan_id: String,
    pub(crate) write: EventEmitWrite,
    pub(crate) return_values: Option<Value>,
}

pub(crate) struct EventEmitWrite {
    pub(crate) name: String,
    pub(crate) event_kind: String,
    pub(crate) payload_schema: String,
    pub(crate) payload: ExternalEventPayload,
    pub(crate) visibility: String,
}

pub(super) fn proposal_apply_params(
    params: &Value,
) -> Result<ProposalApplyParams, RunBoundGraphEffectError> {
    let plan = callback_plan(params, RunBoundGraphEffectError::MissingApplyWrite)?;
    let plan_id = plan.plan_id.clone();
    let write = plan.apply_proposal_batch()?;
    Ok(ProposalApplyParams { plan_id, write })
}

pub(super) fn proposal_submit_params(
    params: &Value,
) -> Result<ProposalSubmitParams, RunBoundGraphEffectError> {
    let plan = callback_plan(params, RunBoundGraphEffectError::MissingProposalSubmitWrite)?;
    let plan_id = plan.plan_id.clone();
    let write = plan.submit_proposal_batch()?;
    Ok(ProposalSubmitParams { plan_id, write })
}

pub(super) fn evaluation_request_params(
    params: &Value,
) -> Result<EvaluationRequestParams, RunBoundGraphEffectError> {
    let plan = callback_plan(
        params,
        RunBoundGraphEffectError::MissingEvaluationRequestWrite,
    )?;
    let plan_id = plan.plan_id.clone();
    let write = plan.request_evaluation()?;
    Ok(EvaluationRequestParams { plan_id, write })
}

pub(super) fn assessment_submit_params(
    params: &Value,
) -> Result<AssessmentSubmitParams, RunBoundGraphEffectError> {
    let plan = callback_plan(params, RunBoundGraphEffectError::MissingAssessmentWrite)?;
    let plan_id = plan.plan_id.clone();
    let write = plan.submit_assessments()?;
    Ok(AssessmentSubmitParams { plan_id, write })
}

pub(super) fn event_emit_params(
    params: &Value,
) -> Result<EventEmitParams, RunBoundGraphEffectError> {
    let plan = callback_plan(params, RunBoundGraphEffectError::MissingEventWrite)?;
    let plan_id = plan.plan_id.clone();
    let return_values = plan.return_values.clone();
    let write = plan.emit_run_event()?;
    Ok(EventEmitParams {
        plan_id,
        write,
        return_values,
    })
}

#[derive(Deserialize)]
struct CallbackPlan {
    plan_id: String,
    ops: Vec<CallbackOperation>,
    #[serde(default, rename = "return")]
    return_values: Option<Value>,
}

#[derive(Deserialize)]
struct CallbackOperation {
    name: Option<String>,
    write: Option<CallbackWrite>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum CallbackWrite {
    ApplyProposalBatch {
        proposal_batch: String,
    },
    SubmitProposalBatch {
        proposals: Value,
    },
    RequestEvaluation {
        request: Value,
    },
    SubmitAssessments {
        evaluation_request_id: String,
        assessments: Value,
    },
    EmitRunEvent {
        event_kind: String,
        payload_schema: String,
        payload: ExternalEventPayload,
        visibility: String,
    },
    #[serde(other)]
    Other,
}

impl CallbackPlan {
    fn apply_proposal_batch(self) -> Result<ProposalApplyWrite, RunBoundGraphEffectError> {
        for op in self.ops {
            let Some(CallbackWrite::ApplyProposalBatch { proposal_batch }) = op.write else {
                continue;
            };
            let proposal_batch_id = proposal_batch_id(&proposal_batch)?;
            return Ok(ProposalApplyWrite { proposal_batch_id });
        }
        Err(RunBoundGraphEffectError::MissingApplyWrite)
    }

    fn submit_proposal_batch(self) -> Result<ProposalSubmitWrite, RunBoundGraphEffectError> {
        for op in self.ops {
            let Some(CallbackWrite::SubmitProposalBatch { proposals }) = op.write else {
                continue;
            };
            return Ok(ProposalSubmitWrite {
                name: operation_name(op.name)?,
                proposals,
            });
        }
        Err(RunBoundGraphEffectError::MissingProposalSubmitWrite)
    }

    fn request_evaluation(self) -> Result<EvaluationRequestWrite, RunBoundGraphEffectError> {
        for op in self.ops {
            let Some(CallbackWrite::RequestEvaluation { request }) = op.write else {
                continue;
            };
            return Ok(EvaluationRequestWrite {
                name: operation_name(op.name)?,
                request,
            });
        }
        Err(RunBoundGraphEffectError::MissingEvaluationRequestWrite)
    }

    fn submit_assessments(self) -> Result<AssessmentSubmitWrite, RunBoundGraphEffectError> {
        for op in self.ops {
            let Some(CallbackWrite::SubmitAssessments {
                evaluation_request_id: public_evaluation_request_id,
                assessments,
            }) = op.write
            else {
                continue;
            };
            return Ok(AssessmentSubmitWrite {
                name: operation_name(op.name)?,
                evaluation_request_id: evaluation_request_id(&public_evaluation_request_id)?,
                assessments,
            });
        }
        Err(RunBoundGraphEffectError::MissingAssessmentWrite)
    }

    fn emit_run_event(self) -> Result<EventEmitWrite, RunBoundGraphEffectError> {
        for op in self.ops {
            let Some(CallbackWrite::EmitRunEvent {
                event_kind,
                payload_schema,
                payload,
                visibility,
            }) = op.write
            else {
                continue;
            };
            return Ok(EventEmitWrite {
                name: operation_name(op.name)?,
                event_kind,
                payload_schema,
                payload,
                visibility,
            });
        }
        Err(RunBoundGraphEffectError::MissingEventWrite)
    }
}

fn callback_plan(
    params: &Value,
    invalid: RunBoundGraphEffectError,
) -> Result<CallbackPlan, RunBoundGraphEffectError> {
    serde_json::from_value(params.clone()).map_err(|_error| invalid)
}

fn proposal_batch_id(public_ref: &str) -> Result<ProposalBatchId, RunBoundGraphEffectError> {
    let uuid = public_ref
        .strip_prefix("pb_")
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(RunBoundGraphEffectError::InvalidProposalBatchRef)?;
    Ok(ProposalBatchId::from_uuid(uuid))
}

fn evaluation_request_id(
    public_ref: &str,
) -> Result<EvaluationRequestId, RunBoundGraphEffectError> {
    let uuid = public_ref
        .strip_prefix("evalreq_")
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(RunBoundGraphEffectError::InvalidEvaluationRequestRef)?;
    Ok(EvaluationRequestId::from_uuid(uuid))
}

fn operation_name(name: Option<String>) -> Result<String, RunBoundGraphEffectError> {
    name.ok_or(RunBoundGraphEffectError::MissingString { field: "name" })
}
