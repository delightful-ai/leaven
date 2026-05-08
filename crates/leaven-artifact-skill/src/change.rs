//! Typed edits over skill banks.

use crate::{SkillFile, SkillFolder, SkillName, SkillPath};

/// Artifact-native change for [`SkillBank`](crate::SkillBank).
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum SkillBankChange {
    /// Add a new skill folder.
    CreateSkill {
        /// New folder.
        folder: SkillFolder,
    },
    /// Replace an existing skill folder atomically.
    ReplaceSkill {
        /// Existing skill name.
        name: SkillName,
        /// Replacement folder. Its manifest name must match `name`.
        folder: SkillFolder,
    },
    /// Remove an existing skill.
    RemoveSkill {
        /// Skill to remove.
        name: SkillName,
    },
    /// Rename a skill folder and its `SKILL.md` name field together.
    RenameSkill {
        /// Existing skill name.
        from: SkillName,
        /// New skill name.
        to: SkillName,
    },
    /// Write or overwrite one file.
    WriteFile {
        /// Skill containing the file.
        skill: SkillName,
        /// Skill-root-relative path.
        path: SkillPath,
        /// Replacement file.
        file: SkillFile,
    },
    /// Remove one file.
    RemoveFile {
        /// Skill containing the file.
        skill: SkillName,
        /// Skill-root-relative path.
        path: SkillPath,
    },
    /// Rename one file inside a skill.
    RenameFile {
        /// Skill containing the file.
        skill: SkillName,
        /// Existing path.
        from: SkillPath,
        /// New path.
        to: SkillPath,
    },
    /// Toggle the executable bit captured by the artifact.
    SetExecutable {
        /// Skill containing the file.
        skill: SkillName,
        /// Existing file path.
        path: SkillPath,
        /// New executable bit.
        executable: bool,
    },
    /// Apply a batch atomically and validate the final bank.
    Atomic(Vec<Self>),
}
