use leaven_engine::EvaluationError;
use leaven_evidence::{CandidateAssessmentOutput, CandidateAssessmentOutputError, OutputRecord};

use super::JudgeCandidateOutput;
use crate::evidence::{ReportableOutputDeclaration, artifact_identity_output};

pub(super) fn assessed_candidate_outputs<A, Out>(
    outputs: &[JudgeCandidateOutput<A, Out>],
) -> Result<Vec<CandidateAssessmentOutput>, EvaluationError> {
    outputs
        .iter()
        .map(|output| {
            let reportable = output.output.reportable_output().ok_or_else(|| {
                EvaluationError::Message(
                    "runner output did not declare reportable assessed output".to_owned(),
                )
            })?;
            CandidateAssessmentOutput::new(output.candidate, reportable.record().clone()).map_err(
                |error| match error {
                    CandidateAssessmentOutputError::MissingAssessedDataClass => {
                        EvaluationError::Message(
                            "runner output did not declare candidate or artifact assessed output"
                                .to_owned(),
                        )
                    }
                    CandidateAssessmentOutputError::EmptyInlineOutput => {
                        EvaluationError::Message(error.to_string())
                    }
                },
            )
        })
        .collect()
}

pub(super) fn assessed_group_output<A, Out>(
    outputs: &[JudgeCandidateOutput<A, Out>],
) -> Option<ReportableOutputDeclaration> {
    let mut texts = Vec::with_capacity(outputs.len());
    let mut truncated = false;
    let mut metadata = None;
    let mut unbound_explicit_assessed_output = false;
    for output in outputs {
        let reportable = output.output.reportable_output()?;
        unbound_explicit_assessed_output |= reportable.is_unbound_explicit_candidate_output()
            || reportable.is_unbound_explicit_candidate_artifact();
        let OutputRecord::Inline {
            text,
            truncated: output_truncated,
            metadata: output_metadata,
        } = reportable.record()
        else {
            return None;
        };
        if let Some(metadata) = &metadata {
            if metadata != output_metadata {
                return None;
            }
        } else {
            metadata = Some(output_metadata.clone());
        }
        truncated |= *output_truncated;
        texts.push(text.clone());
    }
    let record = OutputRecord::Inline {
        text: texts.join("|"),
        truncated,
        metadata: metadata.unwrap_or_else(leaven_evidence::OutputMetadata::public),
    };
    if unbound_explicit_assessed_output {
        Some(ReportableOutputDeclaration::explicit(record))
    } else {
        Some(ReportableOutputDeclaration::derived(record))
    }
}

pub(super) fn grouped_artifact_identity_output<A, Out>(
    outputs: &[JudgeCandidateOutput<A, Out>],
) -> OutputRecord
where
    A: leaven_core::Artifact,
{
    let text = outputs
        .iter()
        .map(|output| match artifact_identity_output(&output.artifact) {
            OutputRecord::Inline { text, .. } => text,
            OutputRecord::BlobRef { .. } => unreachable!("artifact identity output is inline"),
        })
        .collect::<Vec<_>>()
        .join("|");
    OutputRecord::candidate_artifact_inline(text)
}
