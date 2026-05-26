//! Run-graph storage record vocabulary.

use leaven_core::{
    Artifact, ArtifactIdentity, AssessmentTarget, EvaluationRequest, OptimizationProblem,
    ProposalBatchSemantics, ProposalEffect, ProposalProvenance, ResolvedEvaluationSet,
};
use leaven_kernel::{
    ApplyAttemptId, AssessmentId, CandidateId, ErrorRecord, EvaluationRequestId, EvaluatorId,
    EvidenceRef, Fingerprint, IterationId, MetadataBag, ProposalBatchId, ProposalId, StageId,
    Timestamp,
};
use serde::{Deserialize, Serialize};

/// One candidate stored in the append-only run graph.
#[derive(Clone, Serialize, Deserialize)]
#[serde(bound(serialize = "A: Serialize", deserialize = "A: Deserialize<'de>"))]
pub struct CandidateRecord<A: Artifact> {
    pub id: CandidateId,
    pub identity: ArtifactIdentity,
    pub artifact: A,
    pub origin: CandidateOrigin,
    pub created_at: Timestamp,
}

/// Origin of a candidate stored in the run graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum CandidateOrigin {
    /// Initial seed candidate supplied by the run.
    Seed {
        /// Seed position in caller order.
        seed_index: usize,
    },
    /// Candidate produced by applying a proposal.
    Proposal {
        /// Proposal that created this candidate.
        proposal_id: ProposalId,
        /// Apply attempt that admitted this candidate.
        apply_attempt_id: ApplyAttemptId,
    },
}

/// Proposal batch metadata and proposal membership.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProposalBatchRecord {
    pub id: ProposalBatchId,
    pub stage: StageId,
    pub semantics: ProposalBatchSemantics,
    pub proposal_ids: Vec<ProposalId>,
    pub metadata: MetadataBag,
    pub created_at: Timestamp,
    pub iteration: Option<IterationId>,
}

/// One proposal stored in the run graph.
#[derive(Serialize, Deserialize)]
#[serde(bound(
    serialize = "P::Artifact: Serialize, <P::Artifact as Artifact>::Change: Serialize, P::ProposalAnnotations: Serialize",
    deserialize = "P::Artifact: Deserialize<'de>, <P::Artifact as Artifact>::Change: Deserialize<'de>, P::ProposalAnnotations: Deserialize<'de>"
))]
pub struct ProposalRecord<P: OptimizationProblem> {
    pub id: ProposalId,
    pub batch_id: ProposalBatchId,
    pub effect: ProposalEffect<P>,
    pub provenance: ProposalProvenance,
    pub annotations: P::ProposalAnnotations,
    pub metadata: MetadataBag,
    pub created_at: Timestamp,
}

impl<P: OptimizationProblem> Clone for ProposalRecord<P> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            batch_id: self.batch_id,
            effect: self.effect.clone(),
            provenance: self.provenance.clone(),
            annotations: self.annotations.clone(),
            metadata: self.metadata.clone(),
            created_at: self.created_at,
        }
    }
}

/// One proposal application attempt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplyAttemptRecord {
    pub id: ApplyAttemptId,
    pub proposal_id: ProposalId,
    pub outcome: ApplyAttemptOutcome,
    pub created_at: Timestamp,
}

/// Result of applying a proposal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ApplyAttemptOutcome {
    /// Proposal application produced a candidate.
    Success { candidate_id: CandidateId },
    /// Proposal application failed and preserved an auditable error record.
    Failure { error: ErrorRecord },
}

/// One assessment stored in the run graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssessmentRecord {
    pub id: AssessmentId,
    pub request_id: EvaluationRequestId,
    pub evaluator: EvaluatorId,
    pub target: AssessmentRecordTarget,
    pub evidence: EvidenceRef,
    pub metadata: MetadataBag,
    pub created_at: Timestamp,
}

/// Candidate target shape for an assessment record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AssessmentRecordTarget {
    /// Independent assessment of one candidate.
    Independent {
        candidate: CandidateId,
        target: AssessmentTarget,
    },
    /// Pairwise assessment of two candidates.
    Pairwise {
        left: CandidateId,
        right: CandidateId,
        target: AssessmentTarget,
    },
    /// Listwise assessment of candidate order or group quality.
    Listwise {
        candidates: Vec<CandidateId>,
        target: AssessmentTarget,
    },
}

impl AssessmentRecordTarget {
    pub(crate) fn candidates(&self) -> Vec<CandidateId> {
        match self {
            Self::Independent { candidate, .. } => vec![*candidate],
            Self::Pairwise { left, right, .. } => vec![*left, *right],
            Self::Listwise { candidates, .. } => candidates.clone(),
        }
    }
}

/// One evaluation request recorded by the engine.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvaluationRequestRecord {
    pub id: EvaluationRequestId,
    pub evaluator: EvaluatorId,
    pub evaluator_fingerprint: Fingerprint,
    pub request: EvaluationRequest,
    pub resolved_set: ResolvedEvaluationSet,
    pub created_at: Timestamp,
}
