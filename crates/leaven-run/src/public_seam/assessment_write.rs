use leaven_core::OptimizationProblem;
use leaven_engine::{EvaluationReport, RunGraphView};
use leaven_evidence::CaseAssessmentEvidence;
use leaven_store::EvidenceStore;
use serde_json::{Value, json};

mod projection;

pub use projection::PublicAssessmentWriteReceiptProjectionError;
use projection::{
    assessment_plan_entry, evaluation_request_ref, plan_write_result_hash, prefixed_jcs,
    project_assessment_evidence_rows, sorted_assessment_refs,
};

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

    /// Projects graph-backed assessment evidence into a locked Plan Result.
    ///
    /// Unlike [`Self::submit_assessments_plan_result`], this includes
    /// `assessment_summary` rows and the query receipts cited by each evidence
    /// envelope. This is the producer-side proof path for result evidence
    /// visibility: if stored assessment evidence carries audited case-data read
    /// facts, their receipt ids and data classes are emitted into the result
    /// receipt stream instead of remaining private policy metadata.
    pub fn submit_assessments_plan_result_with_evidence<P>(
        &self,
        graph: &RunGraphView<'_, P>,
        evidence_store: &dyn EvidenceStore<CaseAssessmentEvidence>,
        report: &EvaluationReport,
    ) -> Result<Value, PublicAssessmentWriteReceiptProjectionError>
    where
        P: OptimizationProblem<Evidence = CaseAssessmentEvidence>,
    {
        if report.assessment_ids.is_empty() {
            return Err(PublicAssessmentWriteReceiptProjectionError::EmptyAssessmentBatch);
        }
        graph
            .evaluation_request(report.request_id)
            .ok_or(PublicAssessmentWriteReceiptProjectionError::RequestNotInGraph)?;
        let started_at = self
            .started_at
            .as_deref()
            .ok_or(PublicAssessmentWriteReceiptProjectionError::MissingTiming)?;
        let completed_at = self
            .completed_at
            .as_deref()
            .ok_or(PublicAssessmentWriteReceiptProjectionError::MissingTiming)?;
        let projected = project_assessment_evidence_rows(
            graph,
            evidence_store,
            report,
            &self.base_revision,
            started_at,
            completed_at,
        )?;

        let assessment_rows = json!({
            "kind": "graph_set",
            "items": projected.items,
            "graph_revision": self.final_revision,
            "data_classes": projected.data_classes,
            "replayability": "fully_managed"
        });
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
        let assessment_batch = json!({
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
        let mut receipts = projected.query_receipts;
        receipts.push(json!({
            "kind": "write",
            "receipt": receipt,
            "op_var": "assessment_batch",
            "started_at": started_at,
            "completed_at": completed_at,
            "write_kind": "submit_assessments",
            "request_hash": Self::submit_assessments_request_hash(report)?,
            "result_hash": plan_write_result_hash("assessment_batch", &assessment_batch)?,
            "base_revision": self.base_revision,
            "committed_revision": self.final_revision,
            "status": "succeeded",
            "evaluation_request_id": evaluation_request_id,
            "assessment_ids": sorted_assessment_refs(&report.assessment_ids)
        }));

        Ok(json!({
            "schema_version": "leaven.plan_result.v1",
            "plan_id": self.plan_id,
            "capability_fingerprint": self.capability_fingerprint,
            "policy_fingerprint": self.policy_fingerprint,
            "base_revision": self.base_revision,
            "final_revision": self.final_revision,
            "replayability_summary": "fully_managed",
            "values": {
                "assessment_rows": assessment_rows,
                "assessment_batch": assessment_batch
            },
            "receipts": receipts,
            "redactions": [],
            "charges": [],
            "errors": []
        }))
    }

    /// Projects graph-backed assessment evidence into a locked `submit_assessments` Plan document.
    ///
    /// This is the public-seam Plan IR counterpart to
    /// [`Self::submit_assessments_plan_result`]. It refuses to synthesize
    /// assessment rows from ids alone: every assessment must belong to the
    /// report request and its stored `CaseAssessmentEvidence` must be readable
    /// before a `Score.output` field is emitted.
    pub fn submit_assessments_plan_document<P>(
        &self,
        graph: &RunGraphView<'_, P>,
        evidence_store: &dyn EvidenceStore<CaseAssessmentEvidence>,
        report: &EvaluationReport,
    ) -> Result<Value, PublicAssessmentWriteReceiptProjectionError>
    where
        P: OptimizationProblem<Evidence = CaseAssessmentEvidence>,
    {
        if report.assessment_ids.is_empty() {
            return Err(PublicAssessmentWriteReceiptProjectionError::EmptyAssessmentBatch);
        }
        graph
            .evaluation_request(report.request_id)
            .ok_or(PublicAssessmentWriteReceiptProjectionError::RequestNotInGraph)?;
        let mut assessments = Vec::with_capacity(report.assessment_ids.len());
        for assessment_id in &report.assessment_ids {
            let assessment = graph
                .assessment(*assessment_id)
                .ok_or(PublicAssessmentWriteReceiptProjectionError::AssessmentNotInGraph)?;
            if assessment.request_id() != report.request_id {
                return Err(PublicAssessmentWriteReceiptProjectionError::AssessmentRequestMismatch);
            }
            let evidence = evidence_store
                .get(assessment.evidence_ref())
                .map_err(
                    |source| PublicAssessmentWriteReceiptProjectionError::EvidenceLoad { source },
                )?;
            assessments.push(assessment_plan_entry(&assessment, &evidence)?);
        }
        Ok(json!({
            "schema_version": "leaven.plan.v1",
            "plan_id": self.plan_id,
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "dry_run"
            },
            "ops": [
                {
                    "kind": "write",
                    "name": "assessments",
                    "idempotency_key": format!("{}-submit-assessments", self.plan_id),
                    "write": {
                        "kind": "submit_assessments",
                        "evaluation_request_id": evaluation_request_ref(report.request_id),
                        "assessments": assessments
                    }
                }
            ],
            "return": ["assessments"],
            "commit": {
                "kind": "no_graph_writes"
            }
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
