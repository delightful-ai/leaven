//! leaven-lm crate skeleton.

pub struct Completion;
pub struct CompletionBatch;
#[derive(Debug, thiserror::Error)]
pub enum LmError {
    #[error("lm failed")]
    Message,
}
pub struct Message;
pub struct Messages;
pub enum Role {
    System,
    User,
    Assistant,
}
pub trait Lm {}
pub struct SamplingOptions;
pub struct TokenUsage;
pub mod prelude {
    pub use crate::{
        Completion, CompletionBatch, Lm, LmError, Message, Messages, Role, SamplingOptions,
        TokenUsage,
    };
}
