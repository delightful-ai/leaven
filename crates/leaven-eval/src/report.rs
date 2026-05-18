//! Lowered evaluation reports.

use leaven_core::PartitionId;
use leaven_kernel::{
    AssessmentId, CandidateId, CaseId, Cost, EvaluationRequestId, EvidenceRef, Fingerprint,
};

use crate::{EvaluationUse, SplitRole};

/// One reportable case score.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ReportScore {
    /// Case this score describes.
    pub case_id: CaseId,
    /// Numeric score for the case.
    pub score: f64,
    /// Evidence ref containing the generated output, when retained in graph truth.
    pub output_ref: Option<EvidenceRef>,
    /// Evidence ref containing scorer feedback, when retained in graph truth.
    pub feedback_ref: Option<EvidenceRef>,
    /// Additional trace refs attached to the row.
    pub trace_refs: Vec<EvidenceRef>,
    /// Human-readable feedback.
    pub feedback: String,
    /// Generated output that was scored.
    pub output: String,
}

/// Candidate summary for one split.
#[derive(Clone, Debug, serde::Serialize)]
pub struct CandidateEvaluationSummary {
    /// Candidate evaluated.
    pub candidate: CandidateId,
    /// Request that produced the assessment.
    pub request: EvaluationRequestId,
    /// Assessment rows summarized here.
    pub assessments: Vec<AssessmentId>,
    /// Average score across present case scores.
    pub average_score: Option<f64>,
    /// Per-case score and feedback evidence.
    pub cases: Vec<ReportScore>,
}

/// Summary for a single split.
#[derive(Clone, Debug, serde::Serialize)]
pub struct SplitReport {
    /// Split role.
    pub role: SplitRole,
    /// Partition id.
    pub partition: PartitionId,
    /// Candidate summaries.
    pub candidates: Vec<CandidateEvaluationSummary>,
}

/// Split-use report row.
#[derive(Clone, Debug, serde::Serialize)]
pub struct SplitUseSummary {
    /// Partition id.
    pub partition: PartitionId,
    /// Role.
    pub role: SplitRole,
    /// Allowed uses.
    pub uses: Vec<EvaluationUse>,
}

/// Evaluation report facade.
#[derive(Clone, Debug, serde::Serialize)]
pub struct EvaluationReport {
    /// Dataset fingerprint.
    pub dataset: Fingerprint,
    /// Split fingerprint.
    pub splits: Fingerprint,
    /// Total cost charged while producing report-visible evaluations.
    pub cost: Cost,
    /// Reported splits.
    pub splits_reported: Vec<SplitReport>,
}
