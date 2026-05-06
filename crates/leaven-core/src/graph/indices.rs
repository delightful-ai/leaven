//! Derived indices over the run graph. Source of truth lives in
//! [`super::storage::RunGraph`]'s record maps.

use std::collections::HashMap;

use crate::artifact::ContentId;
use crate::ids::{ApplyAttemptId, AssessmentId, CandidateId, ProposalId};
use crate::proposal::InfoRef;

#[derive(Default)]
pub struct GraphIndices {
    /// Content identity → all candidates that reached this content.
    pub by_content: HashMap<ContentId, Vec<CandidateId>>,

    /// Candidate → causal parents.
    pub causal_parents: HashMap<CandidateId, Vec<CandidateId>>,

    /// Candidate → causal children.
    pub causal_children: HashMap<CandidateId, Vec<CandidateId>>,

    /// Candidate → things its proposer was informed by.
    pub informed_by: HashMap<CandidateId, Vec<InfoRef>>,

    /// Candidate → candidates whose proposer it informed.
    pub informed: HashMap<CandidateId, Vec<CandidateId>>,

    /// Proposal → its apply attempt (if any).
    pub apply_by_proposal: HashMap<ProposalId, ApplyAttemptId>,

    /// Candidate → the proposal that created it.
    pub proposal_by_candidate: HashMap<CandidateId, ProposalId>,

    /// Candidate → assessments involving it.
    pub assessments_by_candidate: HashMap<CandidateId, Vec<AssessmentId>>,

    /// (left, right) → pairwise assessments. Order-preserving.
    pub pairwise_assessments: HashMap<(CandidateId, CandidateId), Vec<AssessmentId>>,
}
