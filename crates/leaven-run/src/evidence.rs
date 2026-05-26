//! Public runner and scoring evidence shapes.

use std::fmt;

use leaven_core::Artifact;
use leaven_eval::Case;
use leaven_evidence::{CaseDataReadEvidence, OutputRecord};
use leaven_kernel::{BudgetSnapshot, CaseId, Cost};

mod reportable;
mod runner;

pub use reportable::{ReportableOutput, ReportableOutputDeclaration, ReportableOutputScope};
pub use runner::{IntoRunResult, RunError, RunOutput, artifact_identity_output};

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

#[derive(Clone, Debug, Default)]
pub(crate) struct CaseDataReadLog(std::sync::Arc<std::sync::Mutex<Vec<CaseDataReadEvidence>>>);

impl CaseDataReadLog {
    pub(crate) fn record_target_read(&self, case: CaseId) {
        self.0
            .lock()
            .expect("case data read log lock was poisoned")
            .push(CaseDataReadEvidence::new(
                "case_query.load",
                format!("qrec_case_{}_target", case.0),
                case,
                ["target"],
                ["case.target"],
            ));
    }

    pub(crate) fn snapshot(&self) -> Vec<CaseDataReadEvidence> {
        self.0
            .lock()
            .expect("case data read log lock was poisoned")
            .clone()
    }
}

/// Case view passed to scoring closures.
#[derive(Clone)]
pub struct ScoreCase<I, T = leaven_eval::NoTarget> {
    id: CaseId,
    input: I,
    target: Option<T>,
}

impl<I: fmt::Debug, T> fmt::Debug for ScoreCase<I, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScoreCase")
            .field("id", &self.id)
            .field("input", &self.input)
            .field("target", &"<loaded through ScoreContext::load_target>")
            .finish()
    }
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

    pub(crate) fn target_material(&self) -> Option<&T> {
        self.target.as_ref()
    }
}

/// Context passed to scoring closures.
pub struct ScoreContext<A, I, T = leaven_eval::NoTarget, Out = ()> {
    /// Artifact/candidate that was run.
    pub artifact: A,
    /// Evaluation case visible to the scorer.
    pub case: ScoreCase<I, T>,
    /// Runner output.
    pub output: RunOutput<Out>,
    /// Point-in-time budget snapshot visible to the scorer.
    pub budget: BudgetSnapshot,
    output_scope: ReportableOutputScope,
    expected_output: Option<ReportableOutputDeclaration>,
    case_data_reads: CaseDataReadLog,
}

impl<A, I, T, Out> ScoreContext<A, I, T, Out> {
    pub(crate) fn new(
        artifact: A,
        case: ScoreCase<I, T>,
        output: RunOutput<Out>,
        budget: BudgetSnapshot,
        output_scope: ReportableOutputScope,
        case_data_reads: CaseDataReadLog,
    ) -> Self {
        let expected_output = output.reportable_output().cloned();
        Self {
            artifact,
            case,
            output,
            budget,
            output_scope,
            expected_output,
            case_data_reads,
        }
    }

    /// Loads the optional case target through the scorer's audited case-data read path.
    ///
    /// This is the scorer-side analogue of the public-seam `case_query.load`
    /// target read: callers must make target access explicit, and the evaluator
    /// attaches a target-read evidence record to the assessment.
    #[must_use]
    pub fn load_target(&self) -> Option<&T> {
        let target = self.case.target_material();
        if target.is_some() {
            self.case_data_reads.record_target_read(self.case.id());
        }
        target
    }

    /// Wraps a generated output record for this exact scoring context.
    #[must_use]
    pub fn report_output(&self, output: OutputRecord) -> ReportableOutput {
        ReportableOutput::new(
            output,
            self.output_scope.clone(),
            self.expected_output.clone(),
        )
    }

    /// Wraps inline generated output text for this exact scoring context.
    #[must_use]
    pub fn report_text_output(&self, output: impl Into<String>) -> ReportableOutput {
        self.report_output(OutputRecord::inline(output))
    }

    /// Reports the current candidate artifact identity as the assessed output.
    #[must_use]
    pub fn report_artifact_identity_output(&self) -> ReportableOutput
    where
        A: Artifact,
    {
        let output = artifact_identity_output(&self.artifact);
        ReportableOutput::new(
            output.clone(),
            self.output_scope.clone(),
            Some(ReportableOutputDeclaration::derived(output)),
        )
    }
}
