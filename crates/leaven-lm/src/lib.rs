//! Provider-neutral language-model vocabulary and capability.

mod error;
mod message;
mod model;
mod output;
mod request;
mod response;
mod sampling;
mod usage;

pub use error::{InvalidLmResponse, LmError};
pub use message::{Message, Messages, Role};
pub use model::{Lm, LmId, ModelName, ProviderName};
pub use output::{JsonSchemaOutput, OutputMode};
pub use request::{LmContinuation, LmRequest, ProviderHints};
pub use response::LmResponse;
pub use sampling::{ReasoningEffort, SamplingOptions};
pub use usage::TokenUsage;

pub mod prelude {
    pub use crate::{
        InvalidLmResponse, JsonSchemaOutput, Lm, LmContinuation, LmError, LmId, LmRequest,
        LmResponse, Message, Messages, ModelName, OutputMode, ProviderHints, ProviderName,
        ReasoningEffort, Role, SamplingOptions, TokenUsage,
    };
}
