//! Leaven: optimize anything in Rust.
//!
//! This is the umbrella crate. It is an import experience, not an
//! implementation crate.

pub mod prelude;

pub use leaven_core as core;
pub use leaven_engine as engine;
pub use leaven_eval as eval;
pub use leaven_kernel as kernel;
pub use leaven_run as run;
pub use leaven_surface as surface;

pub use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget, CacheIdentity,
    CausalInputs, ContentAddressed, EvaluationRequest, EvaluationSet, Evidence, InfoRef,
    OptimizationProblem, PairOrder, PartitionId, Preference, Proposal, ProposalBatch,
    ProposalBatchSemantics, ProposalEffect, ProposalProvenance,
};
pub use leaven_engine::{
    Arity, CachePolicy, Engine, EngineBuilder, Evaluator, MaterializationReport,
    MaterializeContext, MaterializeError, Materializer, Optimizer, Population, PreferenceRelation,
    ProposalContext, Proposer, ReadScope, RenderContext, Renderer, RunContext, RunEvent,
    RunGraphView, RunResult, StepStatus, Stopper, TrustPolicy,
};
pub use leaven_kernel::{
    Amount, AmountError, Budget, BudgetSnapshot, CandidateId, ContentId, Cost, CostUnit,
    ErrorRecord, FiniteF64, FiniteF64Error, MetadataBag, ProposalId,
};
pub use leaven_run::{
    OptimizationReport, OptimizeBuilder, OptimizeResult, RunOutput, Score, ScoreContext, optimize,
};
pub use leaven_surface::{
    EditSurface, Part, PartAddress, PartSelection, SurfaceError, SurfaceFingerprint,
};

#[cfg(feature = "derive")]
pub use leaven_derive::{
    Artifact as DeriveArtifact, ContentAddressed as DeriveContentAddressed,
    EditSurface as DeriveEditSurface,
};

#[cfg(feature = "std")]
pub use leaven_std as stdlib;

#[cfg(feature = "gepa")]
pub use leaven_gepa::Gepa;

#[cfg(feature = "workspace")]
pub use leaven_workspace as workspace;

#[cfg(feature = "agentic")]
pub use leaven_agentic as agentic;

#[cfg(feature = "skill")]
pub use leaven_artifact_skill as skill;

#[cfg(feature = "agentic-skill")]
pub use leaven_agentic_skill as agentic_skill;
