//! Common cold-core imports.

pub use crate::artifact::{Artifact, ArtifactIdentity, ContentAddressed};
pub use crate::evaluation::{
    Assessment, AssessmentGranularity, AssessmentTarget, EvaluationRequest, EvaluationSet,
    PairOrder, PartitionId, ResolvedEvaluationRequest, ResolvedEvaluationSet,
};
pub use crate::evidence::Evidence;
pub use crate::preference::Preference;
pub use crate::problem::OptimizationProblem;
pub use crate::proposal::{
    CausalInputs, ExternalRef, InfoRef, Proposal, ProposalBatch, ProposalBatchSemantics,
    ProposalBuilder, ProposalEffect, ProposalProvenance,
};
