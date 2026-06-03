use std::future::Future;
use std::time::Duration;

use leaven_kernel::{Fingerprint, Metered};
use leaven_lm::{Lm, LmError, LmId, LmRequest, LmResponse};
use leaven_lm_mock::{MockLm, MockLmScript};
use leaven_lm_openai::{OpenAiConfig, OpenAiLm, OpenAiRetryPolicy};
use serde::{Deserialize, Serialize};

/// Configured LM provider for public-seam execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SeamLmConfig {
    /// Deterministic local LM script. This is mechanics evidence, not live
    /// provider proof.
    Mock {
        /// Responses consumed in order by executed `lm_complete` calls.
        responses: Vec<MockLmResponseConfig>,
    },
    /// Live OpenAI Responses API provider.
    OpenAi {
        /// Environment variable that carries the OpenAI API key.
        api_key_env: String,
        /// Optional OpenAI-compatible Responses API endpoint.
        base_url: Option<String>,
        /// Optional request timeout in seconds.
        timeout_s: Option<u64>,
        /// Optional maximum retry count for retryable provider/transport failures.
        max_retries: Option<u32>,
    },
}

impl SeamLmConfig {
    pub(crate) fn validate(&self) -> Result<(), ConfiguredLmError> {
        match self {
            Self::Mock { responses } if responses.is_empty() => {
                Err(ConfiguredLmError::EmptyMockLmScript)
            }
            Self::Mock { .. } => Ok(()),
            Self::OpenAi {
                api_key_env,
                timeout_s,
                ..
            } => {
                if api_key_env.is_empty() {
                    return Err(ConfiguredLmError::InvalidConfig(
                        "OpenAI LM config api_key_env must not be empty".to_owned(),
                    ));
                }
                if matches!(timeout_s, Some(0)) {
                    return Err(ConfiguredLmError::InvalidConfig(
                        "OpenAI LM config timeout_s must be positive".to_owned(),
                    ));
                }
                Ok(())
            }
        }
    }

    pub(crate) fn to_lm_runtime(&self) -> Result<ConfiguredLmRuntime, LmError> {
        match self {
            Self::Mock { responses } => {
                let script = responses.iter().fold(MockLmScript::new(), |script, response| {
                    script.then_text(
                        response.text.clone(),
                        response.input_tokens,
                        response.output_tokens,
                    )
                });
                Ok(ConfiguredLmRuntime::Mock(MockLm::new(script)))
            }
            Self::OpenAi {
                api_key_env,
                base_url,
                timeout_s,
                max_retries,
            } => {
                let api_key = std::env::var(api_key_env).map_err(|_| {
                    LmError::invalid_request(format!("{api_key_env} is not set"))
                })?;
                let mut config = OpenAiConfig::new(api_key);
                if let Some(base_url) = base_url {
                    config = config.with_base_url(base_url.clone());
                }
                if let Some(timeout_s) = timeout_s {
                    config = config.with_request_timeout(Duration::from_secs(*timeout_s));
                }
                if let Some(max_retries) = max_retries {
                    config = config.with_retry_policy(OpenAiRetryPolicy::new(
                        *max_retries,
                        Duration::from_millis(250),
                        Duration::from_secs(5),
                    ));
                }
                Ok(ConfiguredLmRuntime::OpenAi(OpenAiLm::new(config)))
            }
        }
    }
}

impl Default for SeamLmConfig {
    fn default() -> Self {
        Self::Mock {
            responses: vec![MockLmResponseConfig::default()],
        }
    }
}

/// One deterministic mock LM response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MockLmResponseConfig {
    /// Assistant text returned by the mock LM.
    pub text: String,
    /// Input-token count charged by the response.
    pub input_tokens: u64,
    /// Output-token count charged by the response.
    pub output_tokens: u64,
}

impl Default for MockLmResponseConfig {
    fn default() -> Self {
        Self {
            text: "ok".to_owned(),
            input_tokens: 1,
            output_tokens: 1,
        }
    }
}

/// Executable LM runtime selected by a seam service config.
pub(crate) enum ConfiguredLmRuntime {
    Mock(MockLm),
    OpenAi(OpenAiLm),
}

impl Lm for ConfiguredLmRuntime {
    fn id(&self) -> LmId {
        match self {
            Self::Mock(lm) => lm.id(),
            Self::OpenAi(lm) => lm.id(),
        }
    }

    fn fingerprint(&self) -> Fingerprint {
        match self {
            Self::Mock(lm) => lm.fingerprint(),
            Self::OpenAi(lm) => lm.fingerprint(),
        }
    }

    fn complete(
        &self,
        request: LmRequest,
    ) -> impl Future<Output = Result<Metered<LmResponse>, LmError>> + Send + '_ {
        async move {
            match self {
                Self::Mock(lm) => lm.complete(request).await,
                Self::OpenAi(lm) => lm.complete(request).await,
            }
        }
    }
}

/// Error while validating configured LM providers.
#[derive(Debug, thiserror::Error)]
pub enum ConfiguredLmError {
    /// A mock LM must include at least one response.
    #[error("mock LM config must include at least one response")]
    EmptyMockLmScript,
    /// A configured LM provider has invalid local configuration.
    #[error("{0}")]
    InvalidConfig(String),
}
