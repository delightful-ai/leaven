//! Run-graph storage records and private mutators.

use indexmap::IndexMap;
use leaven_core::{
    Artifact, ArtifactIdentity, AssessmentTarget, CausalInputs, EvaluationRequest, InfoRef,
    OptimizationProblem, ProposalBatch, ProposalBatchSemantics, ProposalEffect, ProposalEffectKind,
    ProposalProvenance, ResolvedEvaluationSet,
};
use leaven_kernel::{
    ApplyAttemptId, AssessmentId, CandidateId, ErrorKind, ErrorRecord, EvaluationRequestId,
    EvaluatorId, EvidenceRef, IterationId, MetadataBag, ProposalBatchId, ProposalId, RunId,
    StageId, Timestamp, now,
};
use thiserror::Error;

use super::indices::GraphIndices;
use crate::{ReadScope, RunEvent};

pub struct CandidateRecord<A: Artifact> {
    pub id: CandidateId,
    pub identity: ArtifactIdentity,
    pub artifact: A,
    pub origin: CandidateOrigin,
    pub created_at: Timestamp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CandidateOrigin {
    Seed {
        seed_index: usize,
    },
    Proposal {
        proposal_id: ProposalId,
        apply_attempt_id: ApplyAttemptId,
    },
}

pub struct ProposalBatchRecord {
    pub id: ProposalBatchId,
    pub stage: StageId,
    pub semantics: ProposalBatchSemantics,
    pub proposal_ids: Vec<ProposalId>,
    pub metadata: MetadataBag,
    pub created_at: Timestamp,
    pub iteration: Option<IterationId>,
}

pub struct ProposalRecord<P: OptimizationProblem> {
    pub id: ProposalId,
    pub batch_id: ProposalBatchId,
    pub effect: ProposalEffect<P>,
    pub provenance: ProposalProvenance,
    pub annotations: P::ProposalAnnotations,
    pub metadata: MetadataBag,
    pub created_at: Timestamp,
}

pub struct ApplyAttemptRecord {
    pub id: ApplyAttemptId,
    pub proposal_id: ProposalId,
    pub outcome: ApplyAttemptOutcome,
    pub created_at: Timestamp,
}

pub enum ApplyAttemptOutcome {
    Success { candidate_id: CandidateId },
    Failure { error: ErrorRecord },
}

pub struct AssessmentRecord {
    pub id: AssessmentId,
    pub request_id: EvaluationRequestId,
    pub evaluator: EvaluatorId,
    pub target: AssessmentRecordTarget,
    pub evidence: EvidenceRef,
    pub metadata: MetadataBag,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug)]
pub enum AssessmentRecordTarget {
    Independent {
        candidate: CandidateId,
        target: AssessmentTarget,
    },
    Pairwise {
        left: CandidateId,
        right: CandidateId,
        target: AssessmentTarget,
    },
    Listwise {
        candidates: Vec<CandidateId>,
        target: AssessmentTarget,
    },
}

pub struct EvaluationRequestRecord {
    pub id: EvaluationRequestId,
    pub evaluator: EvaluatorId,
    pub request: EvaluationRequest,
    pub resolved_set: ResolvedEvaluationSet,
    pub created_at: Timestamp,
}

pub struct RunGraph<P: OptimizationProblem> {
    pub(crate) run_id: RunId,
    pub(crate) candidates: IndexMap<CandidateId, CandidateRecord<P::Artifact>>,
    pub(crate) proposal_batches: IndexMap<ProposalBatchId, ProposalBatchRecord>,
    pub(crate) proposals: IndexMap<ProposalId, ProposalRecord<P>>,
    pub(crate) apply_attempts: IndexMap<ApplyAttemptId, ApplyAttemptRecord>,
    pub(crate) evaluation_requests: IndexMap<EvaluationRequestId, EvaluationRequestRecord>,
    pub(crate) assessments: IndexMap<AssessmentId, AssessmentRecord>,
    pub(crate) events: Vec<RunEvent>,
    pub(crate) indices: GraphIndices,
}

impl<P: OptimizationProblem> RunGraph<P> {
    #[must_use]
    pub fn new(run_id: RunId) -> Self {
        Self {
            run_id,
            candidates: IndexMap::new(),
            proposal_batches: IndexMap::new(),
            proposals: IndexMap::new(),
            apply_attempts: IndexMap::new(),
            evaluation_requests: IndexMap::new(),
            assessments: IndexMap::new(),
            events: Vec::new(),
            indices: GraphIndices::default(),
        }
    }

    pub(crate) fn view(&self, read_scope: ReadScope) -> crate::RunGraphView<'_, P> {
        crate::RunGraphView::new(self, read_scope)
    }

    pub(crate) fn insert_seed(
        &mut self,
        artifact: P::Artifact,
        seed_index: usize,
    ) -> Result<CandidateId, ApplyProposalError> {
        artifact.validate().map_err(ApplyProposalError::artifact)?;
        let id = CandidateId::new();
        let identity = artifact.identity();
        self.candidates.insert(
            id,
            CandidateRecord {
                id,
                identity: identity.clone(),
                artifact,
                origin: CandidateOrigin::Seed { seed_index },
                created_at: now(),
            },
        );
        self.indices
            .by_identity
            .entry(identity)
            .or_default()
            .push(id);
        Ok(id)
    }

    pub(crate) fn record_proposal_batch(
        &mut self,
        stage: StageId,
        batch: ProposalBatch<P>,
        iteration: Option<IterationId>,
    ) -> (ProposalBatchId, Vec<ProposalId>) {
        let batch_id = ProposalBatchId::new();
        let created_at = now();
        let mut proposal_ids = Vec::with_capacity(batch.proposals.len());
        for proposal in batch.proposals {
            let proposal_id = ProposalId::new();
            proposal_ids.push(proposal_id);
            self.proposals.insert(
                proposal_id,
                ProposalRecord {
                    id: proposal_id,
                    batch_id,
                    effect: proposal.effect,
                    provenance: proposal.provenance,
                    annotations: proposal.annotations,
                    metadata: proposal.metadata,
                    created_at,
                },
            );
        }
        self.proposal_batches.insert(
            batch_id,
            ProposalBatchRecord {
                id: batch_id,
                stage,
                semantics: batch.semantics,
                proposal_ids: proposal_ids.clone(),
                metadata: batch.metadata,
                created_at,
                iteration,
            },
        );
        (batch_id, proposal_ids)
    }

    pub(crate) fn apply_proposal_record(&mut self, proposal_id: ProposalId) -> ApplyAttemptRecord {
        let attempt_id = ApplyAttemptId::new();
        let outcome = match self.try_apply_proposal(proposal_id, attempt_id) {
            Ok(candidate_id) => ApplyAttemptOutcome::Success { candidate_id },
            Err(error) => ApplyAttemptOutcome::Failure {
                error: error.to_record(),
            },
        };
        let record = ApplyAttemptRecord {
            id: attempt_id,
            proposal_id,
            outcome,
            created_at: now(),
        };
        self.apply_attempts.insert(attempt_id, record.clone());
        record
    }

    fn try_apply_proposal(
        &mut self,
        proposal_id: ProposalId,
        attempt_id: ApplyAttemptId,
    ) -> Result<CandidateId, ApplyProposalError> {
        if self
            .apply_attempts
            .values()
            .any(|attempt| attempt.proposal_id == proposal_id)
        {
            return Err(ApplyProposalError::AlreadyApplied(proposal_id));
        }
        let proposal = self
            .proposals
            .get(&proposal_id)
            .ok_or(ApplyProposalError::UnknownProposal(proposal_id))?;
        let effect = proposal.effect.clone();
        let provenance = proposal.provenance.clone();
        self.validate_proposal(&effect, &provenance)?;
        let artifact = match &effect {
            ProposalEffect::Create { artifact } => {
                artifact.validate().map_err(ApplyProposalError::artifact)?;
                artifact.clone()
            }
            ProposalEffect::Change { target, change } => {
                let parent = self
                    .candidates
                    .get(target)
                    .ok_or(ApplyProposalError::UnknownCandidate(*target))?
                    .artifact
                    .clone();
                let child = parent
                    .apply_change(change)
                    .map_err(ApplyProposalError::artifact)?;
                child.validate().map_err(ApplyProposalError::artifact)?;
                child
            }
        };
        let candidate_id = CandidateId::new();
        let identity = artifact.identity();
        self.candidates.insert(
            candidate_id,
            CandidateRecord {
                id: candidate_id,
                identity: identity.clone(),
                artifact,
                origin: CandidateOrigin::Proposal {
                    proposal_id,
                    apply_attempt_id: attempt_id,
                },
                created_at: now(),
            },
        );
        self.indices
            .by_identity
            .entry(identity)
            .or_default()
            .push(candidate_id);
        self.indices
            .proposal_by_candidate
            .insert(candidate_id, proposal_id);
        self.index_candidate_lineage(candidate_id, &provenance);
        Ok(candidate_id)
    }

    fn validate_proposal(
        &self,
        effect: &ProposalEffect<P>,
        provenance: &ProposalProvenance,
    ) -> Result<(), ApplyProposalError> {
        match effect {
            ProposalEffect::Create { .. } => match &provenance.causal {
                CausalInputs::None => Ok(()),
                CausalInputs::NAry(parents) => self.validate_existing_candidates(parents),
                CausalInputs::Single(_) | CausalInputs::Pair(_, _) => {
                    Err(ApplyProposalError::InvalidProvenance)
                }
            },
            ProposalEffect::Change { target, .. } => {
                if !provenance.causal.contains_candidate(*target) {
                    return Err(ApplyProposalError::InvalidProvenance);
                }
                let parents: Vec<_> = provenance.causal.iter().collect();
                self.validate_existing_candidates(&parents)
            }
        }
    }

    fn validate_existing_candidates(&self, ids: &[CandidateId]) -> Result<(), ApplyProposalError> {
        for id in ids {
            if !self.candidates.contains_key(id) {
                return Err(ApplyProposalError::UnknownCandidate(*id));
            }
        }
        Ok(())
    }

    fn index_candidate_lineage(
        &mut self,
        candidate_id: CandidateId,
        provenance: &ProposalProvenance,
    ) {
        let parents: Vec<_> = provenance.causal.iter().collect();
        self.indices
            .causal_parents
            .insert(candidate_id, parents.clone());
        for parent in parents {
            self.indices
                .causal_children
                .entry(parent)
                .or_default()
                .push(candidate_id);
        }
        self.indices
            .informed_by
            .insert(candidate_id, provenance.informed_by.clone());
        for info in &provenance.informed_by {
            if let InfoRef::Candidate(source) = info {
                self.indices
                    .informed
                    .entry(*source)
                    .or_default()
                    .push(candidate_id);
            }
        }
    }

    pub(crate) fn record_event(&mut self, event: RunEvent) {
        self.events.push(event);
    }

    pub(crate) fn record_evaluation_request(
        &mut self,
        evaluator: EvaluatorId,
        request: EvaluationRequest,
        resolved_set: ResolvedEvaluationSet,
    ) -> EvaluationRequestId {
        let id = EvaluationRequestId::new();
        self.evaluation_requests.insert(
            id,
            EvaluationRequestRecord {
                id,
                evaluator,
                request,
                resolved_set,
                created_at: now(),
            },
        );
        id
    }

    pub(crate) fn record_assessment(
        &mut self,
        request_id: EvaluationRequestId,
        evaluator: EvaluatorId,
        target: AssessmentRecordTarget,
        metadata: MetadataBag,
        evidence: EvidenceRef,
    ) -> AssessmentId {
        let id = AssessmentId::new();
        self.assessments.insert(
            id,
            AssessmentRecord {
                id,
                request_id,
                evaluator,
                target,
                evidence,
                metadata,
                created_at: now(),
            },
        );
        id
    }

    pub(crate) fn proposal_effect_kind(effect: &ProposalEffect<P>) -> ProposalEffectKind {
        match effect {
            ProposalEffect::Create { .. } => ProposalEffectKind::Create,
            ProposalEffect::Change { .. } => ProposalEffectKind::Change,
        }
    }
}

impl Clone for ApplyAttemptRecord {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            proposal_id: self.proposal_id,
            outcome: self.outcome.clone(),
            created_at: self.created_at,
        }
    }
}

impl Clone for ApplyAttemptOutcome {
    fn clone(&self) -> Self {
        match self {
            Self::Success { candidate_id } => Self::Success {
                candidate_id: *candidate_id,
            },
            Self::Failure { error } => Self::Failure {
                error: error.clone(),
            },
        }
    }
}

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

impl ApplyProposalError {
    fn artifact(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Artifact {
            source: Box::new(err),
        }
    }

    fn to_record(&self) -> ErrorRecord {
        let kind = match self {
            Self::AlreadyApplied(_) | Self::InvalidProvenance => ErrorKind::GraphInvariant,
            Self::UnknownCandidate(_) | Self::UnknownProposal(_) | Self::Artifact { .. } => {
                ErrorKind::Apply
            }
        };
        ErrorRecord::from_error(kind, self)
    }
}
