//! Git-backed artifact vocabulary.

mod artifact;
mod change;
mod diff;
mod error;
mod path;
mod program;
mod reference;

pub use artifact::{GitArtifact, GitArtifactIdentityMode};
pub use change::{GitChange, GitFsOp};
pub use diff::{GitDiff, GitDiffSummary};
pub use error::GitArtifactError;
pub use path::GitPath;
pub use program::{
    GitProgramArtifact, GitProgramChange, GitProgramLayout, GitRepoArtifact, GitRepoChange,
    GitRevision, GitRevisionKind, RemoteRef, RepoKey, RepoRef, RepoStoreRef,
};
pub use reference::{
    GitLineage, GitObjectId, GitRef, GitRefKey, GitRefKind, GitRefName, GitRefTarget,
};
