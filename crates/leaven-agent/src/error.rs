use std::error::Error;

use leaven_kernel::AgentRuntimeId;
use leaven_workspace::WorkspaceError;

#[derive(Debug, thiserror::Error)]
pub enum AgentRuntimeError {
    #[error("agent runtime {runtime} requires a local workspace mount")]
    LocalMountRequired { runtime: AgentRuntimeId },

    #[error("agent runtime output contract failed: {0}")]
    OutputContract(String),

    #[error("agent runtime policy violation: {0}")]
    Policy(String),

    #[error(transparent)]
    Workspace(#[from] WorkspaceError),

    #[error("agent runtime failed: {0}")]
    Message(String),

    #[error("agent runtime failed: {message}")]
    WithSource {
        message: String,
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
}

impl AgentRuntimeError {
    pub fn with_source(
        message: impl Into<String>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::WithSource {
            message: message.into(),
            source: Box::new(source),
        }
    }
}
