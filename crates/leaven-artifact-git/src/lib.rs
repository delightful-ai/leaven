//! leaven-artifact-git crate skeleton.

pub mod artifact {
    pub struct GitArtifact;
    pub enum GitArtifactIdentityMode {
        Commit,
        Tree,
    }
}
pub mod change {
    pub struct FsOp;
    pub struct GitChange;
}
pub mod diff {
    pub struct GitDiff;
    pub struct GitDiffSummary;
}
pub mod error {
    #[derive(Debug, thiserror::Error)]
    pub enum GitArtifactError {
        #[error("git artifact failed")]
        Message,
    }
}
pub mod surface {
    pub struct GitAgentKitSurface;
    pub struct GitPathSurface;
    pub struct GitSkillFrontmatterSurface;
}
pub use artifact::{GitArtifact, GitArtifactIdentityMode};
pub use change::{FsOp, GitChange};
pub use diff::{GitDiff, GitDiffSummary};
pub use error::GitArtifactError;
pub use surface::{GitAgentKitSurface, GitPathSurface, GitSkillFrontmatterSurface};
