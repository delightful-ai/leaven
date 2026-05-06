//! Read-only graph views.

use leaven_core::{
    ArtifactIdentity, AssessmentTarget, EvaluationSet, InfoRef, OptimizationProblem,
    ProposalBatchSemantics,
};
use leaven_kernel::{
    AssessmentId, CandidateId, EvidenceRef, ProposalBatchId, ProposalId, Timestamp,
};

use super::storage::{
    AssessmentRecord, AssessmentRecordTarget, CandidateOrigin, ProposalBatchRecord, ProposalRecord,
    RunGraph,
};
use crate::{ReadScope, RunEvent};

pub struct RunGraphView<'g, P: OptimizationProblem> {
    graph: &'g RunGraph<P>,
    read_scope: ReadScope,
}

impl<'g, P: OptimizationProblem> RunGraphView<'g, P> {
    #[must_use]
    pub(crate) fn new(graph: &'g RunGraph<P>, read_scope: ReadScope) -> Self {
        Self { graph, read_scope }
    }

    #[must_use]
    pub fn read_scope(&self) -> &ReadScope {
        &self.read_scope
    }

    #[must_use]
    pub fn candidate(&self, id: CandidateId) -> Option<CandidateView<'g, P>> {
        self.graph
            .candidates
            .get(&id)
            .map(|record| CandidateView { record })
    }

    #[must_use]
    pub fn candidate_count(&self) -> usize {
        self.graph.candidates.len()
    }

    #[must_use]
    pub fn artifact(&self, id: CandidateId) -> Option<&'g P::Artifact> {
        self.graph
            .candidates
            .get(&id)
            .map(|candidate| &candidate.artifact)
    }

    #[must_use]
    pub fn candidates_with_identity(&self, identity: &ArtifactIdentity) -> Vec<CandidateId> {
        self.graph
            .indices
            .by_identity
            .get(identity)
            .cloned()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn parents(&self, id: CandidateId) -> Vec<CandidateId> {
        self.graph
            .indices
            .causal_parents
            .get(&id)
            .cloned()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn children(&self, id: CandidateId) -> Vec<CandidateId> {
        self.graph
            .indices
            .causal_children
            .get(&id)
            .cloned()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn informed_by(&self, id: CandidateId) -> Vec<InfoRef> {
        self.graph
            .indices
            .informed_by
            .get(&id)
            .cloned()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn informed(&self, id: CandidateId) -> Vec<CandidateId> {
        self.graph
            .indices
            .informed
            .get(&id)
            .cloned()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn proposal_batch(&self, id: ProposalBatchId) -> Option<ProposalBatchView<'g>> {
        self.graph
            .proposal_batches
            .get(&id)
            .map(|record| ProposalBatchView { record })
    }

    #[must_use]
    pub fn proposal_that_created(&self, id: CandidateId) -> Option<ProposalView<'g, P>> {
        self.graph
            .indices
            .proposal_by_candidate
            .get(&id)
            .and_then(|proposal_id| self.graph.proposals.get(proposal_id))
            .map(|record| ProposalView { record })
    }

    #[must_use]
    pub fn proposal_batch_count(&self) -> usize {
        self.graph.proposal_batches.len()
    }

    #[must_use]
    pub fn proposal_count(&self) -> usize {
        self.graph.proposals.len()
    }

    #[must_use]
    pub fn apply_attempt_count(&self) -> usize {
        self.graph.apply_attempts.len()
    }

    #[must_use]
    pub fn evaluation_request_count(&self) -> usize {
        self.graph.evaluation_requests.len()
    }

    #[must_use]
    pub fn assessment_count(&self) -> usize {
        self.graph.assessments.len()
    }

    #[must_use]
    pub fn event_count(&self) -> usize {
        self.graph.events.len()
    }

    #[must_use]
    pub fn assessment(&self, id: AssessmentId) -> Option<AssessmentView<'g>> {
        let record = self.graph.assessments.get(&id)?;
        let request = self.graph.evaluation_requests.get(&record.request_id)?;
        if !self.allows_evaluation_set(&request.resolved_set.expr) {
            return None;
        }
        Some(AssessmentView { record })
    }

    pub fn events(&self) -> impl Iterator<Item = &'g RunEvent> {
        self.graph.events.iter()
    }

    fn allows_evaluation_set(&self, set: &EvaluationSet) -> bool {
        !references_hidden_partition(set, &self.read_scope.hidden_partitions)
    }
}

pub struct CandidateView<'g, P: OptimizationProblem> {
    record: &'g super::storage::CandidateRecord<P::Artifact>,
}

impl<'g, P: OptimizationProblem> CandidateView<'g, P> {
    #[must_use]
    pub fn id(&self) -> CandidateId {
        self.record.id
    }

    #[must_use]
    pub fn identity(&self) -> &'g ArtifactIdentity {
        &self.record.identity
    }

    #[must_use]
    pub fn origin(&self) -> CandidateOrigin {
        self.record.origin
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.record.created_at
    }
}

pub struct ProposalBatchView<'g> {
    record: &'g ProposalBatchRecord,
}

impl ProposalBatchView<'_> {
    #[must_use]
    pub fn id(&self) -> ProposalBatchId {
        self.record.id
    }

    #[must_use]
    pub fn semantics(&self) -> ProposalBatchSemantics {
        self.record.semantics
    }

    #[must_use]
    pub fn proposal_ids(&self) -> &[ProposalId] {
        &self.record.proposal_ids
    }
}

pub struct ProposalView<'g, P: OptimizationProblem> {
    record: &'g ProposalRecord<P>,
}

impl<P: OptimizationProblem> ProposalView<'_, P> {
    #[must_use]
    pub fn id(&self) -> ProposalId {
        self.record.id
    }
}

pub struct AssessmentView<'g> {
    record: &'g AssessmentRecord,
}

impl AssessmentView<'_> {
    #[must_use]
    pub fn id(&self) -> AssessmentId {
        self.record.id
    }

    #[must_use]
    pub fn request_id(&self) -> leaven_kernel::EvaluationRequestId {
        self.record.request_id
    }

    #[must_use]
    pub fn evidence_ref(&self) -> &EvidenceRef {
        &self.record.evidence
    }

    #[must_use]
    pub fn independent_candidate(&self) -> Option<CandidateId> {
        match &self.record.target {
            AssessmentRecordTarget::Independent { candidate, .. } => Some(*candidate),
            AssessmentRecordTarget::Pairwise { .. } | AssessmentRecordTarget::Listwise { .. } => {
                None
            }
        }
    }

    #[must_use]
    pub fn target(&self) -> &AssessmentTarget {
        match &self.record.target {
            AssessmentRecordTarget::Independent { target, .. }
            | AssessmentRecordTarget::Pairwise { target, .. }
            | AssessmentRecordTarget::Listwise { target, .. } => target,
        }
    }
}
pub struct AssessmentQuery;
pub struct CandidateTree;
pub struct FailureRef;
pub struct Lineage;

fn references_hidden_partition(
    set: &EvaluationSet,
    hidden: &std::collections::BTreeSet<leaven_core::PartitionId>,
) -> bool {
    match set {
        EvaluationSet::Partition(partition) => hidden.contains(partition),
        EvaluationSet::All => !hidden.is_empty(),
        EvaluationSet::Sample { of, .. } | EvaluationSet::Stratified { of, .. } => {
            references_hidden_partition(of, hidden)
        }
        EvaluationSet::Union(sets) | EvaluationSet::Intersect(sets) => sets
            .iter()
            .any(|set| references_hidden_partition(set, hidden)),
        EvaluationSet::Difference(left, right) => {
            references_hidden_partition(left, hidden) || references_hidden_partition(right, hidden)
        }
        EvaluationSet::Unscoped
        | EvaluationSet::Cases(_)
        | EvaluationSet::Tagged(_)
        | EvaluationSet::Recent { .. } => false,
    }
}
