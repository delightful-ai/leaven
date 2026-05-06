//! ID-only reports returned by context mutation methods.

use leaven_kernel::{
    ApplyAttemptId, AssessmentId, CandidateId, Cost, ErrorRecord, EvaluationRequestId,
    ProposalBatchId, ProposalId, ResolvedEvaluationSetId,
};

use crate::CacheStatus;

#[derive(Clone, Debug)]
pub struct ProposalBatchReport {
    pub batch_id: ProposalBatchId,
    pub proposal_ids: Vec<ProposalId>,
    pub cost: Cost,
}

#[derive(Clone, Debug)]
pub struct ApplyReport {
    pub batch_id: ProposalBatchId,
    pub outcomes: Vec<ApplyOneReport>,
}

impl ApplyReport {
    pub fn successful_candidates(&self) -> impl Iterator<Item = CandidateId> + '_ {
        self.outcomes
            .iter()
            .filter_map(|outcome| match &outcome.outcome {
                ApplyOutcome::Success { candidate_id } => Some(*candidate_id),
                ApplyOutcome::Failure { .. } => None,
            })
    }
}

#[derive(Clone, Debug)]
pub struct ApplyOneReport {
    pub proposal_id: ProposalId,
    pub attempt_id: ApplyAttemptId,
    pub outcome: ApplyOutcome,
}

#[derive(Clone, Debug)]
pub enum ApplyOutcome {
    Success { candidate_id: CandidateId },
    Failure { error: ErrorRecord },
}

#[derive(Clone, Debug)]
pub struct EvaluationReport {
    pub request_id: EvaluationRequestId,
    pub resolved_set: ResolvedEvaluationSetId,
    pub assessment_ids: Vec<AssessmentId>,
    pub cost: Cost,
    pub cache: CacheStatus,
}
