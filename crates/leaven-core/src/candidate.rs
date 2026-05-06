//! `Candidate` — graph-local occurrence of an artifact.
//!
//! A `CandidateId` is per-run, per-occurrence. Two candidates with the
//! same `ContentId` are still distinct candidates because their causal
//! histories differ. There is no `Accepted`/`Rejected` candidate state
//! at the candidate level; that is population state.

use serde::{Deserialize, Serialize};

use crate::artifact::{Artifact, ContentId};
use crate::ids::{ApplyAttemptId, CandidateId, ProposalId};
use crate::time::Timestamp;

#[derive(Clone, Debug)]
pub struct Candidate<A: Artifact> {
    pub id: CandidateId,
    pub content_id: ContentId,
    pub artifact: A,
    pub origin: CandidateOrigin,
    pub created_at: Timestamp,
}

/// How a candidate entered the run graph. Origin never changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum CandidateOrigin {
    /// Inserted directly as a seed before optimization began.
    Seed { seed_index: usize },

    /// Produced by applying a proposal.
    Proposal {
        proposal_id: ProposalId,
        apply_attempt_id: ApplyAttemptId,
    },
}
