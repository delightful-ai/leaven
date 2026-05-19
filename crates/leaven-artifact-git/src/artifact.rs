use std::collections::BTreeMap;

use leaven_core::{Artifact, ArtifactIdentity, CacheIdentity};
use leaven_kernel::{ContentId, FingerprintBuilder};

use crate::{GitArtifactError, GitChange, GitPath, GitRef, GitRefKey};

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GitArtifact {
    files: BTreeMap<GitPath, Vec<u8>>,
    refs: BTreeMap<GitRefKey, GitRef>,
}

impl GitArtifact {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn new(files: BTreeMap<GitPath, Vec<u8>>) -> Self {
        Self {
            files,
            refs: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn from_parts(
        files: BTreeMap<GitPath, Vec<u8>>,
        refs: BTreeMap<GitRefKey, GitRef>,
    ) -> Self {
        Self { files, refs }
    }

    #[must_use]
    pub fn files(&self) -> &BTreeMap<GitPath, Vec<u8>> {
        &self.files
    }

    #[must_use]
    pub fn refs(&self) -> &BTreeMap<GitRefKey, GitRef> {
        &self.refs
    }

    #[must_use]
    pub fn ref_by_key(&self, key: &GitRefKey) -> Option<&GitRef> {
        self.refs.get(key)
    }

    pub fn refs_for_prefix<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a GitRef> + 'a {
        self.refs
            .values()
            .filter(move |reference| reference.name().as_str().starts_with(prefix))
    }

    fn content_id(&self) -> ContentId {
        let mut builder = FingerprintBuilder::new();
        builder.update(b"leaven.artifact-git.v1");
        for (path, bytes) in &self.files {
            builder
                .update(b"file")
                .update(path.as_str().as_bytes())
                .update((bytes.len() as u64).to_le_bytes())
                .update(bytes);
        }
        for (key, reference) in &self.refs {
            builder.update(b"ref");
            key.feed_fingerprint(&mut builder);
            reference.feed_fingerprint(&mut builder);
        }
        ContentId::from_bytes(builder.finish().0)
    }
}

impl Artifact for GitArtifact {
    type Change = GitChange;
    type ApplyError = GitArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(self.content_id())
    }

    fn cache_identity(&self) -> Option<CacheIdentity> {
        Some(CacheIdentity::Content(self.content_id()))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        let mut next = self.clone();
        change.apply_to(&mut next)?;
        Ok(next)
    }
}

impl GitArtifact {
    pub(crate) fn write_file(&mut self, path: GitPath, bytes: Vec<u8>) {
        self.files.insert(path, bytes);
    }

    pub(crate) fn remove_file(&mut self, path: &GitPath) -> Result<(), GitArtifactError> {
        if self.files.remove(path).is_none() {
            return Err(GitArtifactError::MissingPath { path: path.clone() });
        }
        Ok(())
    }

    pub(crate) fn replace_files(&mut self, files: BTreeMap<GitPath, Vec<u8>>) {
        self.files = files;
    }

    pub(crate) fn upsert_ref(&mut self, reference: GitRef) {
        self.refs.insert(reference.key().clone(), reference);
    }

    pub(crate) fn remove_ref(&mut self, key: &GitRefKey) -> Result<(), GitArtifactError> {
        if self.refs.remove(key).is_none() {
            return Err(GitArtifactError::MissingRef { key: key.clone() });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GitArtifactIdentityMode {
    Commit,
    Tree,
}
