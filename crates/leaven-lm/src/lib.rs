//! leaven-lm crate skeleton.

mod completion;
mod error;
mod message;
mod model;
mod sampling;
mod usage;

pub use completion::{Completion, CompletionBatch};
pub use error::LmError;
pub use message::{Message, Messages, Role};
pub use model::Lm;
pub use sampling::SamplingOptions;
pub use usage::TokenUsage;

pub mod prelude {
    pub use crate::{
        Completion, CompletionBatch, Lm, LmError, Message, Messages, Role, SamplingOptions,
        TokenUsage,
    };
}
