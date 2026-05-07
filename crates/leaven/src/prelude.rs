//! Common imports for most Leaven users.

pub use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    ContentAddressed, EvaluationRequest, EvaluationSet, Evidence, OptimizationProblem, PairOrder,
    Proposal, ProposalBatch, ProposalEffect,
};
pub use leaven_engine::{
    Arity, CachePolicy, Engine, Evaluator, MaterializationReport, MaterializeError, Materializer,
    Optimizer, Population, PreferenceRelation, Proposer, Renderer, RunContext, RunGraphView,
    Stopper, TrustPolicy, optimize,
};
pub use leaven_kernel::{
    Budget, CandidateId, ContentId, Cost, CostUnit, ErrorRecord, FiniteF64, MetadataBag,
};
pub use leaven_surface::{EditSurface, Part, PartAddress, PartSelection};

#[cfg(feature = "derive")]
pub use leaven_derive::{
    Artifact as DeriveArtifact, ContentAddressed as DeriveContentAddressed,
    EditSurface as DeriveEditSurface,
};

#[cfg(feature = "gepa")]
pub use leaven_gepa::prelude::*;

#[cfg(feature = "std")]
pub use leaven_std::prelude::*;
