//! Ordinary Leaven user imports.
//!
//! Contains the short Layer 1 surface: `optimize(...)`, train/validation/test
//! and runner/score types, budget, ordinary LM vocabulary, artifact and edit
//! surface concepts. Engine-author, GEPA-customizer, cache-wrapper, and
//! derive-macro names are reachable through [`advanced`] or explicit module
//! paths (`leaven::engine::...`, `leaven::gepa::...`, `leaven::lm_cache::...`,
//! `leaven::stdlib::...`).
//!
//! Today this prelude intentionally exposes only behavior-bearing ordinary
//! contracts so a `use leaven::prelude::*` line does not teach engine
//! internals or production-looking placeholders.

pub use leaven_core::{Artifact, ContentAddressed};
pub use leaven_kernel::{
    Budget, CandidateId, ContentId, Cost, CostUnit, Fingerprint, FiniteF64, MetadataBag,
};
pub use leaven_lm::{
    Lm, LmContinuation, LmError, LmId, LmRequest, LmResponse, Message, Messages, ModelName,
    OutputMode, ProviderHints, ProviderName, ReasoningEffort, Role, SamplingOptions, TokenUsage,
};
pub use leaven_run::{
    IntoOptimizeStore, OptimizationReport, OptimizeError, OptimizeResult, OptimizeStore, RunOutput,
    Score, ScoreContext, optimize,
};
pub use leaven_surface::{EditSurface, Part, PartAddress, PartSelection};

#[cfg(feature = "gepa")]
pub use leaven_gepa::Gepa;

/// Engine-author, GEPA-customizer, and cache-wrapper imports.
///
/// This sub-prelude is reachable through `use leaven::prelude::advanced::*`.
/// It collects names that the Layer 1 product path should not require: raw
/// engine contexts, stage traits, evaluation-request/trust vocabulary,
/// proposal substrate, and feature-gated GEPA / standard-library /
/// LM-cache preludes.
pub mod advanced {
    pub use leaven_core::{
        ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget, EvaluationRequest,
        EvaluationSet, Evidence, OptimizationProblem, PairOrder, PartitionId, Proposal,
        ProposalBatch, ProposalEffect,
    };
    pub use leaven_engine::{
        Arity, CachePolicy, Engine, Evaluator, MaterializationReport, MaterializeContext,
        MaterializeError, Materializer, Optimizer, Population, PreferenceRelation, Proposer,
        Renderer, RunContext, RunGraphView, Stopper, TrustPolicy,
    };
    pub use leaven_kernel::ErrorRecord;

    #[cfg(feature = "derive")]
    pub use leaven_derive::{
        Artifact as DeriveArtifact, ContentAddressed as DeriveContentAddressed,
        EditSurface as DeriveEditSurface,
    };

    #[cfg(feature = "gepa")]
    pub use leaven_gepa::prelude::*;

    #[cfg(feature = "lm-cache")]
    pub use leaven_lm_cache::prelude::*;
}

#[cfg(feature = "std")]
pub use leaven_std::prelude::*;

#[cfg(feature = "agentic")]
pub use leaven_agentic::prelude::*;

#[cfg(all(feature = "skill", not(feature = "std")))]
pub use leaven_artifact_skill::*;

#[cfg(feature = "agentic-skill")]
pub use leaven_agentic_skill::*;
