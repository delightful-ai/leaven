use leaven_kernel::{Fingerprint, FingerprintBuilder};

/// Script for deterministic mock LM responses.
#[derive(Clone, Debug, Default)]
pub struct MockLmScript {
    steps: Vec<MockLmStep>,
}

impl MockLmScript {
    /// Creates an empty script.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a text response with token accounting.
    #[must_use]
    pub fn then_text(
        mut self,
        text: impl Into<String>,
        input_tokens: u64,
        output_tokens: u64,
    ) -> Self {
        self.steps.push(MockLmStep::Text {
            text: text.into(),
            input_tokens,
            output_tokens,
        });
        self
    }

    /// Consumes this script into response steps.
    pub(crate) fn into_steps(self) -> VecDeque<MockLmStep> {
        self.steps.into()
    }

    /// Returns the script behavior fingerprint.
    pub(crate) fn fingerprint(&self) -> Fingerprint {
        let mut builder = FingerprintBuilder::new();
        builder.update(b"leaven-lm-mock-v1");
        for step in &self.steps {
            match step {
                MockLmStep::Text {
                    text,
                    input_tokens,
                    output_tokens,
                } => {
                    builder.update(b"text");
                    builder.update(text.as_bytes());
                    builder.update(input_tokens.to_le_bytes());
                    builder.update(output_tokens.to_le_bytes());
                }
            }
        }
        builder.finish()
    }
}

use std::collections::VecDeque;

/// One deterministic mock response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MockLmStep {
    Text {
        text: String,
        input_tokens: u64,
        output_tokens: u64,
    },
}
