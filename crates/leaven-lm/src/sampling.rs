use leaven_kernel::FiniteF64;
use serde::{Deserialize, Serialize};

/// Provider-neutral sampling and model-control options.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SamplingOptions {
    /// Temperature, when supported by the provider/model.
    pub temperature: Option<FiniteF64>,
    /// Top-p nucleus sampling, when supported by the provider/model.
    pub top_p: Option<FiniteF64>,
    /// Maximum output tokens.
    pub max_output_tokens: Option<u32>,
    /// Determinism seed, when supported by the provider/model.
    pub seed: Option<u64>,
    /// Stop sequences requested by the caller.
    pub stop: Vec<String>,
    /// Reasoning effort, when supported by the provider/model.
    pub reasoning_effort: Option<ReasoningEffort>,
}

impl SamplingOptions {
    /// Sets maximum output tokens.
    #[must_use]
    pub const fn with_max_output_tokens(mut self, max_output_tokens: u32) -> Self {
        self.max_output_tokens = Some(max_output_tokens);
        self
    }

    /// Sets reasoning effort.
    #[must_use]
    pub fn with_reasoning_effort(mut self, effort: ReasoningEffort) -> Self {
        self.reasoning_effort = Some(effort);
        self
    }
}

/// Common reasoning-effort controls.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    /// Disable reasoning effort where providers support that mode.
    None,
    /// Low reasoning effort.
    Low,
    /// Medium/default reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
    /// Extra-high reasoning effort.
    XHigh,
}
