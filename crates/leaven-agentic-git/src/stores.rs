use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use leaven_artifact_git::RepoKey;

use crate::GitAgenticGitError;

/// Durable Git object stores available to a Git program workload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitProgramStores {
    stores: BTreeMap<RepoKey, PathBuf>,
}

impl GitProgramStores {
    /// Builds a non-empty durable store map keyed by program repo.
    pub fn new(stores: BTreeMap<RepoKey, PathBuf>) -> Result<Self, GitAgenticGitError> {
        if stores.is_empty() {
            return Err(GitAgenticGitError::MissingStores);
        }
        Ok(Self { stores })
    }

    pub(crate) fn store_for(&self, repo: &RepoKey) -> Result<&Path, GitAgenticGitError> {
        self.stores
            .get(repo)
            .map(PathBuf::as_path)
            .ok_or_else(|| GitAgenticGitError::MissingStore { repo: repo.clone() })
    }
}
