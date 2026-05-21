//! Git-backed artifact vocabulary and git-specific edit surfaces.

mod artifact;
mod change;
mod diff;
mod error;
mod path;
mod reference;
mod repo_set;
mod surface;

pub use artifact::{GitArtifact, GitArtifactIdentityMode};
pub use change::{GitChange, GitFsOp};
pub use diff::{GitDiff, GitDiffSummary};
pub use error::GitArtifactError;
pub use path::GitPath;
pub use reference::{
    GitLineage, GitObjectId, GitRef, GitRefKey, GitRefKind, GitRefName, GitRefTarget,
};
pub use repo_set::{
    GitRepoArtifact, GitRepoChange, GitRepoSetArtifact, GitRepoSetChange, GitRepoSetLayout,
    GitRevision, GitRevisionKind, RemoteRef, RepoKey, RepoRef, RepoStoreRef,
};
pub use surface::{GitAgentKitSurface, GitPathSurface, GitSkillFrontmatterSurface};
