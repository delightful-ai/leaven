//! Read-only graph views.

use leaven_core::{
    ArtifactIdentity, AssessmentTarget, EvaluationRequest, EvaluationSet, InfoRef,
    OptimizationProblem, ProposalBatchSemantics, ProposalEffect, ProposalProvenance,
    ResolvedEvaluationSet, Window,
};
use leaven_kernel::{
    ApplyAttemptId, AssessmentId, CandidateId, ErrorRecord, EvaluationRequestId, EvaluatorId,
    EvidenceRef, IterationId, MetadataBag, ProposalBatchId, ProposalId, StageId, Timestamp,
};

use super::storage::{
    AssessmentRecord, AssessmentRecordTarget, CandidateOrigin, EvaluationRequestRecord,
    ProposalBatchRecord, ProposalRecord, RunGraph,
};
use crate::{ReadScope, RunEvent};

pub struct RunGraphView<'g, P: OptimizationProblem> {
    graph: &'g RunGraph<P>,
    read_scope: ReadScope,
}

impl<P: OptimizationProblem> Clone for RunGraphView<'_, P> {
    fn clone(&self) -> Self {
        Self {
            graph: self.graph,
            read_scope: self.read_scope.clone(),
        }
    }
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
    pub fn lineage(&self, id: CandidateId) -> Lineage<'g, P> {
        Lineage {
            graph: self.graph,
            root: id,
        }
    }

    #[must_use]
    pub fn siblings(&self, id: CandidateId) -> Vec<CandidateId> {
        let mut siblings = Vec::new();
        for parent in self.parents(id) {
            for child in self.children(parent) {
                if child != id && !siblings.contains(&child) {
                    siblings.push(child);
                }
            }
        }
        siblings
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
        if !self.allows_assessment(record) {
            return None;
        }
        Some(AssessmentView { record })
    }

    #[must_use]
    pub fn assessments(&self, id: CandidateId) -> AssessmentQuery<'g> {
        let assessments = self
            .graph
            .indices
            .assessments_by_candidate
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|assessment_id| self.assessment(*assessment_id))
            .collect();
        AssessmentQuery { assessments }
    }

    #[must_use]
    pub fn pairwise_assessments(
        &self,
        left: CandidateId,
        right: CandidateId,
    ) -> AssessmentQuery<'g> {
        let assessments = self
            .graph
            .indices
            .assessments_by_candidate
            .get(&left)
            .into_iter()
            .flatten()
            .filter_map(|assessment_id| self.assessment(*assessment_id))
            .filter(|assessment| assessment.pairwise_candidates() == Some((left, right)))
            .collect();
        AssessmentQuery { assessments }
    }

    #[must_use]
    pub fn evaluation_request(&self, id: EvaluationRequestId) -> Option<EvaluationRequestView<'g>> {
        self.graph
            .evaluation_requests
            .get(&id)
            .filter(|record| self.allows_evaluation_set(&record.resolved_set.expr))
            .map(|record| EvaluationRequestView { record })
    }

    pub fn events(&self) -> impl Iterator<Item = &'g RunEvent> {
        self.graph.events.iter()
    }

    #[must_use]
    pub fn recent_failures(&self, window: Window) -> Vec<FailureRef<'g>> {
        let limit = std::convert::identity(window).limit;
        self.graph
            .apply_attempts
            .values()
            .rev()
            .filter_map(|record| match &record.outcome {
                super::storage::ApplyAttemptOutcome::Failure { .. } => Some(FailureRef { record }),
                super::storage::ApplyAttemptOutcome::Success { .. } => None,
            })
            .take(limit)
            .collect()
    }

    #[must_use]
    pub fn candidate_tree(&self) -> CandidateTree<'g, P> {
        CandidateTree { graph: self.graph }
    }

    fn allows_assessment(&self, record: &AssessmentRecord) -> bool {
        self.graph
            .evaluation_requests
            .get(&record.request_id)
            .is_some_and(|request| self.allows_evaluation_set(&request.resolved_set.expr))
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

    #[must_use]
    pub fn stage(&self) -> &StageId {
        &self.record.stage
    }

    #[must_use]
    pub fn metadata(&self) -> &MetadataBag {
        &self.record.metadata
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.record.created_at
    }

    #[must_use]
    pub const fn iteration(&self) -> Option<IterationId> {
        self.record.iteration
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

    #[must_use]
    pub fn batch_id(&self) -> ProposalBatchId {
        self.record.batch_id
    }

    #[must_use]
    pub fn effect(&self) -> &ProposalEffect<P> {
        &self.record.effect
    }

    #[must_use]
    pub fn provenance(&self) -> &ProposalProvenance {
        &self.record.provenance
    }

    #[must_use]
    pub fn annotations(&self) -> &P::ProposalAnnotations {
        &self.record.annotations
    }

    #[must_use]
    pub fn metadata(&self) -> &MetadataBag {
        &self.record.metadata
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.record.created_at
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
    pub fn evaluator(&self) -> &EvaluatorId {
        &self.record.evaluator
    }

    #[must_use]
    pub fn metadata(&self) -> &MetadataBag {
        &self.record.metadata
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.record.created_at
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

    #[must_use]
    pub fn pairwise_candidates(&self) -> Option<(CandidateId, CandidateId)> {
        match &self.record.target {
            AssessmentRecordTarget::Pairwise { left, right, .. } => Some((*left, *right)),
            AssessmentRecordTarget::Independent { .. }
            | AssessmentRecordTarget::Listwise { .. } => None,
        }
    }

    #[must_use]
    pub fn listwise_candidates(&self) -> Option<&[CandidateId]> {
        match &self.record.target {
            AssessmentRecordTarget::Listwise { candidates, .. } => Some(candidates),
            AssessmentRecordTarget::Independent { .. }
            | AssessmentRecordTarget::Pairwise { .. } => None,
        }
    }
}

pub struct AssessmentQuery<'g> {
    assessments: Vec<AssessmentView<'g>>,
}

impl<'g> AssessmentQuery<'g> {
    #[must_use]
    pub fn len(&self) -> usize {
        self.assessments.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.assessments.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &AssessmentView<'g>> {
        self.assessments.iter()
    }

    #[must_use]
    pub fn ids(&self) -> Vec<AssessmentId> {
        self.assessments.iter().map(AssessmentView::id).collect()
    }
}

pub struct EvaluationRequestView<'g> {
    record: &'g EvaluationRequestRecord,
}

impl EvaluationRequestView<'_> {
    #[must_use]
    pub fn id(&self) -> EvaluationRequestId {
        self.record.id
    }

    #[must_use]
    pub fn evaluator(&self) -> &EvaluatorId {
        &self.record.evaluator
    }

    #[must_use]
    pub fn request(&self) -> &EvaluationRequest {
        &self.record.request
    }

    #[must_use]
    pub fn resolved_set(&self) -> &ResolvedEvaluationSet {
        &self.record.resolved_set
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.record.created_at
    }
}

pub struct CandidateTree<'g, P: OptimizationProblem> {
    graph: &'g RunGraph<P>,
}

impl<P: OptimizationProblem> CandidateTree<'_, P> {
    #[must_use]
    pub fn contains(&self, id: CandidateId) -> bool {
        self.graph.candidates.contains_key(&id)
    }

    #[must_use]
    pub fn roots(&self) -> Vec<CandidateId> {
        self.graph
            .candidates
            .keys()
            .filter(|id| {
                self.graph
                    .indices
                    .causal_parents
                    .get(id)
                    .is_none_or(Vec::is_empty)
            })
            .copied()
            .collect()
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
}

pub struct FailureRef<'g> {
    record: &'g super::storage::ApplyAttemptRecord,
}

impl FailureRef<'_> {
    #[must_use]
    pub fn id(&self) -> ApplyAttemptId {
        self.record.id
    }

    #[must_use]
    pub fn proposal_id(&self) -> ProposalId {
        self.record.proposal_id
    }

    #[must_use]
    pub fn error(&self) -> &ErrorRecord {
        match &self.record.outcome {
            super::storage::ApplyAttemptOutcome::Failure { error } => error,
            super::storage::ApplyAttemptOutcome::Success { .. } => {
                unreachable!("FailureRef is only constructed from failed apply attempts")
            }
        }
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.record.created_at
    }
}

pub struct Lineage<'g, P: OptimizationProblem> {
    graph: &'g RunGraph<P>,
    root: CandidateId,
}

impl<P: OptimizationProblem> Lineage<'_, P> {
    #[must_use]
    pub fn root(&self) -> CandidateId {
        self.root
    }

    #[must_use]
    pub fn parents(&self) -> Vec<CandidateId> {
        self.graph
            .indices
            .causal_parents
            .get(&self.root)
            .cloned()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn ancestors(&self) -> Vec<CandidateId> {
        let mut ancestors = Vec::new();
        let mut queue = self.parents();
        let mut index = 0;
        while let Some(id) = queue.get(index).copied() {
            index += 1;
            if ancestors.contains(&id) {
                continue;
            }
            ancestors.push(id);
            if let Some(parents) = self.graph.indices.causal_parents.get(&id) {
                queue.extend(parents.iter().copied());
            }
        }
        ancestors
    }

    #[must_use]
    pub fn contains(&self, id: CandidateId) -> bool {
        self.root == id || self.ancestors().contains(&id)
    }
}

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
