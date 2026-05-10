//! Common imports for most Leaven users.

pub use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    ContentAddressed, EvaluationRequest, EvaluationSet, Evidence, OptimizationProblem, PairOrder,
    PartitionId, Proposal, ProposalBatch, ProposalEffect,
};
pub use leaven_engine::{
    Arity, CachePolicy, Engine, Evaluator, MaterializationReport, MaterializeContext,
    MaterializeError, Materializer, Optimizer, Population, PreferenceRelation, Proposer, Renderer,
    RunContext, RunGraphView, Stopper, TrustPolicy,
};
pub use leaven_kernel::{
    Budget, CandidateId, ContentId, Cost, CostUnit, ErrorRecord, Fingerprint, FiniteF64,
    MetadataBag,
};
pub use leaven_run::{
    IntoOptimizeStore, OptimizationReport, OptimizeError, OptimizeResult, OptimizeStore, RunOutput,
    Score, ScoreContext, optimize,
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

#[cfg(feature = "agentic")]
pub use leaven_agentic::prelude::*;

#[cfg(all(feature = "skill", not(feature = "std")))]
pub use leaven_artifact_skill::*;

#[cfg(feature = "agentic-skill")]
pub use leaven_agentic_skill::*;
