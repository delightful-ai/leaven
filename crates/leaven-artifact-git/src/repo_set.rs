use std::collections::BTreeMap;
use std::fmt;

use leaven_core::{Artifact, ArtifactIdentity, CacheIdentity};
use leaven_kernel::{ContentId, FingerprintBuilder, RunId};

use crate::{GitArtifactError, GitArtifactIdentityMode, GitObjectId, GitPath};

#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct RepoKey(String);

impl RepoKey {
    pub fn new(key: impl Into<String>) -> Result<Self, GitArtifactError> {
        let key = key.into();
        validate_repo_key(&key)?;
        Ok(Self(key))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        feed_field(builder, self.0.as_bytes());
    }
}

impl fmt::Display for RepoKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RepoStoreRef {
    RunScoped {
        run_id: RunId,
        repo_key: RepoKey,
    },
    Global {
        repo_key: RepoKey,
    },
    Remote {
        remote: RemoteRef,
        repo_key: RepoKey,
    },
}

impl RepoStoreRef {
    #[must_use]
    pub const fn repo_key(&self) -> &RepoKey {
        match self {
            Self::RunScoped { repo_key, .. }
            | Self::Global { repo_key }
            | Self::Remote { repo_key, .. } => repo_key,
        }
    }

    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        match self {
            Self::RunScoped { run_id, repo_key } => {
                builder.update(b"run-scoped");
                feed_field(builder, run_id.to_string().as_bytes());
                repo_key.feed_fingerprint(builder);
            }
            Self::Global { repo_key } => {
                builder.update(b"global");
                repo_key.feed_fingerprint(builder);
            }
            Self::Remote { remote, repo_key } => {
                builder.update(b"remote");
                remote.feed_fingerprint(builder);
                repo_key.feed_fingerprint(builder);
            }
        }
    }
}

#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct RemoteRef(String);

impl RemoteRef {
    pub fn new(remote: impl Into<String>) -> Result<Self, GitArtifactError> {
        let remote = remote.into();
        if remote.is_empty() {
            return Err(GitArtifactError::InvalidRepoKey {
                key: remote,
                reason: "remote ref is empty",
            });
        }
        if remote.contains('\0') {
            return Err(GitArtifactError::InvalidRepoKey {
                key: remote,
                reason: "remote ref contains nul byte",
            });
        }
        Ok(Self(remote))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        feed_field(builder, self.0.as_bytes());
    }
}

impl fmt::Display for RemoteRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RepoRef {
    key: RepoKey,
    store: RepoStoreRef,
}

impl RepoRef {
    #[must_use]
    pub fn global(key: RepoKey) -> Self {
        Self {
            store: RepoStoreRef::Global {
                repo_key: key.clone(),
            },
            key,
        }
    }

    #[must_use]
    pub fn run_scoped(run_id: RunId, key: RepoKey) -> Self {
        Self {
            store: RepoStoreRef::RunScoped {
                run_id,
                repo_key: key.clone(),
            },
            key,
        }
    }

    #[must_use]
    pub fn remote(remote: RemoteRef, key: RepoKey) -> Self {
        Self {
            store: RepoStoreRef::Remote {
                remote,
                repo_key: key.clone(),
            },
            key,
        }
    }

    #[must_use]
    pub const fn key(&self) -> &RepoKey {
        &self.key
    }

    #[must_use]
    pub const fn store(&self) -> &RepoStoreRef {
        &self.store
    }

    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        self.key.feed_fingerprint(builder);
        self.store.feed_fingerprint(builder);
    }
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum GitRevisionKind {
    Commit,
    Tree,
}

impl GitRevisionKind {
    const fn fingerprint_byte(self) -> u8 {
        match self {
            Self::Commit => b'c',
            Self::Tree => b't',
        }
    }
}

#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum GitRevision {
    Commit(GitObjectId),
    Tree(GitObjectId),
}

impl GitRevision {
    pub fn commit(id: impl Into<String>) -> Result<Self, GitArtifactError> {
        Ok(Self::Commit(GitObjectId::new(id)?))
    }

    pub fn tree(id: impl Into<String>) -> Result<Self, GitArtifactError> {
        Ok(Self::Tree(GitObjectId::new(id)?))
    }

    #[must_use]
    pub const fn kind(&self) -> GitRevisionKind {
        match self {
            Self::Commit(_) => GitRevisionKind::Commit,
            Self::Tree(_) => GitRevisionKind::Tree,
        }
    }

    #[must_use]
    pub const fn object_id(&self) -> &GitObjectId {
        match self {
            Self::Commit(id) | Self::Tree(id) => id,
        }
    }

    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        builder.update([self.kind().fingerprint_byte()]);
        feed_field(builder, self.object_id().as_str().as_bytes());
    }
}

impl fmt::Display for GitRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Commit(id) => write!(f, "commit:{id}"),
            Self::Tree(id) => write!(f, "tree:{id}"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GitRepoArtifact {
    repo: RepoRef,
    revision: GitRevision,
    subpath: Option<GitPath>,
    identity_mode: GitArtifactIdentityMode,
}

impl GitRepoArtifact {
    #[must_use]
    pub const fn new(
        repo: RepoRef,
        revision: GitRevision,
        subpath: Option<GitPath>,
        identity_mode: GitArtifactIdentityMode,
    ) -> Self {
        Self {
            repo,
            revision,
            subpath,
            identity_mode,
        }
    }

    #[must_use]
    pub const fn repo(&self) -> &RepoRef {
        &self.repo
    }

    #[must_use]
    pub const fn revision(&self) -> &GitRevision {
        &self.revision
    }

    #[must_use]
    pub const fn subpath(&self) -> Option<&GitPath> {
        self.subpath.as_ref()
    }

    #[must_use]
    pub const fn identity_mode(&self) -> GitArtifactIdentityMode {
        self.identity_mode
    }

    fn advance_to(&mut self, child: GitRevision) {
        self.revision = child;
    }

    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        self.repo.feed_fingerprint(builder);
        self.revision.feed_fingerprint(builder);
        match &self.subpath {
            Some(subpath) => {
                builder.update(b"subpath");
                feed_field(builder, subpath.as_str().as_bytes());
            }
            None => {
                builder.update(b"no-subpath");
            }
        }
        match self.identity_mode {
            GitArtifactIdentityMode::Commit => builder.update(b"identity-commit"),
            GitArtifactIdentityMode::Tree => builder.update(b"identity-tree"),
        };
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GitRepoSetLayout {
    entries: BTreeMap<RepoKey, GitPath>,
}

impl GitRepoSetLayout {
    pub fn new(entries: BTreeMap<RepoKey, GitPath>) -> Result<Self, GitArtifactError> {
        Ok(Self { entries })
    }

    #[must_use]
    pub fn entries(&self) -> &BTreeMap<RepoKey, GitPath> {
        &self.entries
    }

    #[must_use]
    pub fn path_for(&self, repo: &RepoKey) -> Option<&GitPath> {
        self.entries.get(repo)
    }

    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        for (repo, path) in &self.entries {
            builder.update(b"layout");
            repo.feed_fingerprint(builder);
            feed_field(builder, path.as_str().as_bytes());
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GitRepoSetArtifact {
    repos: BTreeMap<RepoKey, GitRepoArtifact>,
    layout: GitRepoSetLayout,
}

impl GitRepoSetArtifact {
    pub fn new(
        repos: BTreeMap<RepoKey, GitRepoArtifact>,
        layout: GitRepoSetLayout,
    ) -> Result<Self, GitArtifactError> {
        let artifact = Self { repos, layout };
        artifact.validate()?;
        Ok(artifact)
    }

    #[must_use]
    pub fn repos(&self) -> &BTreeMap<RepoKey, GitRepoArtifact> {
        &self.repos
    }

    #[must_use]
    pub fn repo(&self, repo: &RepoKey) -> Option<&GitRepoArtifact> {
        self.repos.get(repo)
    }

    #[must_use]
    pub const fn layout(&self) -> &GitRepoSetLayout {
        &self.layout
    }

    fn content_id(&self) -> ContentId {
        let mut builder = FingerprintBuilder::new();
        builder.update(b"leaven.artifact-git.repo-set.v1");
        for (repo, artifact) in &self.repos {
            builder.update(b"repo");
            repo.feed_fingerprint(&mut builder);
            artifact.feed_fingerprint(&mut builder);
        }
        self.layout.feed_fingerprint(&mut builder);
        ContentId::from_bytes(builder.finish().0)
    }

    fn apply_repo_change(
        &mut self,
        repo: &RepoKey,
        change: &GitRepoChange,
    ) -> Result<(), GitArtifactError> {
        let artifact = self
            .repos
            .get_mut(repo)
            .ok_or_else(|| GitArtifactError::MissingRepo { repo: repo.clone() })?;
        match change {
            GitRepoChange::AdvanceTo {
                expected_parent,
                child,
            } => {
                if artifact.revision() != expected_parent {
                    return Err(GitArtifactError::RevisionParentMismatch {
                        repo: repo.clone(),
                        expected_parent: expected_parent.clone(),
                        actual_parent: artifact.revision().clone(),
                    });
                }
                artifact.advance_to(child.clone());
            }
        }
        Ok(())
    }
}

impl Artifact for GitRepoSetArtifact {
    type Change = GitRepoSetChange;
    type ApplyError = GitArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(self.content_id())
    }

    fn cache_identity(&self) -> Option<CacheIdentity> {
        Some(CacheIdentity::Content(self.content_id()))
    }

    fn validate(&self) -> Result<(), Self::ApplyError> {
        if self.repos.is_empty() {
            return Err(GitArtifactError::EmptyRepoSet);
        }
        for (key, repo) in &self.repos {
            if repo.repo().key() != key {
                return Err(GitArtifactError::RepoKeyMismatch {
                    expected: key.clone(),
                    repo: repo.repo().key().clone(),
                });
            }
            if self.layout.path_for(key).is_none() {
                return Err(GitArtifactError::MissingRepoLayout { repo: key.clone() });
            }
        }
        for key in self.layout.entries().keys() {
            if !self.repos.contains_key(key) {
                return Err(GitArtifactError::UnknownRepoLayout { repo: key.clone() });
            }
        }
        Ok(())
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        let mut next = self.clone();
        match change {
            GitRepoSetChange::AdvanceRepo {
                repo,
                expected_parent,
                child,
            } => next.apply_repo_change(
                repo,
                &GitRepoChange::AdvanceTo {
                    expected_parent: expected_parent.clone(),
                    child: child.clone(),
                },
            )?,
            GitRepoSetChange::AdvanceRepos { repo_changes } => {
                for (repo, change) in repo_changes {
                    next.apply_repo_change(repo, change)?;
                }
            }
        }
        next.validate()?;
        Ok(next)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GitRepoChange {
    AdvanceTo {
        expected_parent: GitRevision,
        child: GitRevision,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GitRepoSetChange {
    AdvanceRepo {
        repo: RepoKey,
        expected_parent: GitRevision,
        child: GitRevision,
    },
    AdvanceRepos {
        repo_changes: BTreeMap<RepoKey, GitRepoChange>,
    },
}

fn validate_repo_key(key: &str) -> Result<(), GitArtifactError> {
    if key.is_empty() {
        return invalid_repo_key(key, "repo key is empty");
    }
    if key == "." || key == ".." || key.contains("..") {
        return invalid_repo_key(key, "repo key contains dot-dot");
    }
    if key.starts_with('.') || key.ends_with('.') {
        return invalid_repo_key(key, "repo key cannot start or end with dot");
    }
    if key.contains('/') || key.contains('\\') {
        return invalid_repo_key(key, "repo key cannot contain path separators");
    }
    if key.contains('\0') {
        return invalid_repo_key(key, "repo key contains nul byte");
    }
    if !key
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return invalid_repo_key(key, "repo key contains forbidden character");
    }
    Ok(())
}

fn invalid_repo_key(key: &str, reason: &'static str) -> Result<(), GitArtifactError> {
    Err(GitArtifactError::InvalidRepoKey {
        key: key.to_owned(),
        reason,
    })
}

fn feed_field(builder: &mut FingerprintBuilder, bytes: &[u8]) {
    builder
        .update((bytes.len() as u64).to_le_bytes())
        .update(bytes);
}
