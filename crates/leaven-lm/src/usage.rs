use leaven_kernel::Cost;
use serde::{Deserialize, Serialize};

/// Provider-reported token accounting for one LM response.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input/prompt tokens.
    pub input_tokens: u64,
    /// Input tokens served from provider-side prompt cache.
    pub cached_input_tokens: u64,
    /// Output/completion tokens.
    pub output_tokens: u64,
    /// Output tokens spent on hidden reasoning, when reported.
    pub reasoning_tokens: u64,
}

impl TokenUsage {
    /// Converts usage into Leaven's metered LM-call cost.
    #[must_use]
    pub fn to_cost(&self) -> Cost {
        Cost {
            llm_calls: 1,
            prompt_tokens: self.input_tokens,
            completion_tokens: self.output_tokens,
            ..Cost::zero()
        }
    }
}
