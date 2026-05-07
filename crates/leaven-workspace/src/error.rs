//! Workspace errors.

#[derive(Debug, thiserror::Error)]
pub enum FactoryError {
    #[error("workspace allocation failed: {0}")]
    Allocate(String),
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("workspace command failed: {0}")]
    Command(String),
    #[error("workspace io failed: {0}")]
    Io(String),
    #[error("workspace cleanup failed: {0}")]
    Cleanup(String),
    #[error(transparent)]
    Path(#[from] WorkspacePathError),
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
}
