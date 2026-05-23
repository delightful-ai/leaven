//! Case assessment evidence that preserves generated output, score, and feedback.

use leaven_core::Evidence;
use leaven_kernel::CaseId;
use serde::{Deserialize, Serialize};

use crate::{OutputRecord, ScalarEvidence};

/// Audited case-data materialization used while producing an assessment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CaseDataReadEvidence {
    operation: String,
    receipt: String,
    case: CaseId,
    fields: Vec<String>,
    data_classes: Vec<String>,
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
        }
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
}

/// One case assessment outcome.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseAssessmentEvidence {
    score: ScalarEvidence,
    output: OutputRecord,
    feedback: String,
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
            trace: Vec::new(),
            case_data_reads: Vec::new(),
        }
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
