use std::collections::{BTreeMap, BTreeSet};

use leaven_core::OptimizationProblem;
use leaven_engine::{AssessmentView, EvaluationReport, RunGraphView};
use leaven_evidence::{
    CandidateAssessmentOutput, CaseAssessmentEvidence, DataClass, OutputBlobAudit, OutputRecord,
    OutputVisibility,
};
use leaven_kernel::{AssessmentId, CandidateId, EvaluationRequestId};
use leaven_store::{EvidenceStore, StoreError};
use serde_json::{Value, json};
use thiserror::Error;

pub(super) struct ProjectedAssessmentEvidenceRows {
    pub(super) items: Vec<Value>,
    pub(super) query_receipts: Vec<Value>,
    pub(super) data_classes: Vec<String>,
}

pub(super) fn project_assessment_evidence_rows<P>(
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
    /// Stored case-assessment evidence could not be loaded.
    #[error("assessment write projection could not load assessment evidence")]
    EvidenceLoad {
        /// Store-layer failure while loading the evidence payload.
        #[source]
        source: StoreError,
    },
    /// Stored case-assessment evidence did not carry source read receipts.
    #[error("assessment write projection requires real evidence source read receipts")]
    MissingEvidenceSourceReceipts,
    /// The projection does not yet support the assessment shape.
    #[error("assessment write projection does not support this assessment shape")]
    UnsupportedAssessmentShape,
    /// The projection does not yet support this output record shape.
    #[error(
        "assessment write projection requires candidate/artifact inline output or audited blob output"
    )]
    UnsupportedScoreOutput,
    /// JCS/SHA-256 fingerprint computation failed.
    #[error("assessment write fingerprinting failed: {message}")]
    Fingerprint {
        /// Human-readable fingerprinting error.
        message: String,
    },
}

pub(super) fn prefixed_jcs(
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

pub(super) fn plan_write_result_hash(
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

pub(super) fn assessment_plan_entry(
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum AssessmentPlanShape {
    Independent(CandidateId),
    Pairwise(Vec<CandidateId>),
    Listwise(Vec<CandidateId>),
}

impl AssessmentPlanShape {
    fn from_assessment(
        assessment: &AssessmentView<'_>,
    ) -> Result<Self, PublicAssessmentWriteReceiptProjectionError> {
        if let Some(candidate) = assessment.independent_candidate() {
            return Ok(Self::Independent(candidate));
        }
        if let Some((left, right)) = assessment.pairwise_candidates() {
            return Ok(Self::Pairwise(vec![left, right]));
        }
        if let Some(candidates) = assessment.listwise_candidates() {
            return Ok(Self::Listwise(candidates.to_vec()));
        }
        Err(PublicAssessmentWriteReceiptProjectionError::UnsupportedAssessmentShape)
    }

    const fn kind(&self) -> &'static str {
        match self {
            Self::Independent(_) => "independent",
            Self::Pairwise(_) => "pairwise",
            Self::Listwise(_) => "listwise",
        }
    }

    fn candidates(&self) -> &[CandidateId] {
        match self {
            Self::Independent(candidate) => std::slice::from_ref(candidate),
            Self::Pairwise(candidates) | Self::Listwise(candidates) => candidates,
        }
    }

    fn insert_candidate_fields(&self, entry: &mut Value) {
        let object = entry
            .as_object_mut()
            .expect("assessment plan entry is object");
        match self {
            Self::Independent(candidate) => {
                object.insert("candidate".to_owned(), json!(candidate_ref(*candidate)));
            }
            Self::Pairwise(candidates) | Self::Listwise(candidates) => {
                object.insert("candidates".to_owned(), json!(candidate_refs(candidates)));
            }
        }
    }
}

fn score_output(
    shape: &AssessmentPlanShape,
    evidence: &CaseAssessmentEvidence,
) -> Result<Value, PublicAssessmentWriteReceiptProjectionError> {
    let summary = output_summary(evidence.output())?;
    let value = match shape {
        AssessmentPlanShape::Independent(candidate) => {
            candidate_output_value(*candidate, evidence.output())?
        }
        AssessmentPlanShape::Pairwise(_) | AssessmentPlanShape::Listwise(_) => {
            grouped_candidate_output_values(shape.candidates(), evidence.candidate_outputs())?
        }
    };
    let mut output = json!({
        "kind": "structured",
        "summary": summary,
        "value": value,
        "visibility": visibility_wire(evidence.output().metadata().visibility()),
        "data_classes": data_classes_wire(evidence.output().metadata().data_classes())
    });
    if let Some(blob_ref) = blob_ref_value(evidence.output())? {
        output
            .as_object_mut()
            .expect("score output JSON is object")
            .insert("blob_ref".to_owned(), blob_ref);
    }
    Ok(output)
}

fn grouped_candidate_output_values(
    candidates: &[CandidateId],
    outputs: &[CandidateAssessmentOutput],
) -> Result<Value, PublicAssessmentWriteReceiptProjectionError> {
    if outputs.len() != candidates.len() {
        return Err(PublicAssessmentWriteReceiptProjectionError::UnsupportedScoreOutput);
    }
    candidates
        .iter()
        .zip(outputs)
        .map(|(candidate, output)| {
            if output.candidate() != *candidate {
                return Err(PublicAssessmentWriteReceiptProjectionError::UnsupportedScoreOutput);
            }
            candidate_output_value(*candidate, output.output())
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

fn candidate_output_value(
    candidate: CandidateId,
    output: &OutputRecord,
) -> Result<Value, PublicAssessmentWriteReceiptProjectionError> {
    let data_classes = output.metadata().data_classes();
    let value_field = if data_classes.contains(&DataClass::candidate_output()) {
        "output"
    } else if data_classes.contains(&DataClass::candidate_artifact()) {
        "artifact"
    } else {
        return Err(PublicAssessmentWriteReceiptProjectionError::UnsupportedScoreOutput);
    };
    let output_value = match output {
        OutputRecord::Inline { text, .. } => json!(text),
        OutputRecord::BlobRef { .. } => blob_ref_value(output)?
            .ok_or(PublicAssessmentWriteReceiptProjectionError::UnsupportedScoreOutput)?,
    };
    let mut value = json!({
        "candidate": candidate_ref(candidate)
    });
    value
        .as_object_mut()
        .expect("candidate output JSON is object")
        .insert(value_field.to_owned(), output_value);
    Ok(value)
}

fn output_summary(
    output: &OutputRecord,
) -> Result<String, PublicAssessmentWriteReceiptProjectionError> {
    match output {
        OutputRecord::Inline { text, .. } => Ok(text.clone()),
        OutputRecord::BlobRef {
            reference, audit, ..
        } => {
            let audit = audit
                .as_ref()
                .ok_or(PublicAssessmentWriteReceiptProjectionError::UnsupportedScoreOutput)?;
            Ok(format!(
                "blob {}:{} sha256={} bytes={}",
                reference.store,
                reference.key,
                audit.sha256(),
                audit.bytes()
            ))
        }
    }
}

fn blob_ref_value(
    output: &OutputRecord,
) -> Result<Option<Value>, PublicAssessmentWriteReceiptProjectionError> {
    let OutputRecord::BlobRef {
        reference, audit, ..
    } = output
    else {
        return Ok(None);
    };
    let audit = audit
        .as_ref()
        .ok_or(PublicAssessmentWriteReceiptProjectionError::UnsupportedScoreOutput)?;
    Ok(Some(public_blob_ref_value(
        reference,
        audit,
        output.metadata().data_classes(),
    )))
}

fn public_blob_ref_value(
    reference: &leaven_kernel::BlobRef,
    audit: &OutputBlobAudit,
    data_classes: &leaven_evidence::DataClassSet,
) -> Value {
    let mut value = json!({
        "kind": "blob_ref",
        "id": public_blob_id(reference),
        "sha256": audit.sha256(),
        "bytes": audit.bytes(),
        "data_classes": data_classes_wire(data_classes)
    });
    let object = value.as_object_mut().expect("blob ref JSON is object");
    if let Some(media_type) = audit.media_type() {
        object.insert("media_type".to_owned(), json!(media_type));
    }
    if let Some(uri) = audit.uri() {
        object.insert("uri".to_owned(), json!(uri));
    }
    value
}

fn public_blob_id(reference: &leaven_kernel::BlobRef) -> String {
    let digest = jcs_canonicalize::sha256_jcs_hex(&json!({
        "store": reference.store,
        "key": reference.key
    }))
    .expect("blob reference JSON is canonicalizable");
    format!("blob_{digest}")
}

fn visibility_wire(visibility: OutputVisibility) -> &'static str {
    match visibility {
        OutputVisibility::Public => "public",
        OutputVisibility::OptimizerVisible => "optimizer_visible",
        OutputVisibility::ReflectorVisible => "reflector_visible",
        OutputVisibility::EvaluatorOnly => "evaluator_only",
        OutputVisibility::OperatorOnly => "operator_only",
        OutputVisibility::Private => "private",
        OutputVisibility::Redacted => "redacted",
    }
}

fn data_classes_wire(data_classes: &leaven_evidence::DataClassSet) -> Vec<&str> {
    data_classes
        .iter()
        .map(leaven_evidence::DataClass::as_str)
        .collect()
}

pub(super) fn sorted_assessment_refs(ids: &[AssessmentId]) -> Vec<String> {
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

fn candidate_ref(id: leaven_kernel::CandidateId) -> String {
    uuid_ref("cand", id.as_uuid())
}

fn candidate_refs(ids: &[CandidateId]) -> Vec<String> {
    ids.iter().copied().map(candidate_ref).collect()
}

pub(super) fn evaluation_request_ref(id: EvaluationRequestId) -> String {
    uuid_ref("evalreq", id.as_uuid())
}

fn uuid_ref(prefix: &str, id: uuid::Uuid) -> String {
    format!("{prefix}_{id}")
}
