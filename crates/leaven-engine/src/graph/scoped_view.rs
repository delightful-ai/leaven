//! Stage-scoped graph view.

use leaven_core::OptimizationProblem;
use leaven_kernel::{AssessmentId, CandidateId};

use crate::{
    AssessmentQuery, AssessmentView, CandidateView, EvidenceVisibility, ReadScope, RunGraphView,
};

/// Graph reads exposed to optimizer-stage workspaces.
///
/// This wraps an already read-scoped [`RunGraphView`] and deliberately does not
/// expose the underlying graph view. Stage adapters should receive this type,
/// not a raw `RunGraphView`.
pub struct ScopedRunGraphView<'g, P: OptimizationProblem> {
    graph: RunGraphView<'g, P>,
    read_scope: ReadScope,
}

impl<P: OptimizationProblem> Clone for ScopedRunGraphView<'_, P> {
    fn clone(&self) -> Self {
        Self {
            graph: self.graph.clone(),
            read_scope: self.read_scope.clone(),
        }
    }
}

impl<'g, P: OptimizationProblem> ScopedRunGraphView<'g, P> {
    #[must_use]
    pub(crate) fn new(graph: RunGraphView<'g, P>, read_scope: ReadScope) -> Self {
        Self { graph, read_scope }
    }

    #[must_use]
    pub fn read_scope(&self) -> &ReadScope {
        &self.read_scope
    }

    #[must_use]
    pub fn visible_evidence(&self) -> EvidenceVisibility {
        self.read_scope.visible_evidence
    }

    #[must_use]
    pub fn candidate(&self, id: CandidateId) -> Option<CandidateView<'g, P>> {
        self.graph.candidate(id)
    }

    #[must_use]
    pub fn artifact(&self, id: CandidateId) -> Option<&'g P::Artifact> {
        self.graph.artifact(id)
    }

    #[must_use]
    pub fn assessment(&self, id: AssessmentId) -> Option<AssessmentView<'g>> {
        self.graph.assessment(id)
    }

    #[must_use]
    pub fn assessments_for_candidate(&self, id: CandidateId) -> AssessmentQuery<'g> {
        self.graph.assessments(id)
    }

    #[must_use]
    pub fn candidate_count(&self) -> usize {
        self.graph.candidate_count()
    }
}
