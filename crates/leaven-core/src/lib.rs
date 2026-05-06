//! Cold optimizer algebra.
//!
//! This crate defines the core states that can exist in a Leaven optimization
//! run. It does not define an engine, graph storage, surfaces, renderers,
//! workspaces, populations, or GEPA.

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
