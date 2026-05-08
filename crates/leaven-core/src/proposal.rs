//! Proposals — what an optimizer proposes to do next.
//!
//! A [`Proposal`] is a typed record of one intended action. Its
//! [`ProposalEffect`] is either `Create` (author a brand-new artifact)
//! or `Change` (apply a typed change to one target candidate). Both
//! shapes are first-class so optimizers that synthesize artifacts from
//! scratch don't have to fake a "nothing" parent.
//!
//! Causal and informational lineage are kept separate.
//! [`CausalInputs`] names the candidates whose content *produced* the
//! new artifact's identity; [`InfoRef`] entries record what the proposer
//! *read while deciding* but did not embed into the result. Lineage
//! queries follow `causal`; "what did the proposer look at?" follows
//! `informed_by`.

use crate::artifact::Artifact;
use crate::problem::OptimizationProblem;
use leaven_kernel::{AssessmentId, CandidateId, MetadataBag, ProposalId};

/// A single proposal record.
///
/// Carries four orthogonal pieces of information:
///
/// - [`effect`](Proposal::effect) — what this proposal *does* (Create
///   a fresh artifact, or apply a Change to an existing candidate).
/// - [`provenance`](Proposal::provenance) — what this proposal was
///   *derived from* (causal lineage) and what the proposer *read*
///   while deciding (informational lineage). The two are distinct.
/// - [`annotations`](Proposal::annotations) — typed semantic payload
///   defined by the run's [`OptimizationProblem`]. Reflection notes,
///   behavioral claims, and surrogate predictions live here.
/// - [`metadata`](Proposal::metadata) — operational breadcrumbs only.
///   Optimizer logic must not branch on metadata.
///
/// Most callers should not construct this struct directly. Use
/// [`Proposal::create`], [`Proposal::mutate`], [`Proposal::merge`], or
/// [`Proposal::aggregate`] to obtain a [`ProposalBuilder`] and chain
/// `.informed_by`, `.annotations`, `.metadata`, `.build`.
pub struct Proposal<P: OptimizationProblem> {
    /// What this proposal does to the run graph if applied successfully.
    pub effect: ProposalEffect<P>,
    /// Causal and informational lineage.
    pub provenance: ProposalProvenance,
    /// Typed semantic payload (reflection notes, claims, predictions).
    pub annotations: P::ProposalAnnotations,
    /// Operational metadata. Non-semantic.
    pub metadata: MetadataBag,
}

impl<P: OptimizationProblem> Clone for Proposal<P> {
    fn clone(&self) -> Self {
        Self {
            effect: self.effect.clone(),
            provenance: self.provenance.clone(),
            annotations: self.annotations.clone(),
            metadata: self.metadata.clone(),
        }
    }
}

impl<P: OptimizationProblem> Proposal<P> {
    /// Builds a proposal that authors a brand-new artifact with no causal
    /// predecessor.
    ///
    /// Used by Meta-Harness-style optimizers, fresh program synthesis,
    /// and any case where the proposer constructs the artifact directly
    /// rather than transforming an existing candidate. Sets
    /// [`ProposalEffect::Create`] and [`CausalInputs::None`].
    ///
    /// Lineage for `Create` proposals is *bibliographic* (via
    /// `informed_by`), not causal — the proposer may have read prior
    /// candidates but their content did not contribute to the new
    /// artifact's identity.
    #[must_use]
    pub fn create(artifact: P::Artifact) -> ProposalBuilder<P> {
        ProposalBuilder {
            effect: ProposalEffect::Create { artifact },
            provenance: ProposalProvenance::new(CausalInputs::None),
            annotations: None,
            metadata: MetadataBag::new(),
        }
    }

    /// Builds a proposal that aggregates `parents` into a fresh artifact.
    ///
    /// Used for ensemble reducers and N→1 syntheses where the new
    /// artifact's content is a function of multiple parents but cannot
    /// be expressed as a typed change to any one of them. Sets
    /// [`ProposalEffect::Create`] and [`CausalInputs::NAry`].
    #[must_use]
    pub fn aggregate(parents: Vec<CandidateId>, artifact: P::Artifact) -> ProposalBuilder<P> {
        ProposalBuilder {
            effect: ProposalEffect::Create { artifact },
            provenance: ProposalProvenance::new(CausalInputs::NAry(parents)),
            annotations: None,
            metadata: MetadataBag::new(),
        }
    }

    /// Builds a standard mutation proposal: apply `change` to `target`.
    ///
    /// Sets [`ProposalEffect::Change`] with `target == target` and
    /// [`CausalInputs::Single(target)`](CausalInputs::Single).
    #[must_use]
    pub fn mutate(
        target: CandidateId,
        change: <P::Artifact as Artifact>::Change,
    ) -> ProposalBuilder<P> {
        ProposalBuilder {
            effect: ProposalEffect::Change { target, change },
            provenance: ProposalProvenance::new(CausalInputs::Single(target)),
            annotations: None,
            metadata: MetadataBag::new(),
        }
    }

    /// Builds a merge proposal that canonicalizes onto `left` while
    /// recording both `left` and `right` as causal parents.
    ///
    /// `Artifact::apply_change` only sees one artifact, so merges are
    /// expressed as: pick a canonical apply target (`left`), construct
    /// a `change` that already embeds whatever content is being
    /// imported from `right`, and record [`CausalInputs::Pair`] so the
    /// graph's lineage queries see both contributors.
    ///
    /// The merge proposer is responsible for reading `right` (via the
    /// run graph) when it constructs the change. The framework does
    /// not magically combine artifacts.
    #[must_use]
    pub fn merge(
        left: CandidateId,
        right: CandidateId,
        change: <P::Artifact as Artifact>::Change,
    ) -> ProposalBuilder<P> {
        ProposalBuilder {
            effect: ProposalEffect::Change {
                target: left,
                change,
            },
            provenance: ProposalProvenance::new(CausalInputs::Pair(left, right)),
            annotations: None,
            metadata: MetadataBag::new(),
        }
    }
}

/// Fluent builder for [`Proposal`].
///
/// Returned by [`Proposal::create`], [`Proposal::mutate`],
/// [`Proposal::merge`], and [`Proposal::aggregate`]. Each constructor
/// pre-fills `effect` and `provenance.causal`; the builder lets the
/// caller layer on bibliographic [`InfoRef`]s, typed annotations, and
/// operational metadata before [`build`](ProposalBuilder::build).
pub struct ProposalBuilder<P: OptimizationProblem> {
    effect: ProposalEffect<P>,
    provenance: ProposalProvenance,
    annotations: Option<P::ProposalAnnotations>,
    metadata: MetadataBag,
}

impl<P: OptimizationProblem> Clone for ProposalBuilder<P> {
    fn clone(&self) -> Self {
        Self {
            effect: self.effect.clone(),
            provenance: self.provenance.clone(),
            annotations: self.annotations.clone(),
            metadata: self.metadata.clone(),
        }
    }
}

impl<P: OptimizationProblem> ProposalBuilder<P> {
    /// Records bibliographic influences — things the proposer *read*
    /// while deciding, but whose content did not contribute to the
    /// new artifact's identity.
    ///
    /// Distinct from causal lineage. An agentic proposer that reads
    /// prior candidates' evidence to decide its next move records them
    /// here; a mutation proposer records its parent in `causal`
    /// (already pre-filled by the constructor) and may additionally
    /// list assessments or external references it consulted.
    ///
    /// Calls accumulate; multiple invocations append rather than
    /// replace.
    #[must_use]
    pub fn informed_by(mut self, refs: impl IntoIterator<Item = InfoRef>) -> Self {
        self.provenance.informed_by.extend(refs);
        self
    }

    /// Attaches the typed annotations for this proposal.
    ///
    /// If unset, [`build`](ProposalBuilder::build) substitutes
    /// [`Default::default`] — convenient when annotations carry no
    /// useful payload for this proposal kind.
    #[must_use]
    pub fn annotations(mut self, annotations: P::ProposalAnnotations) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Replaces the operational metadata bag.
    ///
    /// The default is empty. Use this when the proposer wants to record
    /// non-semantic breadcrumbs (worker hostname, prompt blob refs,
    /// token usage breakdowns) alongside the proposal.
    #[must_use]
    pub fn metadata(mut self, metadata: MetadataBag) -> Self {
        self.metadata = metadata;
        self
    }

    /// Finalizes the builder into a [`Proposal`].
    ///
    /// Requires `P::ProposalAnnotations: Default` so that omitted
    /// annotations resolve to the default value. Problems whose
    /// annotations have no sensible default should set them
    /// explicitly via [`annotations`](ProposalBuilder::annotations)
    /// — or define the annotations type with a `Default` impl.
    #[must_use]
    pub fn build(self) -> Proposal<P>
    where
        P::ProposalAnnotations: Default,
    {
        Proposal {
            effect: self.effect,
            provenance: self.provenance,
            annotations: self.annotations.unwrap_or_default(),
            metadata: self.metadata,
        }
    }
}

/// What this proposal does to the graph if applied successfully.
///
/// Two effects, both first-class:
///
/// - `Create` — author a brand-new artifact. The framework records the
///   artifact directly as a new candidate; no apply step runs. Used
///   when the proposer synthesizes content rather than transforming an
///   existing candidate (fresh program synthesis, ensemble aggregates,
///   agentic harness search).
/// - `Change` — apply a typed change to one existing candidate. Even
///   merge-style proposals take this shape: pick a canonical apply
///   target and embed any cross-parent content into the change itself.
///   `Artifact::apply_change` only ever sees one artifact, by design.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "P::Artifact: serde::Serialize, <P::Artifact as Artifact>::Change: serde::Serialize",
    deserialize = "P::Artifact: serde::Deserialize<'de>, <P::Artifact as Artifact>::Change: serde::Deserialize<'de>"
))]
pub enum ProposalEffect<P: OptimizationProblem> {
    /// Author a brand-new artifact with no apply target.
    Create {
        /// The new artifact value.
        artifact: P::Artifact,
    },

    /// Apply a typed change to an existing candidate.
    Change {
        /// Candidate the change is applied to.
        target: CandidateId,
        /// Typed change carried by the proposal.
        change: <P::Artifact as Artifact>::Change,
    },
}

impl<P: OptimizationProblem> Clone for ProposalEffect<P> {
    fn clone(&self) -> Self {
        match self {
            Self::Create { artifact } => Self::Create {
                artifact: artifact.clone(),
            },
            Self::Change { target, change } => Self::Change {
                target: *target,
                change: change.clone(),
            },
        }
    }
}

/// Causal and informational lineage for a proposal.
///
/// Two distinct typed facts:
///
/// - `causal` — content lineage. The candidates whose state directly
///   contributed to the proposal's content. These determine the new
///   candidate's identity.
/// - `informed_by` — bibliographic lineage. Candidates, prior proposals,
///   assessments, or external references the proposer *read while
///   deciding* but whose content did not participate in the result.
///
/// Lineage queries on the run graph follow `causal`; "what did this
/// proposer look at?" follows `informed_by`. The split keeps cache
/// correctness honest — `informed_by` candidates can change without
/// invalidating downstream cache entries — and gives lineage queries
/// well-defined semantics.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ProposalProvenance {
    /// Candidates whose content contributed to the proposal.
    pub causal: CausalInputs,
    /// References the proposer read while deciding.
    pub informed_by: Vec<InfoRef>,
}

impl ProposalProvenance {
    /// Constructs a provenance with the given causal inputs and an
    /// empty `informed_by` list.
    #[must_use]
    pub fn new(causal: CausalInputs) -> Self {
        Self {
            causal,
            informed_by: Vec::new(),
        }
    }

    /// Appends bibliographic references. Repeated calls accumulate.
    #[must_use]
    pub fn informed_by(mut self, refs: impl IntoIterator<Item = InfoRef>) -> Self {
        self.informed_by.extend(refs);
        self
    }
}

/// Content-lineage parents for a proposal.
///
/// The four shapes cover every well-formed lineage in the framework's
/// proposal-validation rules:
///
/// - `None` — fresh authoring (paired with [`ProposalEffect::Create`]).
/// - `Single` — standard mutation (paired with `Change` whose target
///   matches the parent).
/// - `Pair` — merge / crossover (paired with `Change` whose target is
///   one of the two parents).
/// - `NAry` — N→1 aggregate (typically paired with `Create`).
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CausalInputs {
    /// No causal predecessor.
    None,
    /// One causal parent (mutation).
    Single(CandidateId),
    /// Two causal parents (merge / crossover).
    Pair(CandidateId, CandidateId),
    /// N causal parents (aggregate / ensemble reduction).
    NAry(Vec<CandidateId>),
}

impl CausalInputs {
    /// Returns true when `target` appears in the causal inputs.
    #[must_use]
    pub fn contains_candidate(&self, target: CandidateId) -> bool {
        match self {
            Self::None => false,
            Self::Single(c) => *c == target,
            Self::Pair(a, b) => *a == target || *b == target,
            Self::NAry(cs) => cs.contains(&target),
        }
    }

    /// Iterates over the causal parents in declaration order.
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

/// One bibliographic reference the proposer consulted.
///
/// References are typed so graph queries know what they're pointing at
/// and so storage and visibility policies can treat candidates,
/// proposals, assessments, and external refs differently.
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum InfoRef {
    /// Reference to another candidate in the run graph.
    Candidate(CandidateId),
    /// Reference to a prior proposal (typically a sibling or ancestor).
    Proposal(ProposalId),
    /// Reference to an assessment the proposer read.
    Assessment(AssessmentId),
    /// Reference to something outside the run graph.
    External(ExternalRef),
}

/// Reference to something outside the run graph.
///
/// The cold core does not interpret these; downstream tooling
/// (telemetry consumers, lineage viewers, reproducibility harnesses)
/// knows what `kind`/`id` pairs mean for its purposes. Examples:
/// `kind = "paper"`, `id = "arxiv:2403.12345"`; `kind = "checkpoint"`,
/// `id = "blake3:abcd..."`.
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExternalRef {
    /// Reference category.
    pub kind: String,
    /// Reference identifier within that category.
    pub id: String,
}

/// Sibling proposals produced by a single proposer call.
///
/// Sibling proposals share one proposer context (the same `propose`
/// invocation produced them) but may have entirely different causal
/// parents — a reflective proposer may emit alternatives derived from
/// different candidates within one batch. The batch carries
/// [`semantics`] (how the optimizer should treat the group) and
/// [`metadata`]; per-proposal causal lineage lives on each proposal.
///
/// [`semantics`]: ProposalBatch::semantics
/// [`metadata`]: ProposalBatch::metadata
pub struct ProposalBatch<P: OptimizationProblem> {
    /// Proposals in this batch, in proposer-declared order.
    pub proposals: Vec<Proposal<P>>,
    /// How the optimizer should interpret the group.
    pub semantics: ProposalBatchSemantics,
    /// Operational metadata attached to the batch as a whole.
    pub metadata: MetadataBag,
}

impl<P: OptimizationProblem> Clone for ProposalBatch<P> {
    fn clone(&self) -> Self {
        Self {
            proposals: self.proposals.clone(),
            semantics: self.semantics,
            metadata: self.metadata.clone(),
        }
    }
}

/// How an optimizer should interpret a sibling group of proposals.
///
/// Ordered batches (siblings where later proposals depend on earlier
/// ones) are not expressible — express ordered dependencies by issuing
/// multiple batches across optimizer steps, applying intermediate
/// results before proposing the next batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ProposalBatchSemantics {
    /// Independent siblings; any subset (or none) may be applied. Each
    /// surviving alternative is evaluated separately — there is no
    /// implicit deduplication.
    Alternatives,

    /// A pool the optimizer may sample from. The proposer expects
    /// only some proposals to be applied or evaluated; the rest are
    /// hints/options.
    CandidatePool,
}

/// Discriminant tag for [`ProposalEffect`] used by graph events and
/// reports that don't need to carry the full effect payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ProposalEffectKind {
    /// Corresponds to [`ProposalEffect::Create`].
    Create,
    /// Corresponds to [`ProposalEffect::Change`].
    Change,
}
