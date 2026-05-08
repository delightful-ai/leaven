//! Paths inside skill folders.

use std::fmt;
use std::str::FromStr;

use crate::SkillPathError;

/// Portable relative path inside one skill folder.
#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(transparent)]
pub struct SkillPath(String);

impl SkillPath {
    /// Required root instruction file.
    pub const SKILL_MD: &'static str = "SKILL.md";

    /// Validates a skill-relative path.
    ///
    /// # Errors
    ///
    /// Returns [`SkillPathError`] for absolute paths, traversal, empty
    /// components, backslashes, NUL bytes, or empty input.
    pub fn new(value: impl Into<String>) -> Result<Self, SkillPathError> {
        let value = value.into();
        validate_path(&value)?;
        Ok(Self(value))
    }

    /// Returns the relative path string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the validated `SKILL.md` path.
    pub fn skill_md() -> Self {
        Self(Self::SKILL_MD.to_owned())
    }

    /// Whether this path is the required `SKILL.md` file.
    pub fn is_skill_md(&self) -> bool {
        self.as_str() == Self::SKILL_MD
    }
}

impl fmt::Display for SkillPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SkillPath {
    type Err = SkillPathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for SkillPath {
    type Error = SkillPathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

fn validate_path(value: &str) -> Result<(), SkillPathError> {
    if value.is_empty() {
        return Err(SkillPathError::Empty);
    }
    if value.starts_with('/') {
        return Err(SkillPathError::Absolute);
    }
    if value.contains('\\') {
        return Err(SkillPathError::Backslash);
    }
    if value.contains('\0') {
        return Err(SkillPathError::Nul);
    }
    for component in value.split('/') {
        if component.is_empty() {
            return Err(SkillPathError::EmptyComponent);
        }
        if component == "." {
            return Err(SkillPathError::CurrentDirectory);
        }
        if component == ".." {
            return Err(SkillPathError::ParentTraversal);
        }
    }
    Ok(())
}
