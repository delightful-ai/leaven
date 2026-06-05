use std::collections::BTreeSet;

use leaven_core::OptimizationProblem;
use leaven_engine::{ApplyOutcome, ApplyReport, ProposalBatchReport, RunGraphView};
use leaven_kernel::{CandidateId, ProposalBatchId, ProposalId};
use serde_json::{Value, json};
use thiserror::Error;

/// Public-seam fields supplied while lowering `RunContext` proposal writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicProposalWriteReceiptContext {
    plan_id: String,
    base_revision: String,
    final_revision: String,
    capability_fingerprint: String,
    policy_fingerprint: String,
    submit_started_at: Option<String>,
    submit_completed_at: Option<String>,
    apply_started_at: Option<String>,
    apply_completed_at: Option<String>,
}

impl PublicProposalWriteReceiptContext {
    /// Creates a public-seam proposal write receipt context.
    #[must_use]
    pub fn new(
        plan_id: impl Into<String>,
        base_revision: impl Into<String>,
        final_revision: impl Into<String>,
        capability_fingerprint: impl Into<String>,
        policy_fingerprint: impl Into<String>,
    ) -> Self {
        Self {
            plan_id: plan_id.into(),
            base_revision: base_revision.into(),
            final_revision: final_revision.into(),
            capability_fingerprint: capability_fingerprint.into(),
            policy_fingerprint: policy_fingerprint.into(),
            submit_started_at: None,
            submit_completed_at: None,
            apply_started_at: None,
            apply_completed_at: None,
        }
    }

    /// Adds audit timing for proposal-batch submission.
    #[must_use]
    pub fn with_submit_timing(
        mut self,
        started_at: impl Into<String>,
        completed_at: impl Into<String>,
    ) -> Self {
        self.submit_started_at = Some(started_at.into());
        self.submit_completed_at = Some(completed_at.into());
        self
    }

    /// Adds audit timing for proposal-batch application.
    #[must_use]
    pub fn with_apply_timing(
        mut self,
        started_at: impl Into<String>,
        completed_at: impl Into<String>,
    ) -> Self {
        self.apply_started_at = Some(started_at.into());
        self.apply_completed_at = Some(completed_at.into());
        self
    }

    /// Projects `RunContext` proposal submission/application into locked Plan Result receipts.
    ///
    /// This helper refuses to mint receipts from ids alone: the supplied batch
    /// and every created candidate must already be visible in the engine graph.
    pub fn proposal_submit_plan_result<P>(
        &self,
        graph: &RunGraphView<'_, P>,
        batch: &ProposalBatchReport,
    ) -> Result<Value, PublicProposalWriteReceiptProjectionError>
    where
        P: OptimizationProblem,
    {
        let proposal_ids = graph_proposal_ids(graph, batch)?;
        let submit_receipt = "wrec_submit_proposal_batch".to_owned();
        let proposal_value = json!({
            "kind": "proposal_batch_receipt",
            "batch_id": proposal_batch_ref(batch.batch_id),
            "proposal_ids": proposal_ids,
            "status": "staged",
            "graph_revision": self.base_revision,
            "data_classes": ["public"],
            "replayability": "fully_managed",
            "receipt": submit_receipt
        });
        let submit_started_at = self
            .submit_started_at
            .as_deref()
            .ok_or(PublicProposalWriteReceiptProjectionError::MissingTiming)?;
        let submit_completed_at = self
            .submit_completed_at
            .as_deref()
            .ok_or(PublicProposalWriteReceiptProjectionError::MissingTiming)?;
        Ok(json!({
            "schema_version": "leaven.plan_result.v1",
            "plan_id": self.plan_id,
            "capability_fingerprint": self.capability_fingerprint,
            "policy_fingerprint": self.policy_fingerprint,
            "base_revision": self.base_revision,
            "final_revision": self.base_revision,
            "replayability_summary": "fully_managed",
            "values": {
                "proposal_batch": proposal_value
            },
            "receipts": [
                {
                    "kind": "write",
                    "receipt": submit_receipt,
                    "op_var": "proposal_batch",
                    "started_at": submit_started_at,
                    "completed_at": submit_completed_at,
                    "write_kind": "submit_proposal_batch",
                    "request_hash": self.proposal_submit_request_hash(batch)?,
                    "result_hash": plan_write_result_hash("proposal_batch", &proposal_value)?,
                    "base_revision": self.base_revision,
                    "committed_revision": self.base_revision,
                    "status": "succeeded",
                    "proposal_batch_id": proposal_batch_ref(batch.batch_id),
                    "proposal_ids": batch.proposal_ids.iter().copied().map(proposal_ref).collect::<Vec<_>>()
                }
            ],
            "redactions": [],
            "charges": [],
            "errors": []
        }))
    }

    /// Projects `RunContext` proposal submission/application into locked Plan Result receipts.
    ///
    /// This helper refuses to mint receipts from ids alone: the supplied batch
    /// and every created candidate must already be visible in the engine graph.
    pub fn proposal_apply_plan_result<P>(
        &self,
        graph: &RunGraphView<'_, P>,
        batch: &ProposalBatchReport,
        apply: &ApplyReport,
    ) -> Result<Value, PublicProposalWriteReceiptProjectionError>
    where
        P: OptimizationProblem,
    {
        if apply.batch_id != batch.batch_id {
            return Err(PublicProposalWriteReceiptProjectionError::ApplyBatchMismatch);
        }
        let proposal_ids = graph_proposal_ids(graph, batch)?;
        let created_candidates = created_candidates(graph, batch, apply)?;
        let submit_receipt = "wrec_submit_proposal_batch".to_owned();
        let apply_receipt = "wrec_apply_proposal_batch".to_owned();
        let proposal_value = json!({
            "kind": "proposal_batch_receipt",
            "batch_id": proposal_batch_ref(batch.batch_id),
            "proposal_ids": proposal_ids,
            "status": "staged",
            "graph_revision": self.base_revision,
            "data_classes": ["public"],
            "replayability": "fully_managed",
            "receipt": submit_receipt
        });
        let apply_value = json!({
            "kind": "apply_receipt",
            "created_candidates": created_candidates,
            "status": "committed",
            "graph_revision": self.final_revision,
            "data_classes": ["public"],
            "replayability": "fully_managed",
            "receipt": apply_receipt
        });
        let submit_started_at = self
            .submit_started_at
            .as_deref()
            .ok_or(PublicProposalWriteReceiptProjectionError::MissingTiming)?;
        let submit_completed_at = self
            .submit_completed_at
            .as_deref()
            .ok_or(PublicProposalWriteReceiptProjectionError::MissingTiming)?;
        let apply_started_at = self
            .apply_started_at
            .as_deref()
            .ok_or(PublicProposalWriteReceiptProjectionError::MissingTiming)?;
        let apply_completed_at = self
            .apply_completed_at
            .as_deref()
            .ok_or(PublicProposalWriteReceiptProjectionError::MissingTiming)?;
        Ok(json!({
            "schema_version": "leaven.plan_result.v1",
            "plan_id": self.plan_id,
            "capability_fingerprint": self.capability_fingerprint,
            "policy_fingerprint": self.policy_fingerprint,
            "base_revision": self.base_revision,
            "final_revision": self.final_revision,
            "replayability_summary": "fully_managed",
            "values": {
                "proposal_batch": proposal_value,
                "apply": apply_value
            },
            "receipts": [
                {
                    "kind": "write",
                    "receipt": submit_receipt,
                    "op_var": "proposal_batch",
                    "started_at": submit_started_at,
                    "completed_at": submit_completed_at,
                    "write_kind": "submit_proposal_batch",
                    "request_hash": self.proposal_submit_request_hash(batch)?,
                    "result_hash": plan_write_result_hash("proposal_batch", &proposal_value)?,
                    "base_revision": self.base_revision,
                    "committed_revision": self.base_revision,
                    "status": "succeeded",
                    "proposal_batch_id": proposal_batch_ref(batch.batch_id),
                    "proposal_ids": batch.proposal_ids.iter().copied().map(proposal_ref).collect::<Vec<_>>()
                },
                {
                    "kind": "write",
                    "receipt": apply_receipt,
                    "op_var": "apply",
                    "started_at": apply_started_at,
                    "completed_at": apply_completed_at,
                    "write_kind": "apply_proposal_batch",
                    "request_hash": self.proposal_apply_request_hash(batch)?,
                    "result_hash": plan_write_result_hash("apply", &apply_value)?,
                    "base_revision": self.base_revision,
                    "committed_revision": self.final_revision,
                    "status": "succeeded",
                    "created_candidates": created_candidates
                }
            ],
            "redactions": [],
            "charges": [],
            "errors": []
        }))
    }

    fn proposal_submit_request_hash(
        &self,
        batch: &ProposalBatchReport,
    ) -> Result<String, PublicProposalWriteReceiptProjectionError> {
        prefixed_jcs(
            "fp_request_sha256_",
            &json!({
                "schema_version": "leaven.submit_proposal_batch_request.v1",
                "batch_id": proposal_batch_ref(batch.batch_id),
                "proposal_ids": batch.proposal_ids.iter().copied().map(proposal_ref).collect::<Vec<_>>(),
                "base_revision": self.base_revision
            }),
        )
    }

    fn proposal_apply_request_hash(
        &self,
        batch: &ProposalBatchReport,
    ) -> Result<String, PublicProposalWriteReceiptProjectionError> {
        prefixed_jcs(
            "fp_request_sha256_",
            &json!({
                "schema_version": "leaven.apply_proposal_batch_request.v1",
                "batch_id": proposal_batch_ref(batch.batch_id),
                "base_revision": self.base_revision
            }),
        )
    }
}

/// Errors raised while projecting `RunContext` proposal writes into V1 receipts.
#[derive(Debug, Error)]
pub enum PublicProposalWriteReceiptProjectionError {
    /// The context did not include receipt timing.
    #[error("proposal write projection requires submit/apply receipt timing")]
    MissingTiming,
    /// The proposal batch report is not present in the graph view.
    #[error("proposal write projection requires a graph-visible proposal batch")]
    BatchNotInGraph,
    /// The proposal batch report does not match the graph batch.
    #[error("proposal write projection batch report does not match graph truth")]
    BatchReportDoesNotMatchGraph,
    /// The apply report belongs to a different proposal batch.
    #[error("proposal apply report belongs to a different proposal batch")]
    ApplyBatchMismatch,
    /// The apply report contains a failed proposal application.
    #[error("proposal apply projection requires committed RunContext apply outcomes")]
    ApplyFailed,
    /// The apply report did not include any proposal outcomes.
    #[error("proposal apply projection requires at least one apply outcome")]
    EmptyApplyBatch,
    /// The apply report candidate/proposal pair does not match graph truth.
    #[error("proposal apply projection outcome does not match graph-created candidate")]
    ApplyOutcomeMismatch,
    /// The apply report outcome set does not exactly cover the proposal batch.
    #[error("proposal apply projection outcome set must exactly match the proposal batch")]
    ApplyOutcomeSetMismatch,
    /// A created candidate was not graph-backed by a proposal from the batch.
    #[error("proposal apply projection requires graph-backed created candidates")]
    CreatedCandidateNotGraphBacked,
    /// JCS/SHA-256 fingerprint computation failed.
    #[error("proposal write fingerprinting failed: {message}")]
    Fingerprint {
        /// Human-readable fingerprinting error.
        message: String,
    },
}

fn prefixed_jcs(
    prefix: &str,
    value: &Value,
) -> Result<String, PublicProposalWriteReceiptProjectionError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value).map_err(|error| {
        PublicProposalWriteReceiptProjectionError::Fingerprint {
            message: error.to_string(),
        }
    })?;
    Ok(format!("{prefix}{digest}"))
}

fn plan_write_result_hash(
    name: &str,
    value: &Value,
) -> Result<String, PublicProposalWriteReceiptProjectionError> {
    prefixed_jcs(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_result.v1",
            "name": name,
            "value": value
        }),
    )
}

fn graph_proposal_ids<P>(
    graph: &RunGraphView<'_, P>,
    batch: &ProposalBatchReport,
) -> Result<Vec<String>, PublicProposalWriteReceiptProjectionError>
where
    P: OptimizationProblem,
{
    let graph_batch = graph
        .proposal_batch(batch.batch_id)
        .ok_or(PublicProposalWriteReceiptProjectionError::BatchNotInGraph)?;
    if graph_batch.proposal_ids() != batch.proposal_ids.as_slice() {
        return Err(PublicProposalWriteReceiptProjectionError::BatchReportDoesNotMatchGraph);
    }
    Ok(batch
        .proposal_ids
        .iter()
        .copied()
        .map(proposal_ref)
        .collect())
}

fn created_candidates<P>(
    graph: &RunGraphView<'_, P>,
    batch: &ProposalBatchReport,
    apply: &ApplyReport,
) -> Result<Vec<String>, PublicProposalWriteReceiptProjectionError>
where
    P: OptimizationProblem,
{
    if apply.outcomes.is_empty() {
        return Err(PublicProposalWriteReceiptProjectionError::EmptyApplyBatch);
    }
    if apply.outcomes.len() != batch.proposal_ids.len() {
        return Err(PublicProposalWriteReceiptProjectionError::ApplyOutcomeSetMismatch);
    }
    let expected = batch.proposal_ids.iter().copied().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut created = Vec::with_capacity(apply.outcomes.len());
    for outcome in &apply.outcomes {
        if !seen.insert(outcome.proposal_id) {
            return Err(PublicProposalWriteReceiptProjectionError::ApplyOutcomeSetMismatch);
        }
        match &outcome.outcome {
            ApplyOutcome::Success { candidate_id } => {
                let proposal = graph.proposal_that_created(*candidate_id).ok_or(
                    PublicProposalWriteReceiptProjectionError::CreatedCandidateNotGraphBacked,
                )?;
                if !batch.proposal_ids.contains(&proposal.id()) {
                    return Err(
                        PublicProposalWriteReceiptProjectionError::CreatedCandidateNotGraphBacked,
                    );
                }
                if proposal.id() != outcome.proposal_id {
                    return Err(PublicProposalWriteReceiptProjectionError::ApplyOutcomeMismatch);
                }
                created.push(candidate_ref(*candidate_id));
            }
            ApplyOutcome::Failure { .. } => {
                return Err(PublicProposalWriteReceiptProjectionError::ApplyFailed);
            }
        }
    }
    if seen != expected {
        return Err(PublicProposalWriteReceiptProjectionError::ApplyOutcomeSetMismatch);
    }
    Ok(created)
}

fn candidate_ref(id: CandidateId) -> String {
    uuid_ref("cand", id.as_uuid())
}

fn proposal_ref(id: ProposalId) -> String {
    uuid_ref("prop", id.as_uuid())
}

fn proposal_batch_ref(id: ProposalBatchId) -> String {
    uuid_ref("pb", id.as_uuid())
}

fn uuid_ref(prefix: &str, id: uuid::Uuid) -> String {
    format!("{prefix}_{id}")
}
