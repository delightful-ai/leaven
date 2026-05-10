use std::collections::VecDeque;
use std::sync::Arc;

use leaven_kernel::{Fingerprint, FingerprintBuilder, Metered};
use leaven_lm::{Lm, LmError, LmId, LmRequest, LmResponse, Message, TokenUsage};
use parking_lot::Mutex;

use crate::script::{MockLmScript, MockLmStep};

/// Deterministic scripted LM for tests and examples.
#[derive(Clone)]
pub struct MockLm {
    script: Arc<Mutex<VecDeque<MockLmStep>>>,
    fingerprint: Fingerprint,
}

impl MockLm {
    /// Creates a mock LM from a script.
    #[must_use]
    pub fn new(script: MockLmScript) -> Self {
        let fingerprint = script.fingerprint();
        Self {
            script: Arc::new(Mutex::new(script.into_steps())),
            fingerprint,
        }
    }
}

impl Lm for MockLm {
    fn id(&self) -> LmId {
        LmId::new("mock")
    }

    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    async fn complete(&self, _request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
        let step = self
            .script
            .lock()
            .pop_front()
            .ok_or_else(|| LmError::provider("mock", None, "mock script exhausted"))?;
        match step {
            MockLmStep::Text {
                text,
                input_tokens,
                output_tokens,
            } => {
                let usage = TokenUsage {
                    input_tokens,
                    cached_input_tokens: 0,
                    output_tokens,
                    reasoning_tokens: 0,
                };
                let response = LmResponse::new(Message::assistant(text), usage.clone())
                    .map_err(|error| LmError::invalid_response("mock", error.to_string()))?;
                Ok(Metered::new(response, usage.to_cost()))
            }
        }
    }
}

impl Default for MockLm {
    fn default() -> Self {
        Self::new(MockLmScript::default())
    }
}

pub fn fingerprint_steps(steps: &[MockLmStep]) -> Fingerprint {
    let mut builder = FingerprintBuilder::new();
    builder.update(b"leaven-lm-mock-v1");
    for step in steps {
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
