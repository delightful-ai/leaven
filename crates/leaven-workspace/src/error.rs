//! Workspace errors.

use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum FactoryError {
    #[error("workspace allocation failed: {0}")]
    Allocate(String),
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("workspace command failed: {0}")]
    Command(String),
    #[error("workspace command `{program}` timed out after {timeout:?}")]
    CommandTimedOut { program: String, timeout: Duration },
    #[error("workspace io failed: {0}")]
    Io(String),
    #[error("workspace cleanup failed: {0}")]
    Cleanup(String),
    #[error("workspace operation is not supported by this backend: {operation}")]
    UnsupportedOperation { operation: &'static str },
    #[error(transparent)]
    Path(#[from] WorkspacePathError),
}

#[derive(Debug, thiserror::Error)]
pub enum WithWorkspaceError<E> {
    #[error(transparent)]
    Allocate(#[from] FactoryError),
    #[error(transparent)]
    Stage(E),
    #[error(transparent)]
    Cleanup(WorkspaceError),
    #[error("workspace stage and cleanup both failed")]
    StageAndCleanup { stage: E, cleanup: WorkspaceError },
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum WorkspacePathError {
    #[error("workspace path cannot be empty")]
    Empty,
    #[error("workspace path must be relative: {0}")]
    Absolute(String),
    #[error("workspace path escapes the workspace: {0}")]
    ParentTraversal(String),
    #[error("workspace path contains an empty component: {0}")]
    EmptyComponent(String),
    #[error("workspace path {path} is outside view prefix {prefix}")]
    OutsideView { path: String, prefix: String },
}
