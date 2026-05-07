#[derive(Debug, thiserror::Error)]
pub enum AgentRuntimeError {
    #[error("agent runtime failed")]
    Message,
}
