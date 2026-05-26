use leaven_core::{ArtifactIdentity, OptimizationProblem};
use leaven_kernel::{CandidateId, Timestamp};

use crate::graph::storage::{CandidateOrigin, CandidateRecord, RunGraph};

pub struct CandidateView<'g, P: OptimizationProblem> {
    pub(super) record: &'g CandidateRecord<P::Artifact>,
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

pub struct CandidateTree<'g, P: OptimizationProblem> {
    pub(super) graph: &'g RunGraph<P>,
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

pub struct Lineage<'g, P: OptimizationProblem> {
    pub(super) graph: &'g RunGraph<P>,
    pub(super) root: CandidateId,
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
