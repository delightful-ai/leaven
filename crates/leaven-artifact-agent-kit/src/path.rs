use std::fmt;
use std::str::FromStr;

/// Portable path inside an AgentKit repo subtree.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct AgentKitPath(String);

impl AgentKitPath {
    /// Validates and normalizes an AgentKit-relative path.
    ///
    /// A single trailing slash is accepted for directory slots and is not part
    /// of the canonical value.
    ///
    /// # Errors
    ///
    /// Returns [`AgentKitPathError`] for absolute paths, parent traversal,
    /// backslashes, NUL bytes, empty input, or empty non-trailing components.
    pub fn new(value: impl Into<String>) -> Result<Self, AgentKitPathError> {
        let value = value.into();
        let normalized = normalize_path(&value)?;
        Ok(Self(normalized))
    }

    /// Returns the canonical relative path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentKitPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AgentKitPath {
    type Err = AgentKitPathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for AgentKitPath {
    type Error = AgentKitPathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Invalid path inside an AgentKit subtree.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum AgentKitPathError {
    /// Paths must not be empty.
    #[error("agent kit path is empty")]
    Empty,
    /// AgentKit paths are portable POSIX-style relative paths.
    #[error("agent kit path must be relative")]
    Absolute,
    /// Empty path components are not accepted.
    #[error("agent kit path contains an empty component")]
    EmptyComponent,
    /// Current-directory components are not accepted.
    #[error("agent kit path contains a current-directory component")]
    CurrentDirectory,
    /// Parent traversal is not accepted.
    #[error("agent kit path contains parent traversal")]
    ParentTraversal,
    /// Backslashes are rejected to keep materialization platform-neutral.
    #[error("agent kit path contains a backslash")]
    Backslash,
    /// NUL bytes are never valid in paths.
    #[error("agent kit path contains NUL")]
    Nul,
}

fn normalize_path(value: &str) -> Result<String, AgentKitPathError> {
    if value.is_empty() {
        return Err(AgentKitPathError::Empty);
    }
    if value.starts_with('/') {
        return Err(AgentKitPathError::Absolute);
    }
    if value.contains('\\') {
        return Err(AgentKitPathError::Backslash);
    }
    if value.contains('\0') {
        return Err(AgentKitPathError::Nul);
    }

    let normalized = value.trim_end_matches('/');
    if normalized.is_empty() {
        return Err(AgentKitPathError::Empty);
    }

    for component in normalized.split('/') {
        if component.is_empty() {
            return Err(AgentKitPathError::EmptyComponent);
        }
        if component == "." {
            return Err(AgentKitPathError::CurrentDirectory);
        }
        if component == ".." {
            return Err(AgentKitPathError::ParentTraversal);
        }
    }

    Ok(normalized.to_owned())
}
