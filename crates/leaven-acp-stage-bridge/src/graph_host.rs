//! RunContext-backed host effects for worker-initiated graph writes.
//!
//! This module is intentionally narrow: it handles `leaven/proposal.apply`
//! callbacks by applying a previously recorded proposal batch through
//! `RunContext::apply_batch`, then projects the graph-backed report through
//! `leaven-run`'s public-seam receipt helper. It does not mutate `RunGraph`
//! directly and does not make the bridge crate a general engine facade.

use std::cell::RefCell;
use std::collections::BTreeMap;

use leaven_acp::{AcpEffectHost, AcpTransportError, AcpTransportResult};
use leaven_core::OptimizationProblem;
use leaven_engine::{ProposalBatchReport, RunContext};
use leaven_kernel::ProposalBatchId;
use leaven_run::{PublicProposalWriteReceiptContext, PublicProposalWriteReceiptProjectionError};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

/// Host-side effect handler for `leaven/proposal.apply` worker callbacks.
pub struct RunContextProposalApplyHost<'context, 'run, P: OptimizationProblem> {
    context: RefCell<&'context mut RunContext<'run, P>>,
    batches: BTreeMap<ProposalBatchId, ProposalBatchReport>,
    capability_fingerprint: String,
    policy_fingerprint: String,
    base_revision: String,
    final_revision: String,
    started_at: String,
    completed_at: String,
}

impl<'context, 'run, P: OptimizationProblem> RunContextProposalApplyHost<'context, 'run, P> {
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
            capability_fingerprint: capability_fingerprint.into(),
            policy_fingerprint: policy_fingerprint.into(),
            base_revision: base_revision.into(),
            final_revision: final_revision.into(),
            started_at: "2026-06-03T00:00:00Z".to_owned(),
            completed_at: "2026-06-03T00:00:01Z".to_owned(),
        }
    }

    fn proposal_apply(&self, params: &Value) -> Result<Value, RunContextProposalApplyHostError> {
        let plan_id = string_field(params, "plan_id")?;
        let batch_id = proposal_batch_id(params)?;
        let batch = self
            .batches
            .get(&batch_id)
            .ok_or(RunContextProposalApplyHostError::UnknownBatch(batch_id))?
            .clone();
        let mut context = self.context.borrow_mut();
        let apply = context.apply_batch(batch_id)?;
        let graph = context.graph();
        let plan_result = PublicProposalWriteReceiptContext::new(
            plan_id,
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
}

impl<P: OptimizationProblem> AcpEffectHost for RunContextProposalApplyHost<'_, '_, P> {
    fn lm_complete(&self, _params: &Value) -> AcpTransportResult<Value> {
        Err(AcpTransportError::EffectUnimplemented {
            method: "leaven/lm.complete".to_owned(),
        })
    }

    fn service(&self, method: &str, params: &Value) -> AcpTransportResult<Value> {
        match method {
            "leaven/proposal.apply" => self.proposal_apply(params).map_err(protocol),
            "leaven/lm.complete" => self.lm_complete(params),
            other => Err(AcpTransportError::EffectUnimplemented {
                method: other.to_owned(),
            }),
        }
    }
}

/// Errors from RunContext-backed proposal apply callback handling.
#[derive(Debug, Error)]
pub enum RunContextProposalApplyHostError {
    /// A required string field is missing.
    #[error("{field} must be a string")]
    MissingString {
        /// Field name.
        field: &'static str,
    },
    /// The callback did not carry an apply proposal write.
    #[error("leaven/proposal.apply callback must carry an apply_proposal_batch write")]
    MissingApplyWrite,
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
}

fn proposal_batch_id(params: &Value) -> Result<ProposalBatchId, RunContextProposalApplyHostError> {
    let ops = params
        .get("ops")
        .and_then(Value::as_array)
        .ok_or(RunContextProposalApplyHostError::MissingApplyWrite)?;
    for op in ops {
        let Some(write) = op.get("write") else {
            continue;
        };
        if write.get("kind").and_then(Value::as_str) == Some("apply_proposal_batch") {
            let public_ref = write
                .get("proposal_batch")
                .and_then(Value::as_str)
                .ok_or(RunContextProposalApplyHostError::InvalidProposalBatchRef)?;
            let uuid = public_ref
                .strip_prefix("pb_")
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or(RunContextProposalApplyHostError::InvalidProposalBatchRef)?;
            return Ok(ProposalBatchId::from_uuid(uuid));
        }
    }
    Err(RunContextProposalApplyHostError::MissingApplyWrite)
}

fn proposal_apply_extension_result(
    plan_result: &Value,
) -> Result<Value, RunContextProposalApplyHostError> {
    let primary = plan_result
        .pointer("/values/apply")
        .cloned()
        .ok_or(RunContextProposalApplyHostError::MissingApplyWrite)?;
    let receipts = plan_result
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or(RunContextProposalApplyHostError::MissingApplyWrite)?
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

fn string_field<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<&'a str, RunContextProposalApplyHostError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(RunContextProposalApplyHostError::MissingString { field })
}

fn protocol(error: RunContextProposalApplyHostError) -> AcpTransportError {
    AcpTransportError::Protocol {
        message: error.to_string(),
    }
}
