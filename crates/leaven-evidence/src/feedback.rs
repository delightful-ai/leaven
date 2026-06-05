//! Case assessment evidence that preserves generated output, score, and feedback.

use leaven_core::Evidence;
use leaven_kernel::CaseId;
use serde::{Deserialize, Serialize};

use crate::{DataClass, OutputRecord, ScalarEvidence};
use leaven_kernel::CandidateId;

/// Audited case-data materialization used while producing an assessment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CaseDataReadEvidence {
    operation: String,
    receipt: String,
    case: CaseId,
    fields: Vec<String>,
    data_classes: Vec<String>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    values: serde_json::Map<String, serde_json::Value>,
}

impl CaseDataReadEvidence {
    /// Records one case-data read.
    #[must_use]
    pub fn new(
        operation: impl Into<String>,
        receipt: impl Into<String>,
        case: CaseId,
        fields: impl IntoIterator<Item = impl Into<String>>,
        data_classes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            operation: operation.into(),
            receipt: receipt.into(),
            case,
            fields: fields.into_iter().map(Into::into).collect(),
            data_classes: data_classes.into_iter().map(Into::into).collect(),
            values: serde_json::Map::new(),
        }
    }

    /// Attaches one JSON-serializable case field value that was read.
    #[must_use]
    pub fn with_value(mut self, field: impl Into<String>, value: serde_json::Value) -> Self {
        self.values.insert(field.into(), value);
        self
    }

    /// Operation used to read case data.
    #[must_use]
    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// Receipt identifier for the read.
    #[must_use]
    pub fn receipt(&self) -> &str {
        &self.receipt
    }

    /// Case whose data was read.
    #[must_use]
    pub const fn case(&self) -> CaseId {
        self.case
    }

    /// Case fields read by the operation.
    #[must_use]
    pub fn fields(&self) -> &[String] {
        &self.fields
    }

    /// Data classes carried by the read material.
    #[must_use]
    pub fn data_classes(&self) -> &[String] {
        &self.data_classes
    }

    /// JSON values read from the case, keyed by field name.
    #[must_use]
    pub fn values(&self) -> &serde_json::Map<String, serde_json::Value> {
        &self.values
    }
}

/// One case assessment outcome.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseAssessmentEvidence {
    score: ScalarEvidence,
    output: OutputRecord,
    feedback: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    candidate_outputs: Vec<CandidateAssessmentOutput>,
    trace: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    case_data_reads: Vec<CaseDataReadEvidence>,
}

impl CaseAssessmentEvidence {
    /// Builds case assessment evidence.
    #[must_use]
    pub fn new(score: ScalarEvidence, output: OutputRecord, feedback: impl Into<String>) -> Self {
        Self {
            score,
            output,
            feedback: feedback.into(),
            candidate_outputs: Vec::new(),
            trace: Vec::new(),
            case_data_reads: Vec::new(),
        }
    }

    /// Attaches the candidate-bound outputs assessed by a pairwise/listwise score.
    #[must_use]
    pub fn with_candidate_outputs(
        mut self,
        outputs: impl IntoIterator<Item = CandidateAssessmentOutput>,
    ) -> Self {
        self.candidate_outputs = outputs.into_iter().collect();
        self
    }

    /// Attaches trace lines associated with this case assessment.
    #[must_use]
    pub fn with_trace(mut self, trace: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.trace = trace.into_iter().map(Into::into).collect();
        self
    }

    /// Attaches audited case-data reads used by this assessment.
    #[must_use]
    pub fn with_case_data_reads(
        mut self,
        reads: impl IntoIterator<Item = CaseDataReadEvidence>,
    ) -> Self {
        self.case_data_reads = reads.into_iter().collect();
        self
    }

    /// Comparable scalar score.
    #[must_use]
    pub const fn score(&self) -> ScalarEvidence {
        self.score
    }

    /// Generated output that was scored.
    #[must_use]
    pub const fn output(&self) -> &OutputRecord {
        &self.output
    }

    /// Natural-language feedback attached to the score.
    #[must_use]
    pub fn feedback(&self) -> &str {
        &self.feedback
    }

    /// Candidate-bound output records assessed by pairwise/listwise scores.
    #[must_use]
    pub fn candidate_outputs(&self) -> &[CandidateAssessmentOutput] {
        &self.candidate_outputs
    }

    /// Trace lines attached to the runner/scorer assessment.
    #[must_use]
    pub fn trace(&self) -> &[String] {
        &self.trace
    }

    /// Audited case-data reads used while producing this assessment.
    #[must_use]
    pub fn case_data_reads(&self) -> &[CaseDataReadEvidence] {
        &self.case_data_reads
    }
}

impl Evidence for CaseAssessmentEvidence {}

/// Candidate-bound output assessed by a pairwise/listwise evaluator.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CandidateAssessmentOutput {
    candidate: CandidateId,
    output: OutputRecord,
}

impl CandidateAssessmentOutput {
    /// Builds a candidate-bound assessed output.
    pub fn new(
        candidate: CandidateId,
        output: OutputRecord,
    ) -> Result<Self, CandidateAssessmentOutputError> {
        let data_classes = output.data_classes();
        if !data_classes.contains(&DataClass::candidate_output())
            && !data_classes.contains(&DataClass::candidate_artifact())
        {
            return Err(CandidateAssessmentOutputError::MissingAssessedDataClass);
        }
        if matches!(&output, OutputRecord::Inline { text, .. } if text.trim().is_empty()) {
            return Err(CandidateAssessmentOutputError::EmptyInlineOutput);
        }
        Ok(Self { candidate, output })
    }

    /// Candidate whose output was assessed.
    #[must_use]
    pub const fn candidate(&self) -> CandidateId {
        self.candidate
    }

    /// Output record bound to the candidate.
    #[must_use]
    pub const fn output(&self) -> &OutputRecord {
        &self.output
    }
}

/// Invalid candidate-bound assessed output.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CandidateAssessmentOutputError {
    /// The output did not carry candidate output/artifact data.
    #[error("candidate assessment output must carry candidate.output or candidate.artifact")]
    MissingAssessedDataClass,
    /// Inline candidate output was empty.
    #[error("candidate assessment output must not be an empty inline placeholder")]
    EmptyInlineOutput,
}
