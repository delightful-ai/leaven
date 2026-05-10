//! Public runner and scoring evidence shapes.

/// Output produced by running one artifact on one case.
#[derive(Clone, Debug, Default)]
pub struct RunOutput {
    /// User-facing answer/output.
    pub output: String,
    /// Trace lines captured while running the artifact.
    pub trace: Vec<String>,
}

impl RunOutput {
    /// Builds an output with trace lines.
    #[must_use]
    pub fn new(output: impl Into<String>, trace: Vec<String>) -> Self {
        Self {
            output: output.into(),
            trace,
        }
    }
}

/// Scoring result for one case.
#[derive(Clone, Debug)]
pub struct Score {
    /// Comparable numeric score. Higher is better.
    pub value: f64,
    /// Natural-language feedback for reports and reflection.
    pub feedback: String,
    /// Structured feedback projected into stable report text for now.
    pub structured: Vec<(String, String)>,
}

impl Score {
    /// Builds a numeric score with natural-language feedback.
    #[must_use]
    pub fn new(value: f64, feedback: impl Into<String>) -> Self {
        Self {
            value,
            feedback: feedback.into(),
            structured: Vec::new(),
        }
    }
}

/// Context passed to scoring closures.
pub struct ScoreContext<'a, A, C> {
    /// Artifact/candidate that was run.
    pub artifact: &'a A,
    /// Evaluation case.
    pub case: &'a C,
    /// Runner output.
    pub output: &'a RunOutput,
}
