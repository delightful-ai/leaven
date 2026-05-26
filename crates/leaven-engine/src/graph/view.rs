//! Read-only graph views.

mod assessment;
mod candidate;
mod failure;
mod proposal;

pub use assessment::{AssessmentQuery, AssessmentView, EvaluationRequestView};
pub use candidate::{CandidateTree, CandidateView, Lineage};
pub use failure::FailureRef;
pub use proposal::{ProposalBatchView, ProposalView};

use leaven_core::{ArtifactIdentity, EvaluationSet, InfoRef, OptimizationProblem, Window};
use leaven_kernel::{
    AssessmentId, CandidateId, EvaluationRequestId, ProposalBatchId, ProposalId, RunId,
};

use super::storage::{AssessmentRecord, RunGraph};
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
    pub const fn run_id(&self) -> RunId {
        self.graph.run_id
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

    pub fn proposal_batches(&self) -> impl Iterator<Item = ProposalBatchView<'g>> + '_ {
        self.graph
            .proposal_batches
            .values()
            .map(|record| ProposalBatchView { record })
    }

    #[must_use]
    pub fn proposal(&self, id: ProposalId) -> Option<ProposalView<'g, P>> {
        self.graph
            .proposals
            .get(&id)
            .map(|record| ProposalView { record })
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

    pub fn all_assessments(&self) -> impl Iterator<Item = AssessmentView<'g>> + '_ {
        self.graph
            .assessments
            .values()
            .filter(|record| self.allows_assessment(record))
            .map(|record| AssessmentView { record })
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
