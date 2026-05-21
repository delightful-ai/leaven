use crate::{GitPath, GitRefKey, GitRevision, RepoKey};

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
    #[error("invalid repo key `{key}`: {reason}")]
    InvalidRepoKey { key: String, reason: &'static str },
    #[error("git repo set artifact must contain at least one repo")]
    EmptyRepoSet,
    #[error("repo artifact key `{repo}` does not match map key `{expected}`")]
    RepoKeyMismatch { expected: RepoKey, repo: RepoKey },
    #[error("missing layout entry for repo `{repo}`")]
    MissingRepoLayout { repo: RepoKey },
    #[error("layout references unknown repo `{repo}`")]
    UnknownRepoLayout { repo: RepoKey },
    #[error("missing repo `{repo}`")]
    MissingRepo { repo: RepoKey },
    #[error("repo `{repo}` expected parent `{expected_parent}` but found `{actual_parent}`")]
    RevisionParentMismatch {
        repo: RepoKey,
        expected_parent: GitRevision,
        actual_parent: GitRevision,
    },
}
