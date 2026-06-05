//! Public product builder for ordinary Leaven optimization runs.

mod builder;
pub(crate) mod compatibility;
mod error;
mod evaluator;
mod evidence;
mod inspection;
mod public_seam;
mod result;
pub(crate) mod run_report;
pub(crate) mod run_store;
mod store;
#[cfg(test)]
pub(crate) mod test_support;

pub use builder::{OptimizeBuilder, RunProblem, optimize};
pub use compatibility::{
    ResumeCompatibilityError, RunCompatibilityManifest, RuntimeFingerprint, RuntimeKind,
    ScoringEvaluatorIdentity,
};
pub use error::OptimizeError;
pub use evaluator::{JudgeCandidateOutput, JudgeScoreContext, JudgingEvaluator, ScoringEvaluator};
pub use evidence::{
    IntoRunResult, ReportableOutput, RunCase, RunError, RunOutput, Score, ScoreCase, ScoreContext,
    ScoreError, artifact_identity_output,
};
pub use inspection::{
    RUN_BLOB_EXPORT_SCHEMA, RUN_EVIDENCE_EXPORT_SCHEMA, RUN_INSPECTION_EXPORT_SCHEMA,
    RunInspectionExportError, RustRunBlobExport, RustRunEvidenceExport, RustRunInspectionExport,
    export_local_run_blob, export_local_run_evidence, export_local_run_inspection,
};
pub use leaven_engine::CachePolicy;
pub use public_seam::{
    PublicAssessmentWriteReceiptContext, PublicAssessmentWriteReceiptProjectionError,
    PublicEvaluationJobContext, PublicEvaluationJobProjectionError, PublicFailedCallKind,
    PublicFailedCallReceiptContext, PublicFailedCallReceiptProjectionError,
    PublicProposalWriteReceiptContext, PublicProposalWriteReceiptProjectionError,
};
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
        ReportableOutput, RunCacheSummary, RunCase, RunCompatibilitySummary, RunError,
        RunEventSummary, RunNotResumableReason, RunOutput, RunProblem, RunReportPaths,
        RunResumability, RunStorage, Score, ScoreCase, ScoreContext, ScoreError,
        StandardRunSummary, default_local_run_dir, optimize,
    };
}
