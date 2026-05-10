use std::error::Error;

#[derive(Debug, thiserror::Error)]
pub enum LmError {
    /// The request is invalid before provider transport begins.
    #[error("invalid lm request: {reason}")]
    InvalidRequest {
        /// Why the request was refused.
        reason: String,
    },
    /// Provider returned malformed or semantically incomplete data.
    #[error("invalid {provider} lm response: {reason}")]
    InvalidResponse {
        /// Provider family that produced the response.
        provider: String,
        /// Why the response could not be lowered.
        reason: String,
    },
    /// Provider completed the HTTP/API request with a failure response.
    #[error("{provider} lm provider failed{status_text}: {message}", status_text = status.map(|s| format!(" with status {s}")).unwrap_or_default())]
    Provider {
        /// Provider family.
        provider: String,
        /// HTTP status or provider status when available.
        status: Option<u16>,
        /// Provider failure body or message.
        message: String,
    },
    /// Transport failed before a provider response was available.
    #[error("{provider} lm transport failed")]
    Transport {
        /// Provider family.
        provider: String,
        /// Transport source.
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
    /// Response-cache operation failed.
    #[error("lm response cache failed: {message}")]
    Cache {
        /// Cache error message.
        message: String,
    },
}

impl LmError {
    /// Builds an invalid-request error.
    pub fn invalid_request(reason: impl Into<String>) -> Self {
        Self::InvalidRequest {
            reason: reason.into(),
        }
    }

    /// Builds an invalid-response error.
    pub fn invalid_response(provider: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidResponse {
            provider: provider.into(),
            reason: reason.into(),
        }
    }

    /// Builds a provider error.
    pub fn provider(
        provider: impl Into<String>,
        status: Option<u16>,
        message: impl Into<String>,
    ) -> Self {
        Self::Provider {
            provider: provider.into(),
            status,
            message: message.into(),
        }
    }

    /// Builds a transport error.
    pub fn transport(
        provider: impl Into<String>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::Transport {
            provider: provider.into(),
            source: Box::new(source),
        }
    }

    /// Builds a cache error.
    pub fn cache(message: impl Into<String>) -> Self {
        Self::Cache {
            message: message.into(),
        }
    }
}

/// Error returned when constructing an [`LmResponse`](crate::LmResponse).
#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum InvalidLmResponse {
    /// The response message was not authored by the assistant.
    #[error("lm response message must have assistant role")]
    NonAssistantMessage,
}
