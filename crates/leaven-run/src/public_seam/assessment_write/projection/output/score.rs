use leaven_engine::AssessmentView;
use leaven_evidence::{
    CandidateAssessmentOutput, CaseAssessmentEvidence, DataClass, OutputBlobAudit, OutputRecord,
    OutputVisibility,
};
use leaven_kernel::CandidateId;
use serde_json::{Value, json};

use super::super::PublicAssessmentWriteReceiptProjectionError;
use super::super::refs::{candidate_ref, candidate_refs};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum AssessmentPlanShape {
    Independent(CandidateId),
    Pairwise(Vec<CandidateId>),
    Listwise(Vec<CandidateId>),
}

impl AssessmentPlanShape {
    pub(super) fn from_assessment(
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

    pub(super) const fn kind(&self) -> &'static str {
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

    pub(super) fn insert_candidate_fields(&self, entry: &mut Value) {
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

pub(super) fn score_output(
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

pub(super) fn output_summary(
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

pub(super) fn blob_ref_value(
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
