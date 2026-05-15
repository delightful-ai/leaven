//! Public optimization result facade.

use std::path::PathBuf;

use leaven_eval::EvaluationReport;
use leaven_kernel::{BudgetSnapshot, CandidateId, CheckpointId, Cost, RunId};

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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
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
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
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
        /// Whether the public product surface can resume this run.
        resumable: bool,
    },
}

/// Public summary for an ordinary optimization run.
#[derive(Clone, Debug)]
pub struct StandardRunSummary {
    /// Public storage/resume status for this run.
    pub storage: RunStorage,
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

/// Public event summary emitted during an optimization run.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
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
