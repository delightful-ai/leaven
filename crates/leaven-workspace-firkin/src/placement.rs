use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct FirkinProductPodId(String);

impl FirkinProductPodId {
    pub fn new(value: impl Into<String>) -> Result<Self, FirkinRuntimeError> {
        non_empty(value.into(), "product pod id").map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FirkinProductPodId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct FirkinContainerId(String);

impl FirkinContainerId {
    pub fn new(value: impl Into<String>) -> Result<Self, FirkinRuntimeError> {
        non_empty(value.into(), "container id").map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FirkinContainerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct FirkinImageRef(String);

impl FirkinImageRef {
    pub fn new(value: impl Into<String>) -> Result<Self, FirkinRuntimeError> {
        non_empty(value.into(), "image ref").map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FirkinImageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct FirkinGuestPath(String);

impl FirkinGuestPath {
    pub fn new(value: impl Into<String>) -> Result<Self, FirkinRuntimeError> {
        let value = value.into();
        if !value.starts_with('/') {
            return Err(FirkinRuntimeError::InvalidPlacement {
                field: "guest path",
                value,
                reason: "must be absolute",
            });
        }
        if value.split('/').any(|part| part == "..") {
            return Err(FirkinRuntimeError::InvalidPlacement {
                field: "guest path",
                value,
                reason: "must not contain parent traversal",
            });
        }
        Ok(Self(normalize_guest_path(&value)))
    }

    #[must_use]
    pub fn join_workspace_path(&self, path: &leaven_workspace::WorkspacePath) -> Self {
        if path.as_str().is_empty() {
            return self.clone();
        }
        let mut value = self.0.clone();
        if !value.ends_with('/') {
            value.push('/');
        }
        value.push_str(path.as_str());
        Self(value)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FirkinGuestPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirkinWorkspaceContext {
    product_pod_id: FirkinProductPodId,
    container_id: FirkinContainerId,
    workspace_root: FirkinGuestPath,
    image: FirkinImageRef,
}

impl FirkinWorkspaceContext {
    #[must_use]
    pub const fn new(
        product_pod_id: FirkinProductPodId,
        container_id: FirkinContainerId,
        workspace_root: FirkinGuestPath,
        image: FirkinImageRef,
    ) -> Self {
        Self {
            product_pod_id,
            container_id,
            workspace_root,
            image,
        }
    }

    #[must_use]
    pub const fn product_pod_id(&self) -> &FirkinProductPodId {
        &self.product_pod_id
    }

    #[must_use]
    pub const fn container_id(&self) -> &FirkinContainerId {
        &self.container_id
    }

    #[must_use]
    pub const fn workspace_root(&self) -> &FirkinGuestPath {
        &self.workspace_root
    }

    #[must_use]
    pub const fn image(&self) -> &FirkinImageRef {
        &self.image
    }
}

fn non_empty(value: String, field: &'static str) -> Result<String, FirkinRuntimeError> {
    if value.is_empty() {
        return Err(FirkinRuntimeError::InvalidPlacement {
            field,
            value,
            reason: "must not be empty",
        });
    }
    Ok(value)
}

fn normalize_guest_path(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_slash = false;
    for ch in value.chars() {
        if ch == '/' {
            if !previous_slash {
                normalized.push(ch);
            }
            previous_slash = true;
        } else {
            normalized.push(ch);
            previous_slash = false;
        }
    }
    if normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

use crate::FirkinRuntimeError;
