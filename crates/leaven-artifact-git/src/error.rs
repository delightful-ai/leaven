use crate::{GitPath, GitRefKey};

#[derive(Debug, thiserror::Error)]
pub enum GitArtifactError {
    #[error("invalid git path `{path}`: {reason}")]
    InvalidPath { path: String, reason: &'static str },
    #[error("missing git path `{path}`")]
    MissingPath { path: GitPath },
    #[error("invalid git ref name `{name}`: {reason}")]
    InvalidRefName { name: String, reason: &'static str },
    #[error("invalid git object id `{id}`")]
    InvalidObjectId { id: String },
    #[error("missing git ref `{key}`")]
    MissingRef { key: GitRefKey },
}
