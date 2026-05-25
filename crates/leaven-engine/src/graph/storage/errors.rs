use leaven_kernel::{
    ApplyAttemptId, AssessmentId, CandidateId, ErrorKind, ErrorRecord, EvaluationRequestId,
    ProposalBatchId, ProposalId,
};
use thiserror::Error;

/// Refusal reasons for inserting or applying proposals to a run graph.
#[derive(Debug, Error)]
pub enum ApplyProposalError {
    /// The proposal references a candidate that is not present in the graph.
    #[error("unknown candidate: {0}")]
    UnknownCandidate(CandidateId),
    /// The requested proposal is not present in the graph.
    #[error("unknown proposal: {0}")]
    UnknownProposal(ProposalId),
    /// The proposal has already been applied.
    #[error("proposal already applied: {0}")]
    AlreadyApplied(ProposalId),
    /// The proposal's effect and causal provenance disagree.
    #[error("invalid proposal provenance")]
    InvalidProvenance,
    /// The artifact rejected validation or change application.
    #[error("artifact operation failed")]
    Artifact {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

#[derive(Debug, Error)]
pub enum RunGraphRestoreError {
    #[error("duplicate candidate in graph snapshot: {0}")]
    DuplicateCandidate(CandidateId),
    #[error("duplicate proposal batch in graph snapshot: {0}")]
    DuplicateProposalBatch(ProposalBatchId),
    #[error("duplicate proposal in graph snapshot: {0}")]
    DuplicateProposal(ProposalId),
    #[error("duplicate apply attempt in graph snapshot: {0}")]
    DuplicateApplyAttempt(ApplyAttemptId),
    #[error("duplicate evaluation request in graph snapshot: {0}")]
    DuplicateEvaluationRequest(EvaluationRequestId),
    #[error("duplicate assessment in graph snapshot: {0}")]
    DuplicateAssessment(AssessmentId),
    #[error("proposal batch {batch} references missing proposal {proposal}")]
    MissingProposalInBatch {
        batch: ProposalBatchId,
        proposal: ProposalId,
    },
    #[error("proposal {proposal} references missing proposal batch {batch}")]
    MissingBatchForProposal {
        proposal: ProposalId,
        batch: ProposalBatchId,
    },
    #[error("proposal {proposal} is not listed in its proposal batch {batch}")]
    ProposalNotListedInBatch {
        proposal: ProposalId,
        batch: ProposalBatchId,
    },
    #[error("proposal {proposal} is invalid in restored graph snapshot: {reason}")]
    InvalidRestoredProposal {
        proposal: ProposalId,
        reason: String,
    },
    #[error("apply attempt {attempt} references missing proposal {proposal}")]
    MissingProposalForApplyAttempt {
        attempt: ApplyAttemptId,
        proposal: ProposalId,
    },
    #[error("successful apply attempt {attempt} references missing candidate {candidate}")]
    MissingCandidateForSuccessfulApplyAttempt {
        attempt: ApplyAttemptId,
        candidate: CandidateId,
    },
    #[error("successful apply attempt {attempt} and candidate {candidate} disagree")]
    ApplyAttemptCandidateMismatch {
        attempt: ApplyAttemptId,
        candidate: CandidateId,
    },
    #[error(
        "candidate {candidate} was restored from proposal {proposal}, but that proposal is missing"
    )]
    MissingProposalForCandidate {
        candidate: CandidateId,
        proposal: ProposalId,
    },
    #[error("candidate {candidate} references missing apply attempt {attempt}")]
    MissingApplyAttemptForCandidate {
        candidate: CandidateId,
        attempt: ApplyAttemptId,
    },
    #[error("candidate {candidate} and apply attempt {attempt} disagree")]
    CandidateApplyAttemptMismatch {
        candidate: CandidateId,
        attempt: ApplyAttemptId,
    },
    #[error("assessment {assessment} references missing evaluation request {request}")]
    MissingEvaluationRequestForAssessment {
        assessment: AssessmentId,
        request: EvaluationRequestId,
    },
    #[error("assessment {assessment} references missing candidate {candidate}")]
    MissingCandidateForAssessment {
        assessment: AssessmentId,
        candidate: CandidateId,
    },
}

impl ApplyProposalError {
    pub(super) fn artifact(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Artifact {
            source: Box::new(err),
        }
    }

    pub(super) fn to_record(&self) -> ErrorRecord {
        let kind = match self {
            Self::AlreadyApplied(_) | Self::InvalidProvenance => ErrorKind::GraphInvariant,
            Self::UnknownCandidate(_) | Self::UnknownProposal(_) | Self::Artifact { .. } => {
                ErrorKind::Apply
            }
        };
        ErrorRecord::from_error(kind, self)
    }
}
