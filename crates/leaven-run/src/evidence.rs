//! Public runner and scoring evidence shapes.

use leaven_eval::Case;
use leaven_kernel::{BudgetSnapshot, CaseId, Cost};

/// Output produced by running one artifact on one case.
#[derive(Clone, Debug, Default)]
pub struct RunOutput {
    /// User-facing answer/output.
    pub output: String,
    /// Metered cost incurred while producing the output.
    pub cost: Cost,
}

impl RunOutput {
    /// Builds a generated output.
    #[must_use]
    pub fn new(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            cost: Cost::zero(),
        }
    }

    /// Attaches metered runner cost to the output.
    #[must_use]
    pub fn with_cost(mut self, cost: Cost) -> Self {
        self.cost = cost;
        self
    }
}

/// Failure returned by a user runner function.
#[derive(Debug, thiserror::Error)]
#[error("runner failed: {message}")]
pub struct RunError {
    message: String,
    trace: Vec<String>,
    cost: Box<Cost>,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

impl RunError {
    /// Builds a runner failure.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            trace: Vec::new(),
            cost: Box::new(Cost::zero()),
            source: None,
        }
    }

    /// Builds a runner failure while preserving a lower-level source.
    #[must_use]
    pub fn with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            message: message.into(),
            trace: Vec::new(),
            cost: Box::new(Cost::zero()),
            source: Some(Box::new(source)),
        }
    }

    /// Adds one trace line.
    #[must_use]
    pub fn with_trace(mut self, line: impl Into<String>) -> Self {
        self.trace.push(line.into());
        self
    }

    /// Attaches metered runner cost incurred before failure.
    #[must_use]
    pub fn with_cost(mut self, cost: Cost) -> Self {
        self.cost = Box::new(cost);
        self
    }

    /// Human-facing failure message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Trace lines captured before failure.
    #[must_use]
    pub fn trace(&self) -> &[String] {
        &self.trace
    }

    /// Metered cost incurred before failure.
    #[must_use]
    pub fn cost(&self) -> &Cost {
        &self.cost
    }
}

/// Conversion accepted by the public runner builder.
pub trait IntoRunResult {
    /// Converts a runner return value into the fallible runner contract.
    fn into_run_result(self) -> Result<RunOutput, RunError>;
}

impl IntoRunResult for RunOutput {
    fn into_run_result(self) -> Result<RunOutput, RunError> {
        Ok(self)
    }
}

impl IntoRunResult for Result<RunOutput, RunError> {
    fn into_run_result(self) -> Result<RunOutput, RunError> {
        self
    }
}

/// Scoring result for one case.
#[derive(Clone, Debug)]
pub struct Score {
    /// Comparable numeric score. Higher is better.
    pub value: f64,
    /// Natural-language feedback for reports and reflection.
    pub feedback: String,
    /// Metered cost incurred while producing the score.
    pub cost: Cost,
}

impl Score {
    /// Builds a numeric score with natural-language feedback.
    #[must_use]
    pub fn new(value: f64, feedback: impl Into<String>) -> Self {
        Self {
            value,
            feedback: feedback.into(),
            cost: Cost::zero(),
        }
    }

    /// Attaches metered scorer cost.
    #[must_use]
    pub fn with_cost(mut self, cost: Cost) -> Self {
        self.cost = cost;
        self
    }
}

/// Failure returned by a user scoring function.
#[derive(Debug, thiserror::Error)]
#[error("score failed: {message}")]
pub struct ScoreError {
    message: String,
    trace: Vec<String>,
    cost: Box<Cost>,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

impl ScoreError {
    /// Builds a scoring failure.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            trace: Vec::new(),
            cost: Box::new(Cost::zero()),
            source: None,
        }
    }

    /// Builds a scoring failure while preserving a lower-level source.
    #[must_use]
    pub fn with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            message: message.into(),
            trace: Vec::new(),
            cost: Box::new(Cost::zero()),
            source: Some(Box::new(source)),
        }
    }

    /// Adds one trace line.
    #[must_use]
    pub fn with_trace(mut self, line: impl Into<String>) -> Self {
        self.trace.push(line.into());
        self
    }

    /// Attaches metered scorer cost incurred before failure.
    #[must_use]
    pub fn with_cost(mut self, cost: Cost) -> Self {
        self.cost = Box::new(cost);
        self
    }

    /// Human-facing failure message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Trace lines captured before failure.
    #[must_use]
    pub fn trace(&self) -> &[String] {
        &self.trace
    }

    /// Metered cost incurred before failure.
    #[must_use]
    pub fn cost(&self) -> &Cost {
        &self.cost
    }
}

/// Target-free case view passed to ordinary runner closures.
#[derive(Clone, Debug)]
pub struct RunCase<I> {
    id: CaseId,
    input: I,
}

impl<I> RunCase<I> {
    pub(crate) fn from_case<T>(case: &Case<I, T>) -> Self
    where
        I: Clone,
    {
        Self {
            id: case.id,
            input: case.input.clone(),
        }
    }

    /// Stable case id visible to the runner.
    #[must_use]
    pub const fn id(&self) -> CaseId {
        self.id
    }

    /// Target-free input visible to the runner.
    #[must_use]
    pub const fn input(&self) -> &I {
        &self.input
    }

    /// Consumes the view and returns the target-free input.
    #[must_use]
    pub fn into_input(self) -> I {
        self.input
    }
}

/// Empty scorer metadata projection for the ordinary product path.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScoreMetadataView;

impl ScoreMetadataView {
    /// Returns true because the current ordinary scorer projection is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        true
    }
}

/// Target-aware case view passed to scoring closures.
#[derive(Clone, Debug)]
pub struct ScoreCase<I, T = leaven_eval::NoTarget> {
    id: CaseId,
    input: I,
    target: Option<T>,
    metadata: ScoreMetadataView,
}

impl<I, T> ScoreCase<I, T> {
    pub(crate) fn from_case(case: &Case<I, T>) -> Self
    where
        I: Clone,
        T: Clone,
    {
        Self {
            id: case.id,
            input: case.input.clone(),
            target: case.target.clone(),
            metadata: ScoreMetadataView,
        }
    }

    /// Stable case id visible to the scorer.
    #[must_use]
    pub const fn id(&self) -> CaseId {
        self.id
    }

    /// Case input visible to the scorer.
    #[must_use]
    pub const fn input(&self) -> &I {
        &self.input
    }

    /// Optional reference target visible to the scorer.
    #[must_use]
    pub fn target(&self) -> Option<&T> {
        self.target.as_ref()
    }

    /// Explicit scorer metadata projection.
    #[must_use]
    pub const fn metadata(&self) -> ScoreMetadataView {
        self.metadata
    }
}

/// Context passed to scoring closures.
pub struct ScoreContext<A, I, T = leaven_eval::NoTarget> {
    /// Artifact/candidate that was run.
    pub artifact: A,
    /// Evaluation case with target-aware scorer visibility.
    pub case: ScoreCase<I, T>,
    /// Runner output.
    pub output: RunOutput,
    /// Point-in-time budget snapshot visible to the scorer.
    pub budget: BudgetSnapshot,
}
