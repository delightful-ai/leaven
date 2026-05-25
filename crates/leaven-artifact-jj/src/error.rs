#[derive(Debug, thiserror::Error)]
pub enum JjArtifactError {
    #[error("jj artifact failed")]
    Message,
}
