//! Workspace layout for materialized skill banks.

use leaven_workspace::{WorkspacePath, WorkspacePathError};

/// Where a [`SkillBank`](leaven_artifact_skill::SkillBank) appears in a workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillWorkspaceLayout {
    /// Directory containing one child directory per skill.
    pub skills_root: WorkspacePath,
}

impl SkillWorkspaceLayout {
    /// Uses the workspace root as the skill root.
    #[must_use]
    pub const fn root() -> Self {
        Self {
            skills_root: WorkspacePath::root(),
        }
    }

    /// Uses a named workspace subdirectory as the skill root.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspacePathError`] when `skills_root` is not a valid
    /// workspace path.
    pub fn new(skills_root: impl AsRef<str>) -> Result<Self, WorkspacePathError> {
        Ok(Self {
            skills_root: WorkspacePath::new(skills_root)?,
        })
    }
}

impl Default for SkillWorkspaceLayout {
    fn default() -> Self {
        Self::root()
    }
}
