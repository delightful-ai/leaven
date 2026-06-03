//! Public optimization result facade.

use std::path::PathBuf;

use leaven_engine::OptimizerReportPayload;
use leaven_eval::EvaluationReport;
use leaven_kernel::{BudgetSnapshot, CandidateId, CheckpointId, Cost, RunId};
use serde::Serialize;

/// Result returned by the product builder.
#[derive(Clone, Debug)]
pub struct Optimized<A> {
    /// Run id.
    pub run_id: RunId,
    /// Best candidate and artifact when the optimizer selected one.
    pub best: Option<BestCandidate<A>>,
    /// Seed artifact.
    pub seed_artifact: A,
    /// Reason the optimizer portion of the run stopped.
    pub stop: OptimizationStopReason,
    /// Budget snapshot after optimizer work and final report evaluations.
    pub budget: BudgetSnapshot,
    /// Ordinary run summary.
    pub summary: StandardRunSummary,
    /// Public event summaries emitted during the run.
    pub events: Vec<RunEventSummary>,
    pub(crate) optimizer_report: Option<OptimizerReportPayload>,
}

impl<A> Optimized<A> {
    /// Best candidate id.
    #[must_use]
    pub const fn best_id(&self) -> Option<CandidateId> {
        match &self.best {
            Some(best) => Some(best.id),
            None => None,
        }
    }

    /// Best artifact.
    #[must_use]
    pub const fn best(&self) -> Option<&A> {
        match &self.best {
            Some(best) => Some(&best.artifact),
            None => None,
        }
    }

    /// Ordinary run summary.
    #[must_use]
    pub const fn summary(&self) -> &StandardRunSummary {
        &self.summary
    }

    /// Split-aware evaluation report.
    #[must_use]
    pub const fn report(&self) -> &EvaluationReport {
        &self.summary.evaluation
    }

    /// Typed optimizer-specific report, when the configured optimizer produced
    /// one.
    #[must_use]
    pub fn optimizer_report<T>(&self) -> Option<&T>
    where
        T: std::fmt::Debug + Send + Sync + 'static,
    {
        self.optimizer_report
            .as_deref()
            .and_then(|report| report.as_any().downcast_ref::<T>())
    }
}

/// Best candidate returned by an optimization run.
#[derive(Clone, Debug)]
pub struct BestCandidate<A> {
    /// Graph-local candidate id.
    pub id: CandidateId,
    /// Candidate artifact cloned from graph truth.
    pub artifact: A,
}

/// Reason the optimizer portion of a product run stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize)]
pub enum OptimizationStopReason {
    /// Optimizer reported that it had finished.
    OptimizerDone,
    /// Configured optimizer budget stopped the optimizer cleanly.
    BudgetReached,
    /// A stage charge exceeded its hard budget limit.
    BudgetExceeded,
    /// A configured stopper stopped the optimizer cleanly.
    StopperTriggered,
    /// An external controller stopped the run.
    External,
    /// The run stopped because of an error.
    Error,
}

impl From<leaven_engine::StopReason> for OptimizationStopReason {
    fn from(reason: leaven_engine::StopReason) -> Self {
        match reason {
            leaven_engine::StopReason::OptimizerDone => Self::OptimizerDone,
            leaven_engine::StopReason::BudgetReached => Self::BudgetReached,
            leaven_engine::StopReason::BudgetExceeded => Self::BudgetExceeded,
            leaven_engine::StopReason::StopperTriggered => Self::StopperTriggered,
            leaven_engine::StopReason::External => Self::External,
            leaven_engine::StopReason::Error => Self::Error,
        }
    }
}

/// Public storage status for a product run.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
pub enum RunStorage {
    /// No stored-run resume promise exists for this run.
    Ephemeral {
        /// Run id for correlating events and reports.
        run_id: RunId,
    },
    /// Checkpoint persistence was configured for this run.
    Stored {
        /// Run id handed to the configured persistence capability.
        run_id: RunId,
        /// Local run directory when the product builder owns a resumable file
        /// store for this run.
        run_dir: Option<PathBuf>,
        /// Latest checkpoint known at result construction time.
        latest_checkpoint: Option<CheckpointId>,
        /// Whether the public product surface can resume this run, or why not.
        resumability: RunResumability,
    },
}

impl RunStorage {
    /// Whether the public product surface can resume this run.
    #[must_use]
    pub const fn is_resumable(&self) -> bool {
        matches!(
            self,
            Self::Stored {
                resumability: RunResumability::Resumable,
                ..
            }
        )
    }
}

/// Public stored-run resumability status.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
pub enum RunResumability {
    /// Stored run has the artifacts needed for product-level resume.
    Resumable,
    /// Stored run cannot be resumed through the product surface.
    NotResumable {
        /// Typed reason the run cannot be resumed.
        reason: RunNotResumableReason,
    },
}

/// Typed reason a stored run is not resumable.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize)]
pub enum RunNotResumableReason {
    /// The caller supplied a store capability without a Leaven-managed local run directory.
    ExplicitStoreWithoutLocalRunDir,
    /// No latest checkpoint was discoverable when the result was built.
    MissingLatestCheckpoint,
    /// No compatibility manifest summary was available for the stored run.
    MissingCompatibilityManifest,
}

impl RunNotResumableReason {
    /// Stable reason name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitStoreWithoutLocalRunDir => "explicit_store_without_local_run_dir",
            Self::MissingLatestCheckpoint => "missing_latest_checkpoint",
            Self::MissingCompatibilityManifest => "missing_compatibility_manifest",
        }
    }
}

/// Product report paths emitted for a run.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash, Serialize)]
pub struct RunReportPaths {
    /// Durable summary JSON written beside the run, when one exists.
    pub summary_json: Option<PathBuf>,
}

/// Public redacted summary of the compatibility manifest used for resume.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
pub struct RunCompatibilitySummary {
    /// Manifest schema.
    pub schema: String,
    /// Product run kind.
    pub run_kind: String,
    /// Dataset content fingerprint as lowercase hex.
    pub dataset: String,
    /// Split membership fingerprint as lowercase hex.
    pub splits: String,
    /// Derived case-set version.
    pub case_set_version: String,
    /// Runner behavior fingerprint as lowercase hex.
    pub runner: String,
    /// Scorer behavior fingerprint as lowercase hex.
    pub scorer: String,
    /// Composed evaluator/cache-key fingerprint as lowercase hex.
    pub evaluator: String,
    /// Optimizer compatibility declaration.
    pub optimizer: String,
    /// Cache compatibility declaration.
    pub cache: String,
    /// Budget compatibility declaration.
    pub budget: String,
    /// Number of role-specific LM fingerprints declared.
    pub lm_role_count: usize,
}

/// Public summary for an ordinary optimization run.
#[derive(Clone, Debug, Serialize)]
pub struct StandardRunSummary {
    /// Public storage/resume status for this run.
    pub storage: RunStorage,
    /// Report artifacts emitted for this run.
    pub reports: RunReportPaths,
    /// Redacted resume compatibility identity for durable runs.
    pub compatibility: Option<RunCompatibilitySummary>,
    /// Budget snapshot after optimizer work, before final report evaluations.
    pub optimization_budget: BudgetSnapshot,
    /// Budget snapshot after optimizer and final report evaluations.
    pub budget: BudgetSnapshot,
    /// Cost spent by optimizer work before final report evaluations.
    pub optimization_cost: Cost,
    /// Cost spent by final train/validation/test report evaluations.
    pub final_report_cost: Cost,
    /// Total visible evaluation/proposal cost after final reporting.
    pub cost: Cost,
    /// Cache behavior observed during the run.
    pub cache: RunCacheSummary,
    /// Baseline train score when train evidence exists.
    pub baseline_train_score: Option<f64>,
    /// Optimized train score when train evidence exists.
    pub optimized_train_score: Option<f64>,
    /// Optional validation score for the seed candidate.
    pub baseline_validation_score: Option<f64>,
    /// Optional validation score for best candidate.
    pub validation_score: Option<f64>,
    /// Optional final held-out test score for the seed candidate.
    pub baseline_test_score: Option<f64>,
    /// Optional final held-out test score for best candidate.
    pub test_score: Option<f64>,
    /// Split-aware report.
    pub evaluation: EvaluationReport,
}

/// Public cache summary for an ordinary optimization run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RunCacheSummary {
    /// Engine-owned evaluation cache summary.
    pub evaluation: EvaluationCacheSummary,
}

/// Engine evaluation-cache summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvaluationCacheSummary {
    /// Whether evaluation cache state is durable with the run.
    pub durable: bool,
    /// Human-readable backend class.
    pub backend: EvaluationCacheBackend,
    /// Number of evaluation cache hits.
    pub hits: u64,
    /// Number of cacheable evaluation misses.
    pub misses: u64,
    /// Bypasses grouped by reason.
    pub bypasses: Vec<EvaluationCacheBypassSummary>,
    /// Number of cache write errors observed by the product summary.
    pub write_errors: u64,
    /// Whether every observed cache hit charged zero run cost.
    pub hit_cost_zero: bool,
}

/// Evaluation cache backend reported by the product run summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize)]
pub enum EvaluationCacheBackend {
    /// Durable `SQLite` cache table in the local run store.
    SqliteRunStore,
    /// Durable checkpointed run-store cache index.
    CheckpointedRunStore,
    /// Non-durable in-memory cache state.
    InMemory,
}

impl EvaluationCacheBackend {
    /// Stable backend name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SqliteRunStore => "sqlite-run-store",
            Self::CheckpointedRunStore => "checkpointed-run-store",
            Self::InMemory => "in-memory",
        }
    }
}

/// Evaluation cache bypass count.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvaluationCacheBypassSummary {
    /// Bypass reason.
    pub reason: EvaluationCacheBypassReason,
    /// Number of observations.
    pub count: u64,
}

/// Public summary of why engine evaluation caching was bypassed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize)]
pub enum EvaluationCacheBypassReason {
    /// Evaluator declared `CachePolicy::Never`.
    DisabledByPolicy,
    /// Engine run context had no attached cache.
    CacheUnavailable,
    /// At least one candidate lacked a cache-safe identity.
    MissingCandidateIdentity,
}

impl EvaluationCacheBypassReason {
    /// Stable reason name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DisabledByPolicy => "disabled_by_policy",
            Self::CacheUnavailable => "cache_unavailable",
            Self::MissingCandidateIdentity => "missing_candidate_identity",
        }
    }
}

/// Public event summary emitted during an optimization run.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize)]
pub enum RunEventSummary {
    /// Optimization started.
    OptimizationStarted,
    /// Iteration started.
    IterationStarted,
    /// Budget was charged.
    BudgetCharged,
    /// A proposer produced a proposal batch.
    ProposalBatchProduced,
    /// A proposal was recorded.
    ProposalRecorded,
    /// A stage attempt was recorded.
    StageAttemptRecorded,
    /// A proposal applied successfully.
    ApplySucceeded,
    /// A proposal failed to apply.
    ApplyFailed,
    /// Evaluation was requested.
    EvaluationRequested,
    /// Evaluation completed.
    EvaluationCompleted,
    /// An external seam event was recorded.
    ExternalEventEmitted,
    /// Population state was updated.
    PopulationUpdated,
    /// Iteration ended.
    IterationEnded,
    /// Optimization is stopping.
    OptimizationStopping,
    /// Optimization ended.
    OptimizationEnded,
    /// Error event.
    Error,
}

impl RunEventSummary {
    /// Stable snake-case event name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OptimizationStarted => "optimization_started",
            Self::IterationStarted => "iteration_started",
            Self::BudgetCharged => "budget_charged",
            Self::ProposalBatchProduced => "proposal_batch_produced",
            Self::ProposalRecorded => "proposal_recorded",
            Self::StageAttemptRecorded => "stage_attempt_recorded",
            Self::ApplySucceeded => "apply_succeeded",
            Self::ApplyFailed => "apply_failed",
            Self::EvaluationRequested => "evaluation_requested",
            Self::EvaluationCompleted => "evaluation_completed",
            Self::ExternalEventEmitted => "external_event_emitted",
            Self::PopulationUpdated => "population_updated",
            Self::IterationEnded => "iteration_ended",
            Self::OptimizationStopping => "optimization_stopping",
            Self::OptimizationEnded => "optimization_ended",
            Self::Error => "error",
        }
    }
}

pub fn average(cases: &[leaven_eval::ReportScore]) -> Option<f64> {
    if cases.is_empty() {
        return None;
    }
    let total: f64 = cases.iter().map(|case| case.score).sum();
    let count = u32::try_from(cases.len()).expect("case count fits into u32");
    Some(total / f64::from(count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn average_refuses_empty_case_sets() {
        assert_eq!(average(&[]), None);
    }
}
