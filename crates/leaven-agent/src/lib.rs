//! leaven-agent crate skeleton.

#[derive(Debug, thiserror::Error)]
pub enum AgentRuntimeError {
    #[error("agent runtime failed")]
    Message,
}
pub trait AgentRuntime {}
pub struct AgentSessionConfig;
pub struct AgentSessionResult;
pub struct AgentTranscript;
pub struct ToolCallRecord;
pub mod prelude {
    pub use crate::{
        AgentRuntime, AgentRuntimeError, AgentSessionConfig, AgentSessionResult, AgentTranscript,
        ToolCallRecord,
    };
}
