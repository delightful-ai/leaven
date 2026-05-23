//! Public runner and scoring evidence shapes.

use leaven_eval::Case;
use leaven_evidence::OutputRecord;
use leaven_kernel::{BudgetSnapshot, CandidateId, CaseId, Cost};

/// Output produced by running one artifact on one case.
///
/// The default `Out = ()` matches `case_visibility_and_target_isolation.md` §6:
/// runner output is opaque to Leaven by default. Domain runners declare their
/// own typed `Out` (string, structured prediction, agent transcript). The
/// runner also declares the durable assessed output record; the scorer must
/// report that declared record through [`ScoreContext::report_output`] /
/// [`ScoreContext::report_text_output`].
#[derive(Clone, Debug, Default)]
pub struct RunOutput<Out = ()> {
    /// User-facing answer/output.
    pub output: Out,
    /// Metered cost incurred while producing the output.
    pub cost: Cost,
    /// Runner trace lines associated with this successful output.
    pub trace: Vec<String>,
    reportable_output: Option<OutputRecord>,
}

impl RunOutput<String> {
    /// Builds a generated string output.
    #[must_use]
    pub fn new(output: impl Into<String>) -> Self {
        let output = output.into();
        Self {
            reportable_output: Some(candidate_output_record(output.clone())),
            output,
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
            reportable_output: None,
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

    /// Attaches the reportable output record that downstream scores must carry.
    ///
    /// Typed runner outputs are opaque to Leaven. A runner that returns a typed
    /// value must also declare the exact reportable rendering that scorers are
    /// assessing; otherwise the evaluator refuses successful scores because it
    /// cannot distinguish a meaningful `Score.output` from a dummy field.
    #[must_use]
    pub fn with_reportable_output(mut self, output: OutputRecord) -> Self {
        self.reportable_output = Some(output);
        self
    }

    /// Attaches an inline reportable text rendering for a typed runner output.
    #[must_use]
    pub fn with_reportable_text(self, output: impl Into<String>) -> Self {
        self.with_reportable_output(candidate_output_record(output))
    }

    pub(crate) fn reportable_output(&self) -> Option<&OutputRecord> {
        self.reportable_output.as_ref()
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
/// candidate/case or candidate-group/case context currently being assessed.
#[derive(Clone, Debug)]
pub struct ReportableOutput {
    record: OutputRecord,
    scope: ReportableOutputScope,
    expected: Option<OutputRecord>,
}

impl ReportableOutput {
    pub(crate) fn new(
        record: OutputRecord,
        scope: ReportableOutputScope,
        expected: Option<OutputRecord>,
    ) -> Self {
        Self {
            record,
            scope,
            expected,
        }
    }

    pub(crate) fn into_record(
        self,
        expected_scope: &ReportableOutputScope,
    ) -> Result<OutputRecord, ReportableOutputError> {
        if self.scope != *expected_scope {
            return Err(ReportableOutputError::WrongScope);
        }
        if is_placeholder_output(&self.record) {
            return Err(ReportableOutputError::Placeholder);
        }
        let Some(expected) = self.expected else {
            return Err(ReportableOutputError::MissingAssessedOutput);
        };
        if !same_output_payload(&self.record, &expected) {
            return Err(ReportableOutputError::Unrelated);
        }
        Ok(expected)
    }
}

fn is_placeholder_output(record: &OutputRecord) -> bool {
    matches!(record, OutputRecord::Inline { text, .. } if text.trim().is_empty())
}

fn same_output_payload(reported: &OutputRecord, expected: &OutputRecord) -> bool {
    match (reported, expected) {
        (
            OutputRecord::Inline {
                text: reported,
                truncated: reported_truncated,
                ..
            },
            OutputRecord::Inline {
                text: expected,
                truncated: expected_truncated,
                ..
            },
        ) => reported == expected && reported_truncated == expected_truncated,
        (
            OutputRecord::BlobRef {
                reference: reported,
                ..
            },
            OutputRecord::BlobRef {
                reference: expected,
                ..
            },
        ) => reported == expected,
        _ => false,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportableOutputScope {
    candidates: Vec<CandidateId>,
    case: CaseId,
}

impl ReportableOutputScope {
    pub(crate) fn new(candidate: CandidateId, case: CaseId) -> Self {
        Self {
            candidates: vec![candidate],
            case,
        }
    }

    pub(crate) fn group(candidates: Vec<CandidateId>, case: CaseId) -> Self {
        Self { candidates, case }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReportableOutputError {
    /// The output was minted for a different candidate/case scoring context.
    #[error("reportable output came from another scoring context")]
    WrongScope,
    /// The output exists only as an empty inline placeholder.
    #[error("reportable output was an empty placeholder")]
    Placeholder,
    /// The runner did not declare the assessed output record.
    #[error("runner output did not declare reportable assessed output")]
    MissingAssessedOutput,
    /// The score reported output that was not the runner output being assessed.
    #[error("reportable output did not match assessed output")]
    Unrelated,
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
    expected_output: Option<OutputRecord>,
}

impl<A, I, T, Out> ScoreContext<A, I, T, Out> {
    pub(crate) fn new(
        artifact: A,
        case: ScoreCase<I, T>,
        output: RunOutput<Out>,
        budget: BudgetSnapshot,
        output_scope: ReportableOutputScope,
    ) -> Self {
        let expected_output = output.reportable_output().cloned();
        Self {
            artifact,
            case,
            output,
            budget,
            output_scope,
            expected_output,
        }
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
}

fn candidate_output_record(output: impl Into<String>) -> OutputRecord {
    OutputRecord::candidate_inline(output)
}

#[cfg(test)]
mod tests {
    use leaven_evidence::OutputRecord;
    use leaven_kernel::{CandidateId, CaseId};

    use super::{ReportableOutputError, ReportableOutputScope};

    #[test]
    fn grouped_reportable_output_scopes_do_not_match_single_candidate_scopes() {
        let case = CaseId::new(1);
        let left = CandidateId::new();
        let right = CandidateId::new();
        let group = ReportableOutputScope::group(vec![left, right], case);
        let single = ReportableOutputScope::new(left, case);
        let output = super::ReportableOutput {
            record: OutputRecord::inline("left/right comparison"),
            scope: group.clone(),
            expected: Some(OutputRecord::inline("left/right comparison")),
        };

        assert!(matches!(
            output.clone().into_record(&single),
            Err(ReportableOutputError::WrongScope)
        ));
        assert!(output.into_record(&group).is_ok());
    }
}
