//! Public runner and scoring evidence shapes.

use leaven_evidence::FeedbackAttachment;
use leaven_kernel::{BudgetSnapshot, Cost};

/// Output produced by running one artifact on one case.
#[derive(Clone, Debug, Default)]
pub struct RunOutput {
    /// User-facing answer/output.
    pub output: String,
    /// Trace lines captured while running the artifact.
    pub trace: Vec<String>,
    /// Metered cost incurred while producing the output.
    pub cost: Cost,
}

impl RunOutput {
    /// Builds an output with trace lines.
    #[must_use]
    pub fn new(output: impl Into<String>, trace: Vec<String>) -> Self {
        Self {
            output: output.into(),
            trace,
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

/// Scoring result for one case.
#[derive(Clone, Debug)]
pub struct Score {
    /// Comparable numeric score. Higher is better.
    pub value: f64,
    /// Natural-language feedback for reports and reflection.
    pub feedback: String,
    /// Structured feedback projected into stable report text.
    pub structured: Vec<(String, String)>,
    /// Named payloads that preserve richer judge/program evidence.
    pub attachments: Vec<FeedbackAttachment>,
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
            structured: Vec::new(),
            attachments: Vec::new(),
            cost: Cost::zero(),
        }
    }

    /// Adds one structured feedback field.
    #[must_use]
    pub fn with_structured(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.structured.push((key.into(), value.into()));
        self
    }

    /// Adds one attachment.
    #[must_use]
    pub fn with_attachment(mut self, attachment: FeedbackAttachment) -> Self {
        self.attachments.push(attachment);
        self
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
    cost: Cost,
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
            cost: Cost::zero(),
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
            cost: Cost::zero(),
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
        self.cost = cost;
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

/// Context passed to scoring closures.
pub struct ScoreContext<A, C> {
    /// Artifact/candidate that was run.
    pub artifact: A,
    /// Evaluation case.
    pub case: C,
    /// Runner output.
    pub output: RunOutput,
    /// Point-in-time budget snapshot visible to the scorer.
    pub budget: BudgetSnapshot,
}
