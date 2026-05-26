use leaven_artifact_git::{GitArtifactError, RepoKey};
use leaven_workspace::WorkspacePathError;
use leaven_workspace_git::GitWorkspaceGitError;

/// Errors from Git program materialization, readback, and durable import.
#[derive(Debug, thiserror::Error)]
pub enum GitAgenticGitError {
    #[error("git program store map is empty")]
    MissingStores,
    #[error("missing durable git store for repo `{repo}`")]
    MissingStore { repo: RepoKey },
    #[error("missing git program layout for repo `{repo}`")]
    MissingLayout { repo: RepoKey },
    #[error("git program materialization supports commit revisions only for repo `{repo}`")]
    NonCommitMaterialization { repo: RepoKey },
    #[error("git program readback supports commit revisions only for repo `{repo}`")]
    NonCommitReadback { repo: RepoKey },
    #[error("git bundle `{path}` does not contain a head")]
    EmptyBundle { path: String },
    #[error(transparent)]
    Workspace(#[from] leaven_workspace::WorkspaceError),
    #[error(transparent)]
    WorkspacePath(#[from] WorkspacePathError),
    #[error(transparent)]
    Git(#[from] GitWorkspaceGitError),
    #[error(transparent)]
    GitArtifact(#[from] GitArtifactError),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
}
