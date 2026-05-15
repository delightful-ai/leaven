use std::time::Duration;

use leaven_lm::LmError;

/// Configuration for the `OpenAI` Responses API LM provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenAiConfig {
    api_key: String,
    base_url: String,
    request_timeout: Duration,
    retry_policy: OpenAiRetryPolicy,
}

impl OpenAiConfig {
    /// Creates config with the default `OpenAI` Responses API URL.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.openai.com/v1/responses".to_owned(),
            request_timeout: Duration::from_secs(120),
            retry_policy: OpenAiRetryPolicy::default(),
        }
    }

    /// Reads `OPENAI_API_KEY` from the process environment.
    ///
    /// # Errors
    ///
    /// Returns [`LmError::InvalidRequest`] when the key is missing.
    pub fn from_env() -> Result<Self, LmError> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| LmError::invalid_request("OPENAI_API_KEY is not set"))?;
        Ok(Self::new(api_key))
    }

    /// Overrides the Responses API endpoint.
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Overrides the per-request transport timeout.
    #[must_use]
    pub const fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Overrides retry behavior for retryable transport and provider failures.
    #[must_use]
    pub const fn with_retry_policy(mut self, retry_policy: OpenAiRetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    pub(crate) fn api_key(&self) -> &str {
        &self.api_key
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) const fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    pub(crate) const fn retry_policy(&self) -> &OpenAiRetryPolicy {
        &self.retry_policy
    }
}

/// Bounded retry policy for `OpenAI` Responses API requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenAiRetryPolicy {
    max_retries: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl OpenAiRetryPolicy {
    /// Creates an explicit retry policy.
    #[must_use]
    pub const fn new(max_retries: u32, initial_backoff: Duration, max_backoff: Duration) -> Self {
        Self {
            max_retries,
            initial_backoff,
            max_backoff,
        }
    }

    /// Disables retries.
    #[must_use]
    pub const fn none() -> Self {
        Self::new(0, Duration::ZERO, Duration::ZERO)
    }

    pub(crate) const fn max_retries(&self) -> u32 {
        self.max_retries
    }

    pub(crate) const fn initial_backoff(&self) -> Duration {
        self.initial_backoff
    }

    pub(crate) const fn max_backoff(&self) -> Duration {
        self.max_backoff
    }
}

impl Default for OpenAiRetryPolicy {
    fn default() -> Self {
        Self::new(3, Duration::from_millis(250), Duration::from_secs(5))
    }
}
