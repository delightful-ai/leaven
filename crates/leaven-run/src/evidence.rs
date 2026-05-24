//! Public runner and scoring evidence shapes.

use std::fmt;

use leaven_core::{Artifact, ArtifactIdentity};
use leaven_eval::Case;
use leaven_evidence::{CaseDataReadEvidence, DataClass, OutputRecord};
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
    reportable_output: Option<ReportableOutputDeclaration>,
}

impl RunOutput<String> {
    /// Builds a generated string output.
    #[must_use]
    pub fn new(output: impl Into<String>) -> Self {
        let output = output.into();
        Self {
            reportable_output: Some(ReportableOutputDeclaration::derived(
                candidate_output_record(output.clone()),
            )),
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

    /// Attaches an explicit non-candidate reportable output declaration.
    ///
    /// Typed runner outputs are opaque to Leaven. A runner that returns a typed
    /// value must declare the exact reportable rendering that scorers are
    /// assessing; otherwise the evaluator refuses successful scores because it
    /// cannot distinguish a meaningful `Score.output` from a dummy field.
    /// Use [`Self::with_reportable_text`] for candidate outputs derived from
    /// the typed output, or [`Self::with_reportable_artifact_identity`] when the
    /// score intentionally assesses the artifact itself. Generic explicit
    /// `candidate.output` and `candidate.artifact` records are rejected during
    /// evaluator lowering.
    #[must_use]
    pub fn with_reportable_output(mut self, output: OutputRecord) -> Self {
        self.reportable_output = Some(ReportableOutputDeclaration::explicit(output));
        self
    }

    /// Attaches the candidate artifact identity as the reportable output.
    #[must_use]
    pub fn with_reportable_artifact_identity<A: Artifact>(mut self, artifact: &A) -> Self {
        self.reportable_output = Some(ReportableOutputDeclaration::derived(
            artifact_identity_output(artifact),
        ));
        self
    }

    /// Attaches an inline reportable text rendering for a typed runner output.
    #[must_use]
    pub fn with_reportable_text(self, output: impl Into<String>) -> Self {
        let mut this = self;
        this.reportable_output = Some(ReportableOutputDeclaration::derived(
            candidate_output_record(output),
        ));
        this
    }

    pub(crate) fn reportable_output(&self) -> Option<&ReportableOutputDeclaration> {
        self.reportable_output.as_ref()
    }
}

/// Stable public score-output projection for an artifact being assessed.
#[must_use]
pub fn artifact_identity_output<A: Artifact>(artifact: &A) -> OutputRecord {
    OutputRecord::candidate_artifact_inline(artifact_identity_text(artifact.identity()))
}

fn artifact_identity_text(identity: ArtifactIdentity) -> String {
    match identity {
        ArtifactIdentity::Content(content) => {
            format!("artifact:content:{}", hex_bytes(content.as_bytes()))
        }
        ArtifactIdentity::External(label) => format!("artifact:external:{label}"),
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 0x0f) as usize] as char);
    }
    text
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
    expected: Option<ReportableOutputDeclaration>,
}

impl ReportableOutput {
    pub(crate) fn new(
        record: OutputRecord,
        scope: ReportableOutputScope,
        expected: Option<ReportableOutputDeclaration>,
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
        if expected.is_unbound_explicit_candidate_output() {
            return Err(ReportableOutputError::UnboundCandidateOutput);
        }
        if expected.is_unbound_explicit_candidate_artifact() {
            return Err(ReportableOutputError::UnboundCandidateArtifact);
        }
        if !is_assessed_candidate_or_artifact_output(&expected.record) {
            return Err(ReportableOutputError::MissingAssessedDataClass);
        }
        if !same_output_payload(&self.record, &expected.record) {
            return Err(ReportableOutputError::Unrelated);
        }
        Ok(expected.record)
    }
}

#[derive(Clone, Debug)]
pub struct ReportableOutputDeclaration {
    record: OutputRecord,
    origin: ReportableOutputOrigin,
}

impl ReportableOutputDeclaration {
    pub(crate) fn derived(record: OutputRecord) -> Self {
        Self {
            record,
            origin: ReportableOutputOrigin::DerivedFromRunnerOutput,
        }
    }

    pub(crate) fn explicit(record: OutputRecord) -> Self {
        Self {
            record,
            origin: ReportableOutputOrigin::ExplicitRecord,
        }
    }

    pub(crate) fn record(&self) -> &OutputRecord {
        &self.record
    }

    pub(crate) fn is_unbound_explicit_candidate_output(&self) -> bool {
        self.origin == ReportableOutputOrigin::ExplicitRecord
            && self
                .record
                .data_classes()
                .contains(&DataClass::candidate_output())
    }

    pub(crate) fn is_unbound_explicit_candidate_artifact(&self) -> bool {
        self.origin == ReportableOutputOrigin::ExplicitRecord
            && self
                .record
                .data_classes()
                .contains(&DataClass::candidate_artifact())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReportableOutputOrigin {
    DerivedFromRunnerOutput,
    ExplicitRecord,
}

fn is_placeholder_output(record: &OutputRecord) -> bool {
    matches!(record, OutputRecord::Inline { text, .. } if text.trim().is_empty())
}

fn is_assessed_candidate_or_artifact_output(record: &OutputRecord) -> bool {
    let classes = record.data_classes();
    classes.contains(&DataClass::candidate_output())
        || classes.contains(&DataClass::candidate_artifact())
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
    /// The runner-declared output is not classified as assessed candidate/artifact output.
    #[error("runner output did not declare candidate or artifact assessed output")]
    MissingAssessedDataClass,
    /// The runner explicitly declared candidate-output data without a typed-output binding.
    #[error("runner output did not derive candidate output from typed output")]
    UnboundCandidateOutput,
    /// The runner explicitly declared candidate-artifact data without deriving it from artifact identity.
    #[error("runner output did not derive candidate artifact from artifact identity")]
    UnboundCandidateArtifact,
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

#[derive(Clone, Debug, Default)]
pub struct CaseDataReadLog(std::sync::Arc<std::sync::Mutex<Vec<CaseDataReadEvidence>>>);

impl CaseDataReadLog {
    pub fn record_target_read(&self, case: CaseId) {
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

    pub fn snapshot(&self) -> Vec<CaseDataReadEvidence> {
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
    metadata: ScoreMetadataView,
}

impl<I: fmt::Debug, T> fmt::Debug for ScoreCase<I, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScoreCase")
            .field("id", &self.id)
            .field("input", &self.input)
            .field("target", &"<loaded through ScoreContext::load_target>")
            .field("metadata", &self.metadata)
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

    pub(crate) fn target_material(&self) -> Option<&T> {
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

fn candidate_output_record(output: impl Into<String>) -> OutputRecord {
    OutputRecord::candidate_inline(output)
}

#[cfg(test)]
mod tests {
    use leaven_evidence::OutputRecord;
    use leaven_kernel::{CandidateId, CaseId};

    use super::{ReportableOutputDeclaration, ReportableOutputError, ReportableOutputScope};

    #[test]
    fn grouped_reportable_output_scopes_do_not_match_single_candidate_scopes() {
        let case = CaseId::new(1);
        let left = CandidateId::new();
        let right = CandidateId::new();
        let group = ReportableOutputScope::group(vec![left, right], case);
        let single = ReportableOutputScope::new(left, case);
        let output = super::ReportableOutput {
            record: OutputRecord::candidate_inline("left/right comparison"),
            scope: group.clone(),
            expected: Some(ReportableOutputDeclaration::derived(
                OutputRecord::candidate_inline("left/right comparison"),
            )),
        };

        assert!(matches!(
            output.clone().into_record(&single),
            Err(ReportableOutputError::WrongScope)
        ));
        assert!(output.into_record(&group).is_ok());
    }
}
