use serde::{Deserialize, Serialize};

use crate::{InvalidLmResponse, LmContinuation, Message, Role, TokenUsage};

/// Provider-neutral response to one LM completion request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LmResponse {
    /// Assistant message produced by the model.
    pub assistant: Message,
    /// Optional provider continuation state.
    pub continuation: Option<LmContinuation>,
    /// Provider-reported usage.
    pub usage: TokenUsage,
    /// Raw provider response ID for diagnostics and follow-up calls.
    pub provider_response_id: Option<String>,
}

impl LmResponse {
    /// Builds a response around an assistant-authored message.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidLmResponse::NonAssistantMessage`] if the supplied
    /// message does not have [`Role::Assistant`].
    pub fn new(assistant: Message, usage: TokenUsage) -> Result<Self, InvalidLmResponse> {
        if assistant.role() != Role::Assistant {
            return Err(InvalidLmResponse::NonAssistantMessage);
        }
        Ok(Self {
            assistant,
            continuation: None,
            usage,
            provider_response_id: None,
        })
    }

    /// Sets provider continuation state.
    #[must_use]
    pub fn with_continuation(mut self, continuation: LmContinuation) -> Self {
        self.continuation = Some(continuation);
        self
    }

    /// Sets provider response ID.
    #[must_use]
    pub fn with_provider_response_id(mut self, response_id: impl Into<String>) -> Self {
        self.provider_response_id = Some(response_id.into());
        self
    }
}
