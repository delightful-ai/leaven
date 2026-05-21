#[derive(Debug, thiserror::Error)]
pub enum GitWorkspaceGitError {
    #[error("git command `{program}` failed with status {status:?}: {stderr}")]
    Command {
        program: &'static str,
        status: Option<i32>,
        stderr: String,
    },
    #[error("git command `{program}` failed to start: {source}")]
    CommandIo {
        program: &'static str,
        source: std::io::Error,
    },
    #[error("git checkout io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("git output is not utf-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    Artifact(#[from] leaven_artifact_git::GitArtifactError),
    #[error("git ref output line is malformed: {0}")]
    MalformedRefLine(String),
}
