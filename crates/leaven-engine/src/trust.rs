//! Trust and read-scope policy.

use std::collections::BTreeSet;

use leaven_core::{EvaluationPurpose, EvaluationRequest, EvaluationSet, PartitionId};
use leaven_kernel::{CaseId, EvaluatorId, ProposerId, RendererId};

use crate::CaseSet;

/// Read authority carried by graph views and stage contexts.
///
/// Hidden partitions are excluded from assessment and evaluation-request
/// queries. Evidence visibility is carried separately so renderers and future
/// evidence-loading APIs can degrade what they expose without changing which
/// graph records are visible.
#[derive(Clone, Debug, Default)]
pub struct ReadScope {
    /// Case-set partitions hidden from this actor.
    pub hidden_partitions: BTreeSet<PartitionId>,
    /// Level of evidence detail visible to this actor.
    pub visible_evidence: EvidenceVisibility,
}

/// Evidence detail an actor may observe through read-scoped surfaces.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum EvidenceVisibility {
    /// Full evidence records may be rendered or loaded.
    #[default]
    Full,
    /// Only score-like evidence projections may be exposed.
    ScoresOnly,
    /// Only summary evidence projections may be exposed.
    SummariesOnly,
    /// No evidence payload should be exposed.
    None,
}

/// Trust policy for actor-specific read scopes and evaluation requests.
#[derive(Clone, Debug, Default)]
pub struct TrustPolicy {
    hidden: HiddenPartitions,
}

#[derive(Clone, Debug, Default)]
struct HiddenPartitions {
    proposers: Vec<PartitionId>,
    optimizers: Vec<PartitionId>,
    callbacks: Vec<PartitionId>,
}

impl TrustPolicy {
    /// Hide partitions from proposer contexts.
    #[must_use]
    pub fn hide_from_proposers(
        mut self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> Self {
        self.hidden.proposers.extend(partitions);
        self
    }

    /// Hide partitions from optimizer contexts.
    #[must_use]
    pub fn hide_from_optimizers(
        mut self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> Self {
        self.hidden.optimizers.extend(partitions);
        self
    }

    /// Hide partitions from callback graph views.
    #[must_use]
    pub fn hide_from_callbacks(
        mut self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> Self {
        self.hidden.callbacks.extend(partitions);
        self
    }

    /// Read scope for proposer contexts.
    #[must_use]
    pub fn proposer_read_scope(&self) -> ReadScope {
        ReadScope {
            hidden_partitions: self.hidden.proposers.iter().cloned().collect(),
            visible_evidence: EvidenceVisibility::Full,
        }
    }

    /// Read scope for optimizer contexts.
    #[must_use]
    pub fn optimizer_read_scope(&self) -> ReadScope {
        ReadScope {
            hidden_partitions: self.hidden.optimizers.iter().cloned().collect(),
            visible_evidence: EvidenceVisibility::Full,
        }
    }

    /// Read scope for evaluator contexts.
    #[must_use]
    pub fn evaluator_read_scope(&self) -> ReadScope {
        ReadScope::default()
    }

    /// Read scope for renderer contexts.
    #[must_use]
    pub fn renderer_read_scope(&self) -> ReadScope {
        ReadScope::default()
    }

    /// Read scope for callback graph views.
    #[must_use]
    pub fn callback_read_scope(&self) -> ReadScope {
        ReadScope {
            hidden_partitions: self.hidden.callbacks.iter().cloned().collect(),
            visible_evidence: EvidenceVisibility::Full,
        }
    }

    /// Refuse an evaluation request that references partitions hidden from the actor.
    ///
    /// Expression-level checks catch `Partition` / `All` / composed sets. Explicit
    /// `Cases` lists are checked after resolution via
    /// [`Self::check_resolved_cases`]. `EvaluationPurpose::FinalTest` is allowed
    /// so product final-report evaluations can read an otherwise optimizer-hidden
    /// `TEST` partition after search completes.
    pub fn check_evaluation_request(
        &self,
        actor: &Actor,
        request: &EvaluationRequest,
    ) -> Result<(), TrustViolation> {
        if request_purpose(request) == EvaluationPurpose::FinalTest {
            return Ok(());
        }
        let Some(hidden) = self.hidden_for(actor) else {
            return Ok(());
        };
        let set = request_set(request);
        let partitions = hidden_partitions_referenced(set, hidden);
        if partitions.is_empty() {
            Ok(())
        } else {
            Err(TrustViolation::HiddenEvaluationPartitions {
                actor: actor.clone(),
                partitions,
            })
        }
    }

    /// Refuse resolved case IDs that belong to partitions hidden from the actor.
    ///
    /// This closes the `EvaluationSet::Cases` bypass: trust is enforced against
    /// partition membership after resolution, not only against the unresolved
    /// expression shape.
    pub fn check_resolved_cases<C>(
        &self,
        actor: &Actor,
        purpose: EvaluationPurpose,
        case_ids: &[CaseId],
        case_set: &CaseSet<C>,
    ) -> Result<(), TrustViolation> {
        if purpose == EvaluationPurpose::FinalTest {
            return Ok(());
        }
        let Some(hidden) = self.hidden_for(actor) else {
            return Ok(());
        };
        let partitions = case_set.hidden_partitions_for_cases(case_ids, hidden);
        if partitions.is_empty() {
            Ok(())
        } else {
            Err(TrustViolation::HiddenEvaluationPartitions {
                actor: actor.clone(),
                partitions,
            })
        }
    }

    fn hidden_for(&self, actor: &Actor) -> Option<&[PartitionId]> {
        match actor {
            Actor::Optimizer => Some(&self.hidden.optimizers),
            Actor::Proposer(_) => Some(&self.hidden.proposers),
            Actor::Callback => Some(&self.hidden.callbacks),
            Actor::Evaluator(_) | Actor::Renderer(_) => None,
        }
    }
}

fn request_set(request: &EvaluationRequest) -> &EvaluationSet {
    match request {
        EvaluationRequest::Independent { set, .. }
        | EvaluationRequest::Pairwise { set, .. }
        | EvaluationRequest::Listwise { set, .. } => set,
    }
}

fn request_purpose(request: &EvaluationRequest) -> EvaluationPurpose {
    match request {
        EvaluationRequest::Independent { purpose, .. }
        | EvaluationRequest::Pairwise { purpose, .. }
        | EvaluationRequest::Listwise { purpose, .. } => purpose.clone(),
    }
}

fn hidden_partitions_referenced(set: &EvaluationSet, hidden: &[PartitionId]) -> Vec<PartitionId> {
    let mut partitions = BTreeSet::new();
    collect_hidden_partitions(set, hidden, &mut partitions);
    partitions.into_iter().collect()
}

fn collect_hidden_partitions(
    set: &EvaluationSet,
    hidden: &[PartitionId],
    partitions: &mut BTreeSet<PartitionId>,
) {
    match set {
        EvaluationSet::Partition(partition) => {
            if hidden.contains(partition) {
                partitions.insert(partition.clone());
            }
        }
        EvaluationSet::All => partitions.extend(hidden.iter().cloned()),
        EvaluationSet::Sample { of, .. } | EvaluationSet::Stratified { of, .. } => {
            collect_hidden_partitions(of, hidden, partitions);
        }
        EvaluationSet::Union(sets) | EvaluationSet::Intersect(sets) => {
            for set in sets {
                collect_hidden_partitions(set, hidden, partitions);
            }
        }
        EvaluationSet::Difference(left, right) => {
            collect_hidden_partitions(left, hidden, partitions);
            collect_hidden_partitions(right, hidden, partitions);
        }
        EvaluationSet::Unscoped
        | EvaluationSet::Cases(_)
        | EvaluationSet::Tagged(_)
        | EvaluationSet::Recent { .. } => {}
    }
}

/// Actor whose read or evaluation authority is being checked.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Actor {
    /// The optimizer loop itself.
    Optimizer,
    /// A proposer stage.
    Proposer(ProposerId),
    /// An evaluator stage.
    Evaluator(EvaluatorId),
    /// A renderer stage.
    Renderer(RendererId),
    /// A callback observing run events.
    Callback,
}

/// Trust-policy refusal for an actor operation.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TrustViolation {
    /// The actor requested evaluation over one or more hidden partitions.
    #[error("{actor:?} cannot evaluate hidden partitions: {partitions:?}")]
    HiddenEvaluationPartitions {
        /// Actor that made the request.
        actor: Actor,
        /// Hidden partitions referenced by the request.
        partitions: Vec<PartitionId>,
    },
}

/// Reserved probe-evaluation handle.
///
/// The type is public because the topology reserves the capability, but it is
/// intentionally not constructible until the probe evaluation contract lands.
#[derive(Debug)]
#[non_exhaustive]
pub struct EvalHandle {
    _private: (),
}

/// Reserved graph recorder for probe evaluations.
///
/// This becomes constructible only when probe-originated candidates and
/// assessments have their durable graph contract.
#[derive(Debug)]
#[non_exhaustive]
pub struct ProbeRecorder {
    _private: (),
}
