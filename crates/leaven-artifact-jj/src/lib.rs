//! leaven-artifact-jj crate skeleton.

pub mod artifact {
    pub struct JjArtifact;
    pub enum JjArtifactIdentityMode {
        Change,
        Commit,
    }
}
pub mod change {
    pub struct JjChange;
    pub struct JjOp;
}
pub mod conflict {
    pub struct ConflictRegion;
    pub struct ConflictRegionId;
}
pub mod error {
    #[derive(Debug, thiserror::Error)]
    pub enum JjArtifactError {
        #[error("jj artifact failed")]
        Message,
    }
}
pub mod operation_log {
    pub struct OperationId;
    pub struct OperationSummary;
}
pub mod surface {
    pub struct JjChangesetSurface;
    pub struct JjConflictSurface;
    pub struct JjPathSurface;
}
pub use artifact::{JjArtifact, JjArtifactIdentityMode};
pub use change::{JjChange, JjOp};
pub use conflict::{ConflictRegion, ConflictRegionId};
pub use error::JjArtifactError;
pub use operation_log::{OperationId, OperationSummary};
pub use surface::{JjChangesetSurface, JjConflictSurface, JjPathSurface};
