use std::collections::{BTreeMap, BTreeSet};

use leaven_core::OptimizationProblem;
use leaven_engine::{AssessmentView, EvaluationReport, RunGraphView};
use leaven_evidence::CaseAssessmentEvidence;
use leaven_store::EvidenceStore;
use serde_json::{Value, json};

use super::PublicAssessmentWriteReceiptProjectionError;
use super::refs::assessment_ref;

mod score;

use score::{AssessmentPlanShape, blob_ref_value, output_summary, score_output};

pub(in crate::public_seam::assessment_write) struct ProjectedAssessmentEvidenceRows {
    pub(in crate::public_seam::assessment_write) items: Vec<Value>,
    pub(in crate::public_seam::assessment_write) query_receipts: Vec<Value>,
    pub(in crate::public_seam::assessment_write) data_classes: Vec<String>,
}

pub(in crate::public_seam::assessment_write) fn project_assessment_evidence_rows<P>(
    graph: &RunGraphView<'_, P>,
    evidence_store: &dyn EvidenceStore<CaseAssessmentEvidence>,
    report: &EvaluationReport,
    base_revision: &str,
    started_at: &str,
    completed_at: &str,
) -> Result<ProjectedAssessmentEvidenceRows, PublicAssessmentWriteReceiptProjectionError>
where
    P: OptimizationProblem<Evidence = CaseAssessmentEvidence>,
{
    let mut items = Vec::with_capacity(report.assessment_ids.len());
    let mut query_receipts = Vec::with_capacity(report.assessment_ids.len());
    let mut data_classes = BTreeSet::from(["public".to_owned()]);
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
        let source_receipts = evidence_source_read_receipts(&evidence);
        if source_receipts.is_empty() {
            return Err(PublicAssessmentWriteReceiptProjectionError::MissingEvidenceSourceReceipts);
        }
        let source_receipt_ids = source_receipt_ids(&source_receipts);
        data_classes.extend(assessment_summary_data_classes(&assessment, &evidence)?);
        query_receipts.extend(evidence_query_receipts(
            &source_receipts,
            base_revision,
            started_at,
            completed_at,
        ));
        items.push(assessment_summary_item(
            &assessment,
            &evidence,
            &source_receipt_ids,
        )?);
    }
    Ok(ProjectedAssessmentEvidenceRows {
        items,
        query_receipts,
        data_classes: data_classes.into_iter().collect(),
    })
}

pub(in crate::public_seam::assessment_write) fn assessment_plan_entry(
    assessment: &AssessmentView<'_>,
    evidence: &CaseAssessmentEvidence,
) -> Result<Value, PublicAssessmentWriteReceiptProjectionError> {
    let shape = AssessmentPlanShape::from_assessment(assessment)?;
    let output = score_output(&shape, evidence)?;
    let source_receipts = plan_document_source_read_receipts(assessment, evidence);
    let mut entry = json!({
        "kind": shape.kind(),
        "score": {
            "value": evidence.score().score(),
            "output": output
        },
        "evidence": assessment_evidence_envelope(evidence, &source_receipts)?,
        "replayability": "fully_managed"
    });
    shape.insert_candidate_fields(&mut entry);
    Ok(entry)
}

fn assessment_summary_item(
    assessment: &AssessmentView<'_>,
    evidence: &CaseAssessmentEvidence,
    source_receipts: &[String],
) -> Result<Value, PublicAssessmentWriteReceiptProjectionError> {
    let shape = AssessmentPlanShape::from_assessment(assessment)?;
    Ok(json!({
        "kind": "assessment_summary",
        "assessment": assessment_ref(assessment.id()),
        "score": {
            "value": evidence.score().score(),
            "output": score_output(&shape, evidence)?
        },
        "evidence": assessment_evidence_envelope(evidence, source_receipts)?
    }))
}

fn assessment_evidence_envelope(
    evidence: &CaseAssessmentEvidence,
    source_receipts: &[String],
) -> Result<Value, PublicAssessmentWriteReceiptProjectionError> {
    let data_classes = evidence_data_classes(evidence);
    let target_derived = data_classes
        .iter()
        .any(|data_class| data_class == "case.target");
    let trace_refs = evidence_trace_refs(evidence);
    let mut public = json!({
        "summary": output_summary(evidence.output())?,
        "data_classes": &data_classes
    });
    if !trace_refs.is_empty() {
        public
            .as_object_mut()
            .expect("evidence public projection is object")
            .insert("trace_refs".to_owned(), json!(trace_refs));
    }
    Ok(json!({
        "schema_version": "leaven.evidence_envelope.v1",
        "target_derived": target_derived,
        "data_classes": data_classes,
        "public": public,
        "redaction_policy": {
            "optimizer": "score_only",
            "reflector": "score_only",
            "operator": "score_only"
        },
        "producer": {
            "stage_call_id": "sc_public_assessment_projection"
        },
        "source_receipts": {
            "read": source_receipts,
            "effect": []
        }
    }))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EvidenceSourceReadReceipt {
    id: String,
    data_classes: Vec<String>,
}

fn evidence_source_read_receipts(
    evidence: &CaseAssessmentEvidence,
) -> Vec<EvidenceSourceReadReceipt> {
    let mut by_receipt = BTreeMap::<String, BTreeSet<String>>::new();
    for read in evidence.case_data_reads() {
        let entry = by_receipt
            .entry(read.receipt().to_owned())
            .or_insert_with(|| BTreeSet::from(["public".to_owned()]));
        entry.extend(read.data_classes().iter().cloned());
    }
    by_receipt
        .into_iter()
        .map(|(id, data_classes)| EvidenceSourceReadReceipt {
            id,
            data_classes: data_classes.into_iter().collect(),
        })
        .collect()
}

fn source_receipt_ids(receipts: &[EvidenceSourceReadReceipt]) -> Vec<String> {
    receipts.iter().map(|receipt| receipt.id.clone()).collect()
}

fn plan_document_source_read_receipts(
    assessment: &AssessmentView<'_>,
    evidence: &CaseAssessmentEvidence,
) -> Vec<String> {
    let receipts = evidence
        .case_data_reads()
        .iter()
        .map(|read| read.receipt().to_owned())
        .collect::<BTreeSet<_>>();
    if receipts.is_empty() {
        vec![format!(
            "qrec_assessment_evidence_{}",
            assessment.id().as_uuid()
        )]
    } else {
        receipts.into_iter().collect()
    }
}

fn evidence_data_classes(evidence: &CaseAssessmentEvidence) -> Vec<String> {
    let mut data_classes = BTreeSet::from(["public".to_owned()]);
    for read in evidence.case_data_reads() {
        data_classes.extend(read.data_classes().iter().cloned());
    }
    data_classes.into_iter().collect()
}

fn evidence_trace_refs(evidence: &CaseAssessmentEvidence) -> Vec<Value> {
    evidence
        .case_data_reads()
        .iter()
        .map(|read| {
            json!({
                "kind": read.operation(),
                "id": format!("trace_{}", read.receipt()),
                "visibility": "redacted_transcript",
                "data_classes": read.data_classes(),
                "receipt": read.receipt()
            })
        })
        .collect()
}

fn assessment_summary_data_classes(
    assessment: &AssessmentView<'_>,
    evidence: &CaseAssessmentEvidence,
) -> Result<Vec<String>, PublicAssessmentWriteReceiptProjectionError> {
    let shape = AssessmentPlanShape::from_assessment(assessment)?;
    let mut data_classes = BTreeSet::new();
    for data_class in evidence.output().metadata().data_classes().iter() {
        data_classes.insert(data_class.as_str().to_owned());
    }
    if let Some(blob_ref) = blob_ref_value(evidence.output())?
        && let Some(blob_data_classes) = blob_ref.get("data_classes").and_then(Value::as_array)
    {
        data_classes.extend(
            blob_data_classes
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned),
        );
    }
    match &shape {
        AssessmentPlanShape::Independent(_) => {}
        AssessmentPlanShape::Pairwise(_) | AssessmentPlanShape::Listwise(_) => {
            for output in evidence.candidate_outputs() {
                for data_class in output.output().metadata().data_classes().iter() {
                    data_classes.insert(data_class.as_str().to_owned());
                }
            }
        }
    }
    data_classes.extend(evidence_data_classes(evidence));
    Ok(data_classes.into_iter().collect())
}

fn evidence_query_receipts(
    receipts: &[EvidenceSourceReadReceipt],
    graph_revision: &str,
    started_at: &str,
    completed_at: &str,
) -> Vec<Value> {
    receipts
        .iter()
        .map(|receipt| {
            json!({
                "kind": "query",
                "receipt": receipt.id,
                "started_at": started_at,
                "completed_at": completed_at,
                "op_hash": format!("fp_query_sha256_{}", receipt.id),
                "result_hash": format!("fp_result_sha256_{}", receipt.id),
                "graph_revision": graph_revision,
                "status": "succeeded",
                "read_scope_fingerprint": format!("fp_scope_sha256_{}", receipt.id),
                "projection_fingerprint": format!("fp_projection_sha256_{}", receipt.id),
                "trace_refs": [
                    {
                        "kind": "assessment_evidence_visibility",
                        "id": format!("trace_{}", receipt.id),
                        "visibility": "redacted_transcript",
                        "data_classes": receipt.data_classes,
                        "receipt": receipt.id
                    }
                ]
            })
        })
        .collect()
}
