//! Backend-neutral workspace paths.

use std::fmt;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::WorkspacePathError;

/// Normalized relative path inside a workspace.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspacePath {
    inner: String,
}

impl WorkspacePath {
    /// Workspace root.
    #[must_use]
    pub const fn root() -> Self {
        Self {
            inner: String::new(),
        }
    }

    /// Parse a relative workspace path.
    pub fn new(path: impl AsRef<str>) -> Result<Self, WorkspacePathError> {
        let raw = path.as_ref();
        if raw.is_empty() {
            return Err(WorkspacePathError::Empty);
        }
        if raw.starts_with('/') {
            return Err(WorkspacePathError::Absolute(raw.to_owned()));
        }
        if raw.split('/').any(str::is_empty) {
            return Err(WorkspacePathError::EmptyComponent(raw.to_owned()));
        }
        let path = Path::new(raw);
        let mut parts = Vec::new();
        for component in path.components() {
            match component {
                Component::Normal(part) => {
                    let Some(part) = part.to_str() else {
                        return Err(WorkspacePathError::EmptyComponent(raw.to_owned()));
                    };
                    parts.push(part.to_owned());
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    return Err(WorkspacePathError::ParentTraversal(raw.to_owned()));
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(WorkspacePathError::Absolute(raw.to_owned()));
                }
            }
        }
        if parts.is_empty() {
            return Err(WorkspacePathError::Empty);
        }
        Ok(Self {
            inner: parts.join("/"),
        })
    }

    /// Join a child path below this path.
    pub fn join(&self, child: impl AsRef<str>) -> Result<Self, WorkspacePathError> {
        let child = Self::new(child)?;
        if self.inner.is_empty() {
            Ok(child)
        } else {
            Ok(Self {
                inner: format!("{}/{}", self.inner, child.inner),
            })
        }
    }

    /// String form using `/` separators.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    #[must_use]
    pub fn starts_with_component(&self, component: &str) -> bool {
        self.inner.split('/').next() == Some(component)
    }

    /// Convert this workspace path into a relative host path.
    ///
    /// This is only for backends that expose a host filesystem layout. Pure
    /// remote backends should keep using [`WorkspacePath`] directly.
    #[must_use]
    pub fn to_host_relative(&self) -> PathBuf {
        let mut path = PathBuf::new();
        for part in self.inner.split('/').filter(|part| !part.is_empty()) {
            path.push(part);
        }
        path
    }
}

impl fmt::Display for WorkspacePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
