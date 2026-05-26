use leaven_core::{
    OptimizationProblem, ProposalBatchSemantics, ProposalEffect, ProposalProvenance,
};
use leaven_kernel::{IterationId, MetadataBag, ProposalBatchId, ProposalId, StageId, Timestamp};

use crate::graph::storage::{ProposalBatchRecord, ProposalRecord};

pub struct ProposalBatchView<'g> {
    pub(super) record: &'g ProposalBatchRecord,
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
    pub(super) record: &'g ProposalRecord<P>,
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
