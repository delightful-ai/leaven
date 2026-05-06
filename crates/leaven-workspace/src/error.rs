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
}
