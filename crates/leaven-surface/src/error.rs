//! Surface errors.

#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
    #[error("surface part was not found")]
    UnknownPart,
    #[error("surface operation failed: {0}")]
    Message(String),
}
