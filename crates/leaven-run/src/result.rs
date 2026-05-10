//! Public optimization result facade.

use leaven_eval::EvaluationReport;
use leaven_kernel::{BudgetSnapshot, CandidateId, Cost, Fingerprint, RunId};

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

/// Public report for an optimization run.
#[derive(Clone, Debug)]
pub struct OptimizationReport {
    /// Dataset fingerprint.
    pub dataset: Fingerprint,
    /// Split fingerprint.
    pub splits: Fingerprint,
    /// Budget remaining after the run.
    pub budget: BudgetSnapshot,
    /// Total visible evaluation/proposal cost.
    pub cost: Cost,
    /// Baseline train score.
    pub baseline_train_score: f64,
    /// Optimized train score.
    pub optimized_train_score: f64,
    /// Optional validation score for best candidate.
    pub validation_score: Option<f64>,
    /// Optional final held-out test score for best candidate.
    pub test_score: Option<f64>,
    /// Split-aware report.
    pub evaluation: EvaluationReport,
    /// Event names emitted during the run.
    pub events: Vec<String>,
}

pub(crate) fn average(cases: &[leaven_eval::ReportScore]) -> f64 {
    if cases.is_empty() {
        return 0.0;
    }
    let total: f64 = cases.iter().map(|case| case.score).sum();
    let count = u32::try_from(cases.len()).expect("case count fits into u32");
    total / f64::from(count)
}
