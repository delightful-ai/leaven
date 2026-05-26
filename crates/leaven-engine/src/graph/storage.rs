//! Run-graph storage records and private mutators.

use indexmap::IndexMap;
use leaven_core::{
    Artifact, CausalInputs, EvaluationRequest, InfoRef, OptimizationProblem, ProposalBatch,
    ProposalEffect, ProposalEffectKind, ProposalProvenance, ResolvedEvaluationSet,
};
use leaven_kernel::{
    ApplyAttemptId, AssessmentId, CandidateId, EvaluationRequestId, EvaluatorId, EvidenceRef,
    Fingerprint, IterationId, MetadataBag, ProposalBatchId, ProposalId, RunId, StageId, now,
};
use serde::{Deserialize, Serialize};

use super::indices::GraphIndices;
use crate::{ReadScope, RunEvent};

mod errors;
mod records;

pub use errors::{ApplyProposalError, RunGraphRestoreError};
pub use records::{
    ApplyAttemptOutcome, ApplyAttemptRecord, AssessmentRecord, AssessmentRecordTarget,
    CandidateOrigin, CandidateRecord, EvaluationRequestRecord, ProposalBatchRecord, ProposalRecord,
};

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

#[derive(Serialize, Deserialize)]
#[serde(bound(
    serialize = "P::Artifact: Serialize, <P::Artifact as Artifact>::Change: Serialize, P::ProposalAnnotations: Serialize",
    deserialize = "P::Artifact: Deserialize<'de>, <P::Artifact as Artifact>::Change: Deserialize<'de>, P::ProposalAnnotations: Deserialize<'de>"
))]
pub struct RunGraphSnapshot<P: OptimizationProblem> {
    pub run_id: RunId,
    pub candidates: Vec<CandidateRecord<P::Artifact>>,
    pub proposal_batches: Vec<ProposalBatchRecord>,
    pub proposals: Vec<ProposalRecord<P>>,
    pub apply_attempts: Vec<ApplyAttemptRecord>,
    pub evaluation_requests: Vec<EvaluationRequestRecord>,
    pub assessments: Vec<AssessmentRecord>,
    pub events: Vec<RunEvent>,
}

impl<P: OptimizationProblem> Clone for RunGraphSnapshot<P> {
    fn clone(&self) -> Self {
        Self {
            run_id: self.run_id,
            candidates: self.candidates.clone(),
            proposal_batches: self.proposal_batches.clone(),
            proposals: self.proposals.clone(),
            apply_attempts: self.apply_attempts.clone(),
            evaluation_requests: self.evaluation_requests.clone(),
            assessments: self.assessments.clone(),
            events: self.events.clone(),
        }
    }
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

    #[must_use]
    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }

    #[must_use]
    pub fn snapshot(&self) -> RunGraphSnapshot<P>
    where
        P::Artifact: Serialize,
        <P::Artifact as Artifact>::Change: Serialize,
        P::ProposalAnnotations: Serialize,
    {
        RunGraphSnapshot {
            run_id: self.run_id,
            candidates: self.candidates.values().cloned().collect(),
            proposal_batches: self.proposal_batches.values().cloned().collect(),
            proposals: self.proposals.values().cloned().collect(),
            apply_attempts: self.apply_attempts.values().cloned().collect(),
            evaluation_requests: self.evaluation_requests.values().cloned().collect(),
            assessments: self.assessments.values().cloned().collect(),
            events: self.events.clone(),
        }
    }

    pub fn from_snapshot(snapshot: RunGraphSnapshot<P>) -> Result<Self, RunGraphRestoreError>
    where
        P::Artifact: for<'de> Deserialize<'de>,
        <P::Artifact as Artifact>::Change: for<'de> Deserialize<'de>,
        P::ProposalAnnotations: for<'de> Deserialize<'de>,
    {
        let mut graph = Self::new(snapshot.run_id);
        for candidate in snapshot.candidates {
            insert_unique(
                &mut graph.candidates,
                candidate.id,
                candidate,
                RunGraphRestoreError::DuplicateCandidate,
            )?;
        }
        for batch in snapshot.proposal_batches {
            insert_unique(
                &mut graph.proposal_batches,
                batch.id,
                batch,
                RunGraphRestoreError::DuplicateProposalBatch,
            )?;
        }
        for proposal in snapshot.proposals {
            insert_unique(
                &mut graph.proposals,
                proposal.id,
                proposal,
                RunGraphRestoreError::DuplicateProposal,
            )?;
        }
        for attempt in snapshot.apply_attempts {
            insert_unique(
                &mut graph.apply_attempts,
                attempt.id,
                attempt,
                RunGraphRestoreError::DuplicateApplyAttempt,
            )?;
        }
        for request in snapshot.evaluation_requests {
            insert_unique(
                &mut graph.evaluation_requests,
                request.id,
                request,
                RunGraphRestoreError::DuplicateEvaluationRequest,
            )?;
        }
        for assessment in snapshot.assessments {
            insert_unique(
                &mut graph.assessments,
                assessment.id,
                assessment,
                RunGraphRestoreError::DuplicateAssessment,
            )?;
        }
        graph.events = snapshot.events;
        graph.validate_restored_references()?;
        graph.rebuild_indices()?;
        Ok(graph)
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
            ProposalEffect::Create { .. } => match provenance.causal() {
                CausalInputs::None => Ok(()),
                CausalInputs::NAry(parents) => self.validate_existing_candidates(parents),
                CausalInputs::Single(_) | CausalInputs::Pair(_, _) => {
                    Err(ApplyProposalError::InvalidProvenance)
                }
            },
            ProposalEffect::Change { target, .. } => {
                if !provenance.causal().contains_candidate(*target) {
                    return Err(ApplyProposalError::InvalidProvenance);
                }
                let parents: Vec<_> = provenance.causal().iter().collect();
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
        let parents: Vec<_> = provenance.causal().iter().collect();
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
            .insert(candidate_id, provenance.informed_by_refs().to_vec());
        for info in provenance.informed_by_refs() {
            if let InfoRef::Candidate(source) = info {
                self.indices
                    .informed
                    .entry(*source)
                    .or_default()
                    .push(candidate_id);
            }
        }
    }

    fn rebuild_indices(&mut self) -> Result<(), RunGraphRestoreError> {
        self.indices = GraphIndices::default();
        for (candidate_id, candidate) in self.candidates.clone() {
            self.indices
                .by_identity
                .entry(candidate.identity.clone())
                .or_default()
                .push(candidate_id);
            if let CandidateOrigin::Proposal { proposal_id, .. } = candidate.origin {
                self.indices
                    .proposal_by_candidate
                    .insert(candidate_id, proposal_id);
                let provenance = self
                    .proposals
                    .get(&proposal_id)
                    .ok_or(RunGraphRestoreError::MissingProposalForCandidate {
                        candidate: candidate_id,
                        proposal: proposal_id,
                    })?
                    .provenance
                    .clone();
                self.index_candidate_lineage(candidate_id, &provenance);
            }
        }
        for (assessment_id, assessment) in &self.assessments {
            for candidate in assessment.target.candidates() {
                self.indices
                    .assessments_by_candidate
                    .entry(candidate)
                    .or_default()
                    .push(*assessment_id);
            }
        }
        Ok(())
    }

    fn validate_restored_references(&self) -> Result<(), RunGraphRestoreError> {
        self.validate_restored_batches()?;
        self.validate_restored_proposals()?;
        self.validate_restored_apply_attempts()?;
        self.validate_restored_candidate_origins()?;
        self.validate_restored_assessments()
    }

    fn validate_restored_batches(&self) -> Result<(), RunGraphRestoreError> {
        for (batch_id, batch) in &self.proposal_batches {
            for proposal_id in &batch.proposal_ids {
                if !self.proposals.contains_key(proposal_id) {
                    return Err(RunGraphRestoreError::MissingProposalInBatch {
                        batch: *batch_id,
                        proposal: *proposal_id,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_restored_proposals(&self) -> Result<(), RunGraphRestoreError> {
        for (proposal_id, proposal) in &self.proposals {
            let batch = self.proposal_batches.get(&proposal.batch_id).ok_or(
                RunGraphRestoreError::MissingBatchForProposal {
                    proposal: *proposal_id,
                    batch: proposal.batch_id,
                },
            )?;
            if !batch.proposal_ids.contains(proposal_id) {
                return Err(RunGraphRestoreError::ProposalNotListedInBatch {
                    proposal: *proposal_id,
                    batch: proposal.batch_id,
                });
            }
            self.validate_proposal(&proposal.effect, &proposal.provenance)
                .map_err(|source| RunGraphRestoreError::InvalidRestoredProposal {
                    proposal: *proposal_id,
                    reason: source.to_string(),
                })?;
        }
        Ok(())
    }

    fn validate_restored_apply_attempts(&self) -> Result<(), RunGraphRestoreError> {
        for (attempt_id, attempt) in &self.apply_attempts {
            if !self.proposals.contains_key(&attempt.proposal_id) {
                return Err(RunGraphRestoreError::MissingProposalForApplyAttempt {
                    attempt: *attempt_id,
                    proposal: attempt.proposal_id,
                });
            }
            if let ApplyAttemptOutcome::Success { candidate_id } = &attempt.outcome {
                let candidate = self.candidates.get(candidate_id).ok_or(
                    RunGraphRestoreError::MissingCandidateForSuccessfulApplyAttempt {
                        attempt: *attempt_id,
                        candidate: *candidate_id,
                    },
                )?;
                match candidate.origin {
                    CandidateOrigin::Proposal {
                        proposal_id,
                        apply_attempt_id,
                    } if proposal_id == attempt.proposal_id && apply_attempt_id == *attempt_id => {}
                    _ => {
                        return Err(RunGraphRestoreError::ApplyAttemptCandidateMismatch {
                            attempt: *attempt_id,
                            candidate: *candidate_id,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_restored_candidate_origins(&self) -> Result<(), RunGraphRestoreError> {
        for (candidate_id, candidate) in &self.candidates {
            if let CandidateOrigin::Proposal {
                proposal_id,
                apply_attempt_id,
            } = candidate.origin
            {
                if !self.proposals.contains_key(&proposal_id) {
                    return Err(RunGraphRestoreError::MissingProposalForCandidate {
                        candidate: *candidate_id,
                        proposal: proposal_id,
                    });
                }
                let attempt = self.apply_attempts.get(&apply_attempt_id).ok_or(
                    RunGraphRestoreError::MissingApplyAttemptForCandidate {
                        candidate: *candidate_id,
                        attempt: apply_attempt_id,
                    },
                )?;
                if attempt.proposal_id != proposal_id {
                    return Err(RunGraphRestoreError::CandidateApplyAttemptMismatch {
                        candidate: *candidate_id,
                        attempt: apply_attempt_id,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_restored_assessments(&self) -> Result<(), RunGraphRestoreError> {
        for (assessment_id, assessment) in &self.assessments {
            if !self
                .evaluation_requests
                .contains_key(&assessment.request_id)
            {
                return Err(
                    RunGraphRestoreError::MissingEvaluationRequestForAssessment {
                        assessment: *assessment_id,
                        request: assessment.request_id,
                    },
                );
            }
            for candidate in assessment.target.candidates() {
                if !self.candidates.contains_key(&candidate) {
                    return Err(RunGraphRestoreError::MissingCandidateForAssessment {
                        assessment: *assessment_id,
                        candidate,
                    });
                }
            }
        }
        Ok(())
    }

    pub(crate) fn record_event(&mut self, event: RunEvent) {
        self.events.push(event);
    }

    pub(crate) fn record_evaluation_request(
        &mut self,
        evaluator: EvaluatorId,
        evaluator_fingerprint: Fingerprint,
        request: EvaluationRequest,
        resolved_set: ResolvedEvaluationSet,
    ) -> EvaluationRequestId {
        let id = EvaluationRequestId::new();
        self.evaluation_requests.insert(
            id,
            EvaluationRequestRecord {
                id,
                evaluator,
                evaluator_fingerprint,
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
        for candidate in target.candidates() {
            self.indices
                .assessments_by_candidate
                .entry(candidate)
                .or_default()
                .push(id);
        }
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

fn insert_unique<K, V, F>(
    map: &mut indexmap::IndexMap<K, V>,
    key: K,
    value: V,
    error: F,
) -> Result<(), RunGraphRestoreError>
where
    K: std::hash::Hash + Eq + Copy,
    F: FnOnce(K) -> RunGraphRestoreError,
{
    if map.contains_key(&key) {
        return Err(error(key));
    }
    map.insert(key, value);
    Ok(())
}
