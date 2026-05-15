//! Public optimization result facade.

use leaven_eval::EvaluationReport;
use leaven_kernel::{BudgetSnapshot, CandidateId, Cost, RunId};

/// Result returned by the product builder.
#[derive(Clone, Debug)]
pub struct OptimizeResult<A> {
    /// Run id.
    pub run_id: RunId,
    /// Best candidate id.
    pub best: CandidateId,
    /// Best artifact cloned from graph truth.
    pub best_artifact: A,
    /// Seed artifact.
    pub seed_artifact: A,
    /// Public report.
    pub report: OptimizationReport,
}

impl<A> OptimizeResult<A> {
    /// Best artifact.
    #[must_use]
    pub const fn best(&self) -> &A {
        &self.best_artifact
    }

    /// Report facade.
    #[must_use]
    pub const fn report(&self) -> &OptimizationReport {
        &self.report
    }
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum RunStorage {
    /// No stored-run resume promise exists for this run.
    Ephemeral {
        /// Run id for correlating events and reports.
        run_id: RunId,
    },
    /// Checkpoint persistence was configured, but resume is not promised yet.
    Stored {
        /// Run id handed to the configured persistence capability.
        run_id: RunId,
        /// Whether the public product surface can resume this run.
        resumable: bool,
    },
}

/// Public report for an optimization run.
#[derive(Clone, Debug)]
pub struct OptimizationReport {
    /// Reason the optimizer stopped before final report evaluations ran.
    pub stop_reason: OptimizationStopReason,
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
    /// Event names emitted during the run.
    pub events: Vec<String>,
}

pub fn average(cases: &[leaven_eval::ReportScore]) -> Option<f64> {
    if cases.is_empty() {
        return None;
    }
    let total: f64 = cases.iter().map(|case| case.score).sum();
    let count = u32::try_from(cases.len()).expect("case count fits into u32");
    Some(total / f64::from(count))
}
