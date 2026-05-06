//! Derived graph indices.

use std::collections::HashMap;

use leaven_core::{ArtifactIdentity, InfoRef};
use leaven_kernel::{AssessmentId, CandidateId, ProposalId};

#[derive(Default)]
pub struct GraphIndices {
    pub by_identity: HashMap<ArtifactIdentity, Vec<CandidateId>>,
    pub causal_parents: HashMap<CandidateId, Vec<CandidateId>>,
    pub causal_children: HashMap<CandidateId, Vec<CandidateId>>,
    pub informed_by: HashMap<CandidateId, Vec<InfoRef>>,
    pub informed: HashMap<CandidateId, Vec<CandidateId>>,
    pub proposal_by_candidate: HashMap<CandidateId, ProposalId>,
    pub assessments_by_candidate: HashMap<CandidateId, Vec<AssessmentId>>,
}
