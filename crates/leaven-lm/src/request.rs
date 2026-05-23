use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Messages, ModelName, OutputMode, ProviderName, SamplingOptions};

/// Provider transport state for efficient follow-up turns.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct LmContinuation {
    /// Provider family that minted the response ID.
    pub provider: ProviderName,
    /// Provider response ID.
    pub response_id: String,
    /// Number of canonical messages covered by this provider state.
    pub covered_messages: usize,
}

/// Provider-neutral request for one LM completion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LmRequest {
    /// Target model name.
    pub model: ModelName,
    /// Canonical multi-turn text conversation.
    pub messages: Messages,
    /// Sampling and model-control options.
    pub sampling: SamplingOptions,
    /// Requested output shape.
    pub output: OutputMode,
    /// Tool definitions made available to the model.
    pub tools: Vec<LmTool>,
    /// Optional provider continuation state.
    pub continuation: Option<LmContinuation>,
    /// Provider transport hints.
    pub provider_hints: ProviderHints,
}

impl LmRequest {
    /// Builds a text-completion-friendly request.
    pub fn new(model: impl Into<ModelName>, messages: Messages) -> Self {
        Self {
            model: model.into(),
            messages,
            sampling: SamplingOptions::default(),
            output: OutputMode::Text,
            tools: Vec::new(),
            continuation: None,
            provider_hints: ProviderHints::default(),
        }
    }

    /// Sets sampling options.
    #[must_use]
    pub fn with_sampling(mut self, sampling: SamplingOptions) -> Self {
        self.sampling = sampling;
        self
    }

    /// Sets output mode.
    #[must_use]
    pub fn with_output(mut self, output: OutputMode) -> Self {
        self.output = output;
        self
    }

    /// Sets tool definitions.
    #[must_use]
    pub fn with_tools(mut self, tools: impl IntoIterator<Item = LmTool>) -> Self {
        self.tools = tools.into_iter().collect();
        self
    }

    /// Sets provider continuation state.
    #[must_use]
    pub fn with_continuation(mut self, continuation: LmContinuation) -> Self {
        self.continuation = Some(continuation);
        self
    }

    /// Sets provider hints.
    #[must_use]
    pub fn with_provider_hints(mut self, provider_hints: ProviderHints) -> Self {
        self.provider_hints = provider_hints;
        self
    }
}

/// Provider transport hints that are not part of canonical messages.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderHints {
    /// Provider-side prompt-cache routing key.
    pub prompt_cache_key: Option<String>,
    /// Provider-side storage toggle when supported.
    pub store: Option<bool>,
    /// Small provider metadata map.
    pub metadata: BTreeMap<String, String>,
    /// Provider-specific hint values that are intentionally outside the
    /// canonical prompt.
    pub values: BTreeMap<String, Value>,
}

/// Provider-neutral tool definition for LM calls.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LmTool {
    /// Tool name visible to the model.
    pub name: String,
    /// Optional human-readable description.
    pub description: Option<String>,
    /// JSON schema for tool arguments.
    pub input_schema: Value,
    /// Optional public-seam capability action required before the tool may run.
    pub requires_capability_action: Option<String>,
}
