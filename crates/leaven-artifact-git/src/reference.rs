use std::collections::BTreeMap;
use std::fmt;

use leaven_kernel::FingerprintBuilder;

use crate::GitArtifactError;

#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct GitObjectId(String);

impl GitObjectId {
    pub fn new(id: impl Into<String>) -> Result<Self, GitArtifactError> {
        let id = id.into();
        let valid_width = id.len() == 40 || id.len() == 64;
        if !valid_width || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(GitArtifactError::InvalidObjectId { id });
        }
        Ok(Self(id.to_ascii_lowercase()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GitObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct GitRefName(String);

impl GitRefName {
    pub fn new(name: impl Into<String>) -> Result<Self, GitArtifactError> {
        let name = name.into();
        validate_ref_name(&name)?;
        Ok(Self(name))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GitRefName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum GitRefKind {
    Branch,
    Tag,
}

impl GitRefKind {
    const fn fingerprint_byte(self) -> u8 {
        match self {
            Self::Branch => b'b',
            Self::Tag => b't',
        }
    }
}

impl fmt::Display for GitRefKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Branch => f.write_str("branch"),
            Self::Tag => f.write_str("tag"),
        }
    }
}

#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct GitRefKey {
    kind: GitRefKind,
    name: GitRefName,
}

impl GitRefKey {
    #[must_use]
    pub const fn new(kind: GitRefKind, name: GitRefName) -> Self {
        Self { kind, name }
    }

    #[must_use]
    pub const fn kind(&self) -> GitRefKind {
        self.kind
    }

    #[must_use]
    pub const fn name(&self) -> &GitRefName {
        &self.name
    }

    pub(crate) fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        builder
            .update([self.kind.fingerprint_byte()])
            .update(self.name.as_str().as_bytes());
    }
}

impl fmt::Display for GitRefKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GitRefTarget {
    Object(GitObjectId),
    Symbolic(GitRefName),
}

impl GitRefTarget {
    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        match self {
            Self::Object(id) => {
                builder.update(b"object").update(id.as_str().as_bytes());
            }
            Self::Symbolic(name) => {
                builder.update(b"symbolic").update(name.as_str().as_bytes());
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GitLineage {
    parent: Option<GitRefKey>,
    generation: u32,
}

impl GitLineage {
    #[must_use]
    pub const fn root() -> Self {
        Self {
            parent: None,
            generation: 0,
        }
    }

    #[must_use]
    pub fn child(parent: &GitRefKey, generation: u32) -> Self {
        Self {
            parent: Some(parent.clone()),
            generation,
        }
    }

    #[must_use]
    pub const fn parent(&self) -> Option<&GitRefKey> {
        self.parent.as_ref()
    }

    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        builder.update(self.generation.to_le_bytes());
        match &self.parent {
            Some(parent) => {
                builder.update(b"parent");
                parent.feed_fingerprint(builder);
            }
            None => {
                builder.update(b"root");
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GitRef {
    key: GitRefKey,
    target: GitRefTarget,
    lineage: Option<GitLineage>,
    metadata: BTreeMap<String, String>,
}

impl GitRef {
    #[must_use]
    pub fn new(kind: GitRefKind, name: GitRefName, target: GitRefTarget) -> Self {
        Self {
            key: GitRefKey::new(kind, name),
            target,
            lineage: None,
            metadata: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn key(&self) -> &GitRefKey {
        &self.key
    }

    #[must_use]
    pub const fn name(&self) -> &GitRefName {
        self.key.name()
    }

    #[must_use]
    pub const fn target(&self) -> &GitRefTarget {
        &self.target
    }

    #[must_use]
    pub const fn lineage(&self) -> Option<&GitLineage> {
        self.lineage.as_ref()
    }

    #[must_use]
    pub fn metadata(&self) -> &BTreeMap<String, String> {
        &self.metadata
    }

    #[must_use]
    pub fn with_lineage(mut self, lineage: GitLineage) -> Self {
        self.lineage = Some(lineage);
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    pub(crate) fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        self.target.feed_fingerprint(builder);
        match &self.lineage {
            Some(lineage) => {
                builder.update(b"lineage");
                lineage.feed_fingerprint(builder);
            }
            None => {
                builder.update(b"no-lineage");
            }
        }
        for (key, value) in &self.metadata {
            builder
                .update(b"metadata")
                .update(key.as_bytes())
                .update(value.as_bytes());
        }
    }
}

fn validate_ref_name(name: &str) -> Result<(), GitArtifactError> {
    if name.is_empty() {
        return invalid_ref(name, "ref name is empty");
    }
    if name.starts_with('/') || name.ends_with('/') {
        return invalid_ref(name, "ref name cannot start or end with slash");
    }
    if name.contains("//") {
        return invalid_ref(name, "ref name contains empty component");
    }
    if name.contains("..") {
        return invalid_ref(name, "ref name contains dot-dot");
    }
    if ends_with_lock_suffix(name) {
        return invalid_ref(name, "ref name cannot end with .lock");
    }
    for byte in name.bytes() {
        if byte.is_ascii_control()
            || byte.is_ascii_whitespace()
            || matches!(byte, b'~' | b'^' | b':' | b'?' | b'*' | b'[' | b'\\')
        {
            return invalid_ref(name, "ref name contains forbidden character");
        }
    }
    for component in name.split('/') {
        if component == "." || ends_with_lock_suffix(component) {
            return invalid_ref(name, "ref name contains forbidden component");
        }
    }
    Ok(())
}

fn invalid_ref(name: &str, reason: &'static str) -> Result<(), GitArtifactError> {
    Err(GitArtifactError::InvalidRefName {
        name: name.to_owned(),
        reason,
    })
}

fn ends_with_lock_suffix(value: &str) -> bool {
    value
        .get(value.len().saturating_sub(5)..)
        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(".lock"))
}
