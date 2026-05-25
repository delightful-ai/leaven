use leaven_core::{Artifact, ArtifactIdentity};
use leaven_evidence::OutputRecord;
use leaven_kernel::Cost;

use super::ReportableOutputDeclaration;

/// Output produced by running one artifact on one case.
///
/// The default `Out = ()` matches `case_visibility_and_target_isolation.md` §6:
/// runner output is opaque to Leaven by default. Domain runners declare their
/// own typed `Out` (string, structured prediction, agent transcript). The
/// runner also declares the durable assessed output record; the scorer must
/// report that declared record through [`crate::ScoreContext::report_output`] /
/// [`crate::ScoreContext::report_text_output`].
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

fn candidate_output_record(output: impl Into<String>) -> OutputRecord {
    OutputRecord::candidate_inline(output)
}
