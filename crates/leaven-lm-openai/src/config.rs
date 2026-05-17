use std::{num::NonZeroUsize, time::Duration};

use leaven_lm::LmError;

/// Configuration for the `OpenAI` Responses API LM provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenAiConfig {
    api_key: String,
    base_url: String,
    request_timeout: Duration,
    retry_policy: OpenAiRetryPolicy,
    throttle_policy: OpenAiThrottlePolicy,
}

impl OpenAiConfig {
    const REQUEST_TIMEOUT_ENV: &'static str = "LEAVEN_OPENAI_REQUEST_TIMEOUT_SECONDS";

    /// Creates config with the default `OpenAI` Responses API URL.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.openai.com/v1/responses".to_owned(),
            request_timeout: Duration::from_secs(120),
            retry_policy: OpenAiRetryPolicy::default(),
            throttle_policy: OpenAiThrottlePolicy::default(),
        }
    }

    /// Reads `OPENAI_API_KEY` from the process environment.
    ///
    /// `LEAVEN_OPENAI_REQUEST_TIMEOUT_SECONDS` may override the default
    /// per-request transport timeout for long-running release calls.
    ///
    /// # Errors
    ///
    /// Returns [`LmError::InvalidRequest`] when the key is missing.
    pub fn from_env() -> Result<Self, LmError> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| LmError::invalid_request("OPENAI_API_KEY is not set"))?;
        let mut config = Self::new(api_key);
        if let Some(timeout) = request_timeout_from_env()? {
            config = config.with_request_timeout(timeout);
        }
        Ok(config)
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

    /// Overrides proactive concurrency throttling for provider calls.
    #[must_use]
    pub const fn with_throttle_policy(mut self, throttle_policy: OpenAiThrottlePolicy) -> Self {
        self.throttle_policy = throttle_policy;
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

    pub(crate) const fn throttle_policy(&self) -> &OpenAiThrottlePolicy {
        &self.throttle_policy
    }
}

fn request_timeout_from_env() -> Result<Option<Duration>, LmError> {
    let Some(raw) = std::env::var_os(OpenAiConfig::REQUEST_TIMEOUT_ENV) else {
        return Ok(None);
    };
    let raw = raw.to_string_lossy();
    let seconds = raw.parse::<u64>().map_err(|_| {
        LmError::invalid_request(format!(
            "{} must be a positive integer number of seconds",
            OpenAiConfig::REQUEST_TIMEOUT_ENV
        ))
    })?;
    if seconds == 0 {
        return Err(LmError::invalid_request(format!(
            "{} must be a positive integer number of seconds",
            OpenAiConfig::REQUEST_TIMEOUT_ENV
        )));
    }
    Ok(Some(Duration::from_secs(seconds)))
}

/// Proactive concurrency throttle for `OpenAI` Responses API requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenAiThrottlePolicy {
    max_concurrent_requests: NonZeroUsize,
    acquire_timeout: Duration,
}

impl OpenAiThrottlePolicy {
    /// Creates an explicit provider-call concurrency policy.
    #[must_use]
    pub const fn new(max_concurrent_requests: NonZeroUsize, acquire_timeout: Duration) -> Self {
        Self {
            max_concurrent_requests,
            acquire_timeout,
        }
    }

    pub(crate) const fn max_concurrent_requests(&self) -> NonZeroUsize {
        self.max_concurrent_requests
    }

    pub(crate) const fn acquire_timeout(&self) -> Duration {
        self.acquire_timeout
    }
}

impl Default for OpenAiThrottlePolicy {
    fn default() -> Self {
        Self::new(
            NonZeroUsize::new(32).expect("default OpenAI concurrency is non-zero"),
            Duration::ZERO,
        )
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
