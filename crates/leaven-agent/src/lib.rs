//! leaven-agent crate skeleton.

mod error;
mod runtime;
mod session;
mod transcript;

pub use error::AgentRuntimeError;
pub use runtime::AgentRuntime;
pub use session::{AgentSessionConfig, AgentSessionResult};
pub use transcript::{AgentTranscript, ToolCallRecord};

pub mod prelude {
    pub use crate::{
        AgentRuntime, AgentRuntimeError, AgentSessionConfig, AgentSessionResult, AgentTranscript,
        ToolCallRecord,
    };
}
