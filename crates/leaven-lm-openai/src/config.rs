use leaven_lm::LmError;

/// Configuration for the `OpenAI` Responses API LM provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenAiConfig {
    api_key: String,
    base_url: String,
}

impl OpenAiConfig {
    /// Creates config with the default `OpenAI` Responses API URL.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.openai.com/v1/responses".to_owned(),
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

    pub(crate) fn api_key(&self) -> &str {
        &self.api_key
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }
}
