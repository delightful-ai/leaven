use leaven_kernel::MetadataBag;
use leaven_workspace::{WorkspaceError, WorkspacePath, WorkspacePathError};

use crate::OutputEntryId;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub code: Option<String>,
    pub metadata: MetadataBag,
}

impl Diagnostic {
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            code: None,
            metadata: MetadataBag::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, thiserror::Error)]
pub enum StageOutputContractError {
    #[error("stage output contract must require at least one output")]
    NoRequiredOutputs,
    #[error("invalid output entry id `{0}`")]
    InvalidEntryId(String),
    #[error("invalid output role `{0}`")]
    InvalidOutputRole(String),
    #[error("output `{id:?}` path `{path}` is invalid: {source}")]
    InvalidOutputPath {
        id: OutputEntryId,
        path: WorkspacePath,
        #[source]
        source: WorkspacePathError,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceSetupError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    WorkspacePath(#[from] WorkspacePathError),
    #[error(transparent)]
    OutputContract(#[from] StageOutputContractError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("workspace setup failed: {0:?}")]
    Diagnostic(Diagnostic),
}

#[derive(Debug, thiserror::Error)]
pub enum StageOutputParseError {
    #[error("required output `{entry:?}` missing at `{path}`")]
    MissingRequiredOutput {
        entry: OutputEntryId,
        path: WorkspacePath,
    },
    #[error("malformed output at `{path}`: {diagnostic:?}")]
    Malformed {
        path: WorkspacePath,
        diagnostic: Diagnostic,
    },
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum StageBootstrapError {
    #[error("stage bootstrap failed: {0:?}")]
    Diagnostic(Diagnostic),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum StageQueryError {
    #[error("query policy denied request")]
    PolicyDenied,
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
}

#[derive(Debug, thiserror::Error)]
pub enum StageReadError {
    #[error("stage read failed: {0:?}")]
    Diagnostic(Diagnostic),
}

pub use StageBootstrapError as BootstrapError;
