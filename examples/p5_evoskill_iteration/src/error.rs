use leaven_agent::AgentRuntimeError;
use leaven_agentic::AgenticAdapterError;
use leaven_engine::{RunContextError, RunPersistenceError};
use leaven_store::StoreError;
use leaven_workspace::{FactoryError, WorkspaceError, WorkspacePathError};

#[derive(Debug, thiserror::Error)]
pub enum ExampleError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    WorkspacePath(#[from] WorkspacePathError),
    #[error(transparent)]
    Factory(#[from] FactoryError),
    #[error(transparent)]
    Runtime(#[from] AgentRuntimeError),
    #[error(transparent)]
    RunContext(#[from] RunContextError),
    #[error(transparent)]
    RunPersistence(#[from] RunPersistenceError),
    #[error(transparent)]
    Agentic(#[from] AgenticAdapterError),
    #[error(transparent)]
    Skill(#[from] leaven_artifact_skill::SkillBankError),
    #[error(transparent)]
    SkillName(#[from] leaven_artifact_skill::SkillNameError),
    #[error(transparent)]
    SkillPath(#[from] leaven_artifact_skill::SkillPathError),
    #[error(transparent)]
    Scalar(#[from] leaven_evidence::ScalarEvidenceError),
    #[error(transparent)]
    Sampler(#[from] leaven_eval::SamplerError),
}

pub type Result<T> = std::result::Result<T, ExampleError>;

pub fn msg(message: impl Into<String>) -> ExampleError {
    ExampleError::Message(message.into())
}
