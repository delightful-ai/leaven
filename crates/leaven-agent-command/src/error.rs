use leaven_agent::AgentRuntimeError;
use leaven_workspace::WorkspaceError;

#[derive(Debug, thiserror::Error)]
pub enum CommandAgentError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),

    #[error("command session parser failed: {0}")]
    Parse(String),
}

impl From<CommandAgentError> for AgentRuntimeError {
    fn from(error: CommandAgentError) -> Self {
        match error {
            CommandAgentError::Workspace(error) => Self::Workspace(error),
            CommandAgentError::Parse(message) => Self::Message(message),
        }
    }
}
