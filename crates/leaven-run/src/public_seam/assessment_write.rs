use std::collections::BTreeSet;

use leaven_core::OptimizationProblem;
use leaven_engine::{EvaluationReport, RunGraphView};
use leaven_kernel::{AssessmentId, EvaluationRequestId};
use serde_json::{Value, json};
use thiserror::Error;

/// Public-seam fields supplied while lowering `RunContext` assessment writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicAssessmentWriteReceiptContext {
    plan_id: String,
    base_revision: String,
    final_revision: String,
    capability_fingerprint: String,
    policy_fingerprint: String,
    started_at: Option<String>,
    completed_at: Option<String>,
}

impl PublicAssessmentWriteReceiptContext {
    /// Creates a public-seam assessment write receipt context.
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
            started_at: None,
            completed_at: None,
        }
    }

    /// Adds audit timing for assessment submission.
    #[must_use]
    pub fn with_timing(
        mut self,
        started_at: impl Into<String>,
        completed_at: impl Into<String>,
    ) -> Self {
        self.started_at = Some(started_at.into());
        self.completed_at = Some(completed_at.into());
        self
    }

    /// Projects `RunContext` evaluation output into locked `submit_assessments` receipts.
    ///
    /// This helper refuses global-bucket assessment ids: every assessment must
    /// exist in the graph and belong to the evaluation request in the report.
    pub fn submit_assessments_plan_result<P>(
        &self,
        graph: &RunGraphView<'_, P>,
        report: &EvaluationReport,
    ) -> Result<Value, PublicAssessmentWriteReceiptProjectionError>
    where
        P: OptimizationProblem,
    {
        if report.assessment_ids.is_empty() {
            return Err(PublicAssessmentWriteReceiptProjectionError::EmptyAssessmentBatch);
        }
        graph
            .evaluation_request(report.request_id)
            .ok_or(PublicAssessmentWriteReceiptProjectionError::RequestNotInGraph)?;
        for assessment_id in &report.assessment_ids {
            let assessment = graph
                .assessment(*assessment_id)
                .ok_or(PublicAssessmentWriteReceiptProjectionError::AssessmentNotInGraph)?;
            if assessment.request_id() != report.request_id {
                return Err(PublicAssessmentWriteReceiptProjectionError::AssessmentRequestMismatch);
            }
        }
        let receipt = "wrec_submit_assessments".to_owned();
        let evaluation_request_id = evaluation_request_ref(report.request_id);
        let assessment_ids = sorted_assessment_refs(&report.assessment_ids);
        let per_assessment = assessment_ids
            .iter()
            .map(|assessment| {
                json!({
                    "assessment": assessment,
                    "replayability": "fully_managed"
                })
            })
            .collect::<Vec<_>>();
        let value = json!({
            "kind": "assessment_batch_receipt",
            "assessment_ids": assessment_ids,
            "evaluation_request_id": evaluation_request_id,
            "per_assessment": per_assessment,
            "status": "committed",
            "graph_revision": self.final_revision,
            "data_classes": ["public"],
            "replayability": "fully_managed",
            "receipt": receipt
        });
        let started_at = self
            .started_at
            .as_deref()
            .ok_or(PublicAssessmentWriteReceiptProjectionError::MissingTiming)?;
        let completed_at = self
            .completed_at
            .as_deref()
            .ok_or(PublicAssessmentWriteReceiptProjectionError::MissingTiming)?;
        Ok(json!({
            "schema_version": "leaven.plan_result.v1",
            "plan_id": self.plan_id,
            "capability_fingerprint": self.capability_fingerprint,
            "policy_fingerprint": self.policy_fingerprint,
            "base_revision": self.base_revision,
            "final_revision": self.final_revision,
            "replayability_summary": "fully_managed",
            "values": {
                "assessment_batch": value
            },
            "receipts": [
                {
                    "kind": "write",
                    "receipt": receipt,
                    "op_var": "assessment_batch",
                    "started_at": started_at,
                    "completed_at": completed_at,
                    "write_kind": "submit_assessments",
                    "request_hash": Self::submit_assessments_request_hash(report)?,
                    "result_hash": plan_write_result_hash("assessment_batch", &value)?,
                    "base_revision": self.base_revision,
                    "committed_revision": self.final_revision,
                    "status": "succeeded",
                    "evaluation_request_id": evaluation_request_id,
                    "assessment_ids": sorted_assessment_refs(&report.assessment_ids)
                }
            ],
            "redactions": [],
            "charges": [],
            "errors": []
        }))
    }

    fn submit_assessments_request_hash(
        report: &EvaluationReport,
    ) -> Result<String, PublicAssessmentWriteReceiptProjectionError> {
        prefixed_jcs(
            "fp_request_sha256_",
            &json!({
                "schema_version": "leaven.submit_assessments_request.v1",
                "evaluation_request_id": evaluation_request_ref(report.request_id),
                "assessment_ids": sorted_assessment_refs(&report.assessment_ids)
            }),
        )
    }
}

/// Errors raised while projecting `RunContext` assessment writes into V1 receipts.
#[derive(Debug, Error)]
pub enum PublicAssessmentWriteReceiptProjectionError {
    /// The context did not include receipt timing.
    #[error("assessment write projection requires receipt timing")]
    MissingTiming,
    /// The evaluation request from the report is not visible in the graph.
    #[error("assessment write projection requires a graph-visible evaluation request")]
    RequestNotInGraph,
    /// The report did not include any assessment ids.
    #[error("assessment write projection requires at least one assessment")]
    EmptyAssessmentBatch,
    /// A reported assessment is not visible in the graph.
    #[error("assessment write projection requires graph-visible assessments")]
    AssessmentNotInGraph,
    /// A reported assessment belongs to a different evaluation request.
    #[error("assessment write projection assessment request mismatch")]
    AssessmentRequestMismatch,
    /// JCS/SHA-256 fingerprint computation failed.
    #[error("assessment write fingerprinting failed: {message}")]
    Fingerprint {
        /// Human-readable fingerprinting error.
        message: String,
    },
}

fn prefixed_jcs(
    prefix: &str,
    value: &Value,
) -> Result<String, PublicAssessmentWriteReceiptProjectionError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value).map_err(|error| {
        PublicAssessmentWriteReceiptProjectionError::Fingerprint {
            message: error.to_string(),
        }
    })?;
    Ok(format!("{prefix}{digest}"))
}

fn plan_write_result_hash(
    name: &str,
    value: &Value,
) -> Result<String, PublicAssessmentWriteReceiptProjectionError> {
    prefixed_jcs(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_result.v1",
            "name": name,
            "value": value
        }),
    )
}

fn sorted_assessment_refs(ids: &[AssessmentId]) -> Vec<String> {
    ids.iter()
        .copied()
        .map(assessment_ref)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn assessment_ref(id: AssessmentId) -> String {
    uuid_ref("assess", id.as_uuid())
}

fn evaluation_request_ref(id: EvaluationRequestId) -> String {
    uuid_ref("evalreq", id.as_uuid())
}

fn uuid_ref(prefix: &str, id: uuid::Uuid) -> String {
    format!("{prefix}_{id}")
}
