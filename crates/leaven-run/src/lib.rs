//! Public product builder for ordinary Leaven optimization runs.

mod builder;
pub(crate) mod compatibility;
mod error;
mod evaluator;
mod evidence;
mod result;
pub(crate) mod run_report;
pub(crate) mod run_store;
mod store;

pub use builder::{OptimizeBuilder, RunProblem, optimize};
pub use compatibility::{
    ResumeCompatibilityError, RunCompatibilityManifest, RuntimeFingerprint, RuntimeKind,
    ScoringEvaluatorIdentity,
};
pub use error::OptimizeError;
pub use evaluator::ScoringEvaluator;
pub use evidence::{
    IntoRunResult, RunCase, RunError, RunOutput, Score, ScoreCase, ScoreContext, ScoreError,
    ScoreMetadataView,
};
pub use leaven_engine::CachePolicy;
pub use result::{
    BestCandidate, EvaluationCacheBackend, EvaluationCacheBypassReason,
    EvaluationCacheBypassSummary, EvaluationCacheSummary, OptimizationStopReason, Optimized,
    RunCacheSummary, RunCompatibilitySummary, RunEventSummary, RunNotResumableReason,
    RunReportPaths, RunResumability, RunStorage, StandardRunSummary,
};
pub use run_store::default_local_run_dir;
pub use store::{IntoOptimizeStore, OptimizeStore};

pub mod prelude {
    //! Common public-run imports.

    pub use crate::{
        BestCandidate, CachePolicy, EvaluationCacheBackend, EvaluationCacheBypassReason,
        EvaluationCacheBypassSummary, EvaluationCacheSummary, IntoOptimizeStore,
        OptimizationStopReason, OptimizeBuilder, OptimizeError, OptimizeStore, Optimized,
        RunCacheSummary, RunCase, RunCompatibilitySummary, RunError, RunEventSummary,
        RunNotResumableReason, RunOutput, RunProblem, RunReportPaths, RunResumability, RunStorage,
        Score, ScoreCase, ScoreContext, ScoreError, ScoreMetadataView, StandardRunSummary,
        default_local_run_dir, optimize,
    };
}
