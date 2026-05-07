//! Cold optimizer algebra.
//!
//! `leaven-core` is the smallest possible vocabulary for talking about an
//! optimization run: an [`Artifact`] being optimized, [`Proposal`]s that
//! transform it or author new ones, [`Evaluation`] requests that ask "how
//! good is this?", and [`Evidence`] that answers without committing to a
//! specific shape. The cold core knows nothing about *running* a run — no
//! engine, no graph storage, no LLM SDK, no GEPA. It just defines the
//! states a run can be in.
//!
//! [`Evaluation`]: crate::evaluation
//!
//! # Why split this from the engine
//!
//! Optimizer authors should be able to read the algebra without dragging in
//! the run loop, the workspace substrate, or any specific persistence
//! choice. Storage and event sinks should be able to talk about
//! [`Proposal`]s and [`Assessment`]s without depending on the engine
//! crate's mutation surface. Keeping the algebra in its own crate forces
//! that separation to be real, not aspirational.
//!
//! # Structure
//!
//! - [`artifact`] — [`Artifact`] (the thing being optimized) and the
//!   stronger [`ContentAddressed`] capability.
//! - [`proposal`] — [`Proposal`], [`ProposalEffect`] (Create vs Change),
//!   [`ProposalProvenance`] (causal vs informational lineage),
//!   [`ProposalBatch`].
//! - [`evaluation`] — [`EvaluationRequest`] (Independent / Pairwise /
//!   Listwise), [`EvaluationSet`] expressions and their resolved forms,
//!   [`Assessment`] outputs, granularity and purpose tags.
//! - [`evidence`] — [`Evidence`] marker. Optional capability traits live
//!   in `leaven-evidence`, not here.
//! - [`preference`] — [`Preference`] result. The `PreferenceRelation` trait
//!   lives in `leaven-engine`.
//! - [`problem`] — [`OptimizationProblem`], the trait that bundles a run's
//!   associated types.
//!
//! # Negative space
//!
//! These intentionally do *not* live here:
//!
//! - **Artifact parts.** Artifacts are opaque except for change-application.
//!   Part structure is a chosen lens, not an intrinsic property —
//!   that is what `leaven-surface::EditSurface` is for.
//! - **Workspaces / renderers.** Materializing an artifact into a
//!   filesystem layout is a rendering concern. The cold-core `Artifact`
//!   trait stays free of workspace dependencies.
//! - **Run graph / engine.** Mutation goes through `leaven-engine`'s
//!   `RunContext`; the algebra here is what gets recorded, not the recorder.
//! - **Populations / preferences with state.** Stateless `Preference` is a
//!   simple result; populations and fitted preference models live in
//!   `leaven-population` because their state evolves with the run.

pub mod artifact;
pub mod evaluation;
pub mod evidence;
pub mod preference;
pub mod prelude;
pub mod problem;
pub mod proposal;

pub use artifact::{Artifact, ArtifactIdentity, ContentAddressed};
pub use evaluation::{
    Assessment, AssessmentGranularity, AssessmentTarget, CaseSetVersion, EvaluationPurpose,
    EvaluationRequest, EvaluationSet, PairOrder, PartitionId, ResolvedEvaluationRequest,
    ResolvedEvaluationSet, ResolvedRequestKind, Tag, Window,
};
pub use evidence::Evidence;
pub use preference::Preference;
pub use problem::OptimizationProblem;
pub use proposal::{
    CausalInputs, ExternalRef, InfoRef, Proposal, ProposalBatch, ProposalBatchSemantics,
    ProposalBuilder, ProposalEffect, ProposalEffectKind, ProposalProvenance,
};
