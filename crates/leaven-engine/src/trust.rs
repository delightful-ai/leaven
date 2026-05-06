//! Trust and read-scope policy.

use std::collections::BTreeSet;

use leaven_core::{EvaluationRequest, EvaluationSet, PartitionId};
use leaven_kernel::{EvaluatorId, ProposerId, RendererId};

#[derive(Clone, Debug, Default)]
pub struct ReadScope {
    pub hidden_partitions: BTreeSet<PartitionId>,
    pub visible_evidence: EvidenceVisibility,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum EvidenceVisibility {
    #[default]
    Full,
    ScoresOnly,
    SummariesOnly,
    None,
}

#[derive(Clone, Debug, Default)]
pub struct TrustPolicy {
    hidden_from_proposers: Vec<PartitionId>,
    hidden_from_optimizers: Vec<PartitionId>,
}

impl TrustPolicy {
    #[must_use]
    pub fn hide_from_proposers(
        mut self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> Self {
        self.hidden_from_proposers.extend(partitions);
        self
    }

    #[must_use]
    pub fn hide_from_optimizers(
        mut self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> Self {
        self.hidden_from_optimizers.extend(partitions);
        self
    }

    #[must_use]
    pub fn proposer_read_scope(&self) -> ReadScope {
        ReadScope {
            hidden_partitions: self.hidden_from_proposers.iter().cloned().collect(),
            visible_evidence: EvidenceVisibility::Full,
        }
    }

    #[must_use]
    pub fn optimizer_read_scope(&self) -> ReadScope {
        ReadScope {
            hidden_partitions: self.hidden_from_optimizers.iter().cloned().collect(),
            visible_evidence: EvidenceVisibility::Full,
        }
    }

    #[must_use]
    pub fn evaluator_read_scope(&self) -> ReadScope {
        ReadScope::default()
    }

    pub fn check_evaluation_request(&self, actor: &Actor, request: &EvaluationRequest) -> bool {
        let hidden = match actor {
            Actor::Optimizer => &self.hidden_from_optimizers,
            Actor::Proposer(_) => &self.hidden_from_proposers,
            Actor::Evaluator(_) | Actor::Renderer(_) | Actor::Callback => return true,
        };
        let set = match request {
            EvaluationRequest::Independent { set, .. }
            | EvaluationRequest::Pairwise { set, .. }
            | EvaluationRequest::Listwise { set, .. } => set,
        };
        !references_hidden_partition(set, hidden)
    }
}

fn references_hidden_partition(set: &EvaluationSet, hidden: &[PartitionId]) -> bool {
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

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Actor {
    Optimizer,
    Proposer(ProposerId),
    Evaluator(EvaluatorId),
    Renderer(RendererId),
    Callback,
}

pub struct EvalHandle;
pub struct ProbeRecorder;
