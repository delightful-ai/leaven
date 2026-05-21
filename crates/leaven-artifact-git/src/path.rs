use std::fmt;

use crate::GitArtifactError;

#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct GitPath(String);

impl GitPath {
    pub fn new(path: impl Into<String>) -> Result<Self, GitArtifactError> {
        let path = path.into();
        validate_path(&path)?;
        Ok(Self(path))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GitPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn validate_path(path: &str) -> Result<(), GitArtifactError> {
    if path.is_empty() {
        return invalid(path, "path is empty");
    }
    if path.starts_with('/') {
        return invalid(path, "path must be relative");
    }
    if path.contains('\\') {
        return invalid(path, "path must use forward slashes");
    }
    if path.contains('\0') {
        return invalid(path, "path contains nul byte");
    }
    for component in path.split('/') {
        if component.is_empty() {
            return invalid(path, "path contains empty component");
        }
        if component == "." || component == ".." {
            return invalid(path, "path contains non-normal component");
        }
    }
    Ok(())
}

fn invalid(path: &str, reason: &'static str) -> Result<(), GitArtifactError> {
    Err(GitArtifactError::InvalidPath {
        path: path.to_owned(),
        reason,
    })
}
