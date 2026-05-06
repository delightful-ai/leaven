//! Common re-exports.
//!
//! `use leaven_core::prelude::*;` brings in the cold-core types most
//! optimizer authors and end-users will reach for.

pub use crate::artifact::{Artifact, ContentId, Decomposable};
pub use crate::candidate::{Candidate, CandidateOrigin};
pub use crate::cost::{Budget, BudgetSnapshot, Cost, Metered};
pub use crate::error::{ErrorKind, ErrorRecord, IntoErrorRecord};
pub use crate::evaluation::{
    Assessment, AssessmentGranularity, AssessmentTarget, EvaluationPurpose, EvaluationRequest,
    EvaluationSet, PairOrder, ResolvedEvaluationRequest,
};
pub use crate::evidence::{Evidence, EvidenceRef, EvidenceStore};
pub use crate::ids::{
    ApplyAttemptId, AssessmentId, CandidateId, EvaluationRequestId, EvaluatorId, IterationId,
    PartitionId, PopulationId, ProposalBatchId, ProposalId, ProposerId, RendererId, RunId, StageId,
};
pub use crate::metadata::{BlobRef, MetadataBag, MetadataKey, MetadataValue};
pub use crate::population::{PopulationEvent, PopulationRemovalReason};
pub use crate::preference::Preference;
pub use crate::problem::OptimizationProblem;
pub use crate::proposal::{
    CausalInputs, ExternalRef, InfoRef, Proposal, ProposalBatch, ProposalBatchSemantics,
    ProposalEffect, ProposalProvenance,
};
pub use crate::time::Timestamp;
