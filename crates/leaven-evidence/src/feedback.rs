//! Score evidence that preserves textual feedback and execution traces.

use leaven_core::Evidence;
use serde::{Deserialize, Serialize};

use crate::{OutputRecord, ScalarEvidence};

/// Named feedback payload attached to one scored outcome.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FeedbackAttachment {
    name: String,
    media_type: Option<String>,
    record: OutputRecord,
}

impl FeedbackAttachment {
    /// Builds a feedback attachment from an output record.
    #[must_use]
    pub fn new(name: impl Into<String>, media_type: Option<String>, record: OutputRecord) -> Self {
        Self {
            name: name.into(),
            media_type,
            record,
        }
    }

    /// Builds an untruncated inline text attachment.
    #[must_use]
    pub fn text(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(
            name,
            Some("text/plain".to_owned()),
            OutputRecord::inline(text),
        )
    }

    /// Attachment name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Media type, when known.
    #[must_use]
    pub fn media_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }

    /// Attached payload.
    #[must_use]
    pub const fn record(&self) -> &OutputRecord {
        &self.record
    }
}

/// One scored evaluation outcome plus proposer-readable feedback.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScoredFeedbackEvidence {
    score: ScalarEvidence,
    feedback: String,
    trace: Vec<String>,
    attachments: Vec<FeedbackAttachment>,
}

impl ScoredFeedbackEvidence {
    /// Builds a scored feedback record.
    #[must_use]
    pub fn new(score: ScalarEvidence, feedback: impl Into<String>, trace: Vec<String>) -> Self {
        Self {
            score,
            feedback: feedback.into(),
            trace,
            attachments: Vec::new(),
        }
    }

    /// Attaches named feedback payloads.
    #[must_use]
    pub fn with_attachments(mut self, attachments: Vec<FeedbackAttachment>) -> Self {
        self.attachments = attachments;
        self
    }

    /// Comparable scalar score.
    #[must_use]
    pub const fn score(&self) -> ScalarEvidence {
        self.score
    }

    /// Natural-language feedback attached to the score.
    #[must_use]
    pub fn feedback(&self) -> &str {
        &self.feedback
    }

    /// Execution trace lines captured while producing the score.
    #[must_use]
    pub fn trace(&self) -> &[String] {
        &self.trace
    }

    /// Named payloads attached to this feedback.
    #[must_use]
    pub fn attachments(&self) -> &[FeedbackAttachment] {
        &self.attachments
    }
}

impl Evidence for ScoredFeedbackEvidence {}
