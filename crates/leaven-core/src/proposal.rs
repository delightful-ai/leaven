//! Proposals — what an optimizer proposes to do next.
//!
//! v0.2.1 corrected the v0.2 awkwardness around brand-new authored
//! artifacts: a `Proposal` carries a typed [`ProposalEffect`] which is
//! either `Create` (fresh artifact, no change applied) or `Change`
//! (apply a typed change to one target candidate). The earlier
//! `Parents::None + change` shape lied about what was happening; this
//! shape doesn't.
//!
//! Causal lineage and informational provenance are kept separate.
//! [`CausalInputs`] determines which existing candidates *produced*
//! this proposal's content; [`InfoRef`] entries record what the
//! proposer *read* without becoming lineage.

use serde::{Deserialize, Serialize};

use crate::artifact::Artifact;
use crate::ids::{AssessmentId, CandidateId, ProposalBatchId, ProposalId};
use crate::metadata::MetadataBag;
use crate::problem::OptimizationProblem;

/// A single proposal record.
#[derive(Clone, Debug)]
pub struct Proposal<P: OptimizationProblem> {
    pub effect: ProposalEffect<P>,
    pub provenance: ProposalProvenance,
    pub annotations: P::ProposalAnnotations,
    pub metadata: MetadataBag,
}

/// What this proposal does to the graph if applied successfully.
#[derive(Clone, Debug)]
pub enum ProposalEffect<P: OptimizationProblem> {
    /// Brand-new authored artifact. Used by Meta-Harness style
    /// optimizers, fresh program synthesis, and cases where the
    /// optimizer does not mutate from a concrete parent.
    Create { artifact: P::Artifact },

    /// Apply a typed change to one existing candidate. The
    /// `<P::Artifact as Artifact>::Change` is canonical: even merge
    /// proposals canonicalise to a single target plus a change that
    /// embeds whatever cross-parent content is needed.
    Change {
        target: CandidateId,
        change: <P::Artifact as Artifact>::Change,
    },
}

/// Provenance of a proposal.
///
/// Two distinct typed facts:
/// - `causal` — content lineage. The candidates whose state directly
///   contributed to the proposal's content.
/// - `informed_by` — bibliographic / informational lineage. Things the
///   proposer *read* (other candidates, prior proposals, assessments,
///   external references) without their content participating in the
///   change.
///
/// Lineage queries on the run graph use only `causal`; "what did this
/// proposer look at?" uses `informed_by`. Conflating the two was the
/// python-gepa stringly-typed metadata-parsing failure mode.
#[derive(Clone, Debug)]
pub struct ProposalProvenance {
    pub causal: CausalInputs,
    pub informed_by: Vec<InfoRef>,
}

impl ProposalProvenance {
    #[must_use]
    pub fn new(causal: CausalInputs) -> Self {
        Self {
            causal,
            informed_by: Vec::new(),
        }
    }

    #[must_use]
    pub fn informed_by(mut self, refs: impl IntoIterator<Item = InfoRef>) -> Self {
        self.informed_by.extend(refs);
        self
    }
}

/// Content-lineage parents for a proposal.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CausalInputs {
    /// Brand-new authored content with no causal predecessor.
    None,
    Single(CandidateId),
    Pair(CandidateId, CandidateId),
    NAry(Vec<CandidateId>),
}

impl CausalInputs {
    /// Does the given candidate appear as a causal input?
    #[must_use]
    pub fn contains_candidate(&self, target: CandidateId) -> bool {
        match self {
            Self::None => false,
            Self::Single(c) => *c == target,
            Self::Pair(a, b) => *a == target || *b == target,
            Self::NAry(cs) => cs.contains(&target),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = CandidateId> + '_ {
        let v: Box<dyn Iterator<Item = CandidateId>> = match self {
            Self::None => Box::new(std::iter::empty()),
            Self::Single(c) => Box::new(std::iter::once(*c)),
            Self::Pair(a, b) => Box::new([*a, *b].into_iter()),
            Self::NAry(cs) => Box::new(cs.clone().into_iter()),
        };
        v
    }
}

/// One thing the proposer read while producing the proposal. Not
/// lineage; it never affects which candidates are considered parents.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum InfoRef {
    Candidate(CandidateId),
    Proposal(ProposalId),
    ProposalBatch(ProposalBatchId),
    Assessment(AssessmentId),
    External(ExternalRef),
}

/// Reference to something outside the run graph (a paper, a model
/// checkpoint, a prior run's artefact). The cold core does not
/// interpret it; downstream tooling does.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct ExternalRef {
    pub kind: String,
    pub id: String,
}

/// Sibling proposals from a single proposer call.
#[derive(Clone, Debug)]
pub struct ProposalBatch<P: OptimizationProblem> {
    pub proposals: Vec<Proposal<P>>,
    pub semantics: ProposalBatchSemantics,
    pub metadata: MetadataBag,
}

/// What a sibling group of proposals means.
///
/// `Ordered` was considered and rejected: the optimizer rhythm already
/// covers ordered dependencies via multiple optimizer steps. Re-add if
/// a real prototype proves otherwise.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum ProposalBatchSemantics {
    /// Independent siblings produced from one proposer context. Any
    /// subset (or none) may be applied.
    Alternatives,

    /// A pool the optimizer may sample from; the proposer expects only
    /// some to be applied or evaluated.
    CandidatePool,
}
