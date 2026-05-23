//! Public runner and scoring evidence shapes.

use leaven_eval::Case;
use leaven_evidence::OutputRecord;
use leaven_kernel::{BudgetSnapshot, CandidateId, CaseId, Cost};

/// Output produced by running one artifact on one case.
///
/// The default `Out = ()` matches `case_visibility_and_target_isolation.md` §6:
/// runner output is opaque to Leaven by default. Domain runners declare their
/// own typed `Out` (string, structured prediction, agent transcript), and the
/// scorer renders that output into a context-scoped durable `ReportableOutput`
/// via [`ScoreContext::report_output`] / [`ScoreContext::report_text_output`].
#[derive(Clone, Debug, Default)]
pub struct RunOutput<Out = ()> {
    /// User-facing answer/output.
    pub output: Out,
    /// Metered cost incurred while producing the output.
    pub cost: Cost,
    /// Runner trace lines associated with this successful output.
    pub trace: Vec<String>,
}

impl RunOutput<String> {
    /// Builds a generated string output.
    #[must_use]
    pub fn new(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            cost: Cost::zero(),
            trace: Vec::new(),
        }
    }
}

impl<Out> RunOutput<Out> {
    /// Builds a typed generated output.
    #[must_use]
    pub fn typed(output: Out) -> Self {
        Self {
            output,
            cost: Cost::zero(),
            trace: Vec::new(),
        }
    }

    /// Attaches metered runner cost to the output.
    #[must_use]
    pub fn with_cost(mut self, cost: Cost) -> Self {
        self.cost = cost;
        self
    }

    /// Adds one successful-run trace line.
    #[must_use]
    pub fn with_trace(mut self, line: impl Into<String>) -> Self {
        self.trace.push(line.into());
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
pub trait IntoRunResult<Out = ()> {
    /// Converts a runner return value into the fallible runner contract.
    fn into_run_result(self) -> Result<RunOutput<Out>, RunError>;
}

impl<Out> IntoRunResult<Out> for RunOutput<Out> {
    fn into_run_result(self) -> Result<Self, RunError> {
        Ok(self)
    }
}

impl<Out> IntoRunResult<Out> for Result<RunOutput<Out>, RunError> {
    fn into_run_result(self) -> Self {
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
    /// Scorer trace lines associated with this successful score.
    pub trace: Vec<String>,
    /// Reportable generated output minted by the current scoring context.
    pub output: Option<ReportableOutput>,
}

impl Score {
    /// Builds a numeric score with natural-language feedback.
    #[must_use]
    pub fn new(value: f64, feedback: impl Into<String>) -> Self {
        Self {
            value,
            feedback: feedback.into(),
            cost: Cost::zero(),
            trace: Vec::new(),
            output: None,
        }
    }

    /// Attaches metered scorer cost.
    #[must_use]
    pub fn with_cost(mut self, cost: Cost) -> Self {
        self.cost = cost;
        self
    }

    /// Adds one successful-score trace line.
    #[must_use]
    pub fn with_trace(mut self, line: impl Into<String>) -> Self {
        self.trace.push(line.into());
        self
    }

    /// Supplies the context-scoped generated output for reports and feedback.
    #[must_use]
    pub fn with_output(mut self, output: ReportableOutput) -> Self {
        self.output = Some(output);
        self
    }
}

/// Reportable score output minted from one scoring context.
///
/// The private scope prevents a scorer from satisfying the output contract with
/// a reusable placeholder. The evaluator unwraps it only when it belongs to the
/// candidate/case context currently being assessed.
#[derive(Clone, Debug)]
pub struct ReportableOutput {
    record: OutputRecord,
    scope: ReportableOutputScope,
}

impl ReportableOutput {
    pub(crate) fn into_record(
        self,
        expected_scope: ReportableOutputScope,
    ) -> Result<OutputRecord, ReportableOutputScopeError> {
        if self.scope == expected_scope {
            Ok(self.record)
        } else {
            Err(ReportableOutputScopeError)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportableOutputScope {
    candidate: CandidateId,
    case: CaseId,
}

impl ReportableOutputScope {
    pub(crate) const fn new(candidate: CandidateId, case: CaseId) -> Self {
        Self { candidate, case }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("reportable output came from another scoring context")]
pub struct ReportableOutputScopeError;

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
pub struct ScoreContext<A, I, T = leaven_eval::NoTarget, Out = ()> {
    /// Artifact/candidate that was run.
    pub artifact: A,
    /// Evaluation case with target-aware scorer visibility.
    pub case: ScoreCase<I, T>,
    /// Runner output.
    pub output: RunOutput<Out>,
    /// Point-in-time budget snapshot visible to the scorer.
    pub budget: BudgetSnapshot,
    output_scope: ReportableOutputScope,
}

impl<A, I, T, Out> ScoreContext<A, I, T, Out> {
    pub(crate) const fn new(
        artifact: A,
        case: ScoreCase<I, T>,
        output: RunOutput<Out>,
        budget: BudgetSnapshot,
        output_scope: ReportableOutputScope,
    ) -> Self {
        Self {
            artifact,
            case,
            output,
            budget,
            output_scope,
        }
    }

    /// Wraps a generated output record for this exact scoring context.
    #[must_use]
    pub fn report_output(&self, output: OutputRecord) -> ReportableOutput {
        ReportableOutput {
            record: output,
            scope: self.output_scope,
        }
    }

    /// Wraps inline generated output text for this exact scoring context.
    #[must_use]
    pub fn report_text_output(&self, output: impl Into<String>) -> ReportableOutput {
        self.report_output(OutputRecord::inline(output))
    }
}
