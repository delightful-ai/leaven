//! Skill-bank diffing into artifact-native changes.

use leaven_artifact_skill::{
    SkillBank, SkillBankChange, SkillFile, SkillFolder, SkillName, SkillPath,
};

/// Computes skill-bank changes after workspace mutation.
#[derive(Clone, Copy, Debug, Default)]
pub struct SkillBankDiff;

impl SkillBankDiff {
    /// Computes a change that transforms `parent` into `child`.
    ///
    /// Returns `None` when the banks are identical. This only emits operation
    /// kinds that are knowable from a final tree. A removed folder plus an added
    /// folder is represented as remove/create, not a rename, because final-state
    /// readback has no durable operation history. Stages that know they
    /// performed a rename should propose [`SkillBankChange::RenameSkill`]
    /// directly.
    pub fn diff(parent: &SkillBank, child: &SkillBank) -> Option<SkillBankChange> {
        let mut changes = Vec::new();

        for name in parent.folders().keys() {
            if !child.folders().contains_key(name) {
                changes.push(SkillBankChange::RemoveSkill { name: name.clone() });
            }
        }

        for (name, child_folder) in child.folders() {
            match parent.get(name) {
                None => changes.push(SkillBankChange::CreateSkill {
                    folder: child_folder.clone(),
                }),
                Some(parent_folder) => diff_folder(name, parent_folder, child_folder, &mut changes),
            }
        }

        match changes.len() {
            0 => None,
            1 => changes.pop(),
            _ => Some(SkillBankChange::Atomic(changes)),
        }
    }

    /// Detects whether the diff contains a named skill.
    pub fn mentions(change: &SkillBankChange, skill: &SkillName) -> bool {
        match change {
            SkillBankChange::CreateSkill { folder } => folder.name() == skill,
            SkillBankChange::ReplaceSkill { name, .. } | SkillBankChange::RemoveSkill { name } => {
                name == skill
            }
            SkillBankChange::RenameSkill { from, to } => from == skill || to == skill,
            SkillBankChange::WriteFile { skill: name, .. }
            | SkillBankChange::RemoveFile { skill: name, .. }
            | SkillBankChange::RenameFile { skill: name, .. }
            | SkillBankChange::SetExecutable { skill: name, .. } => name == skill,
            SkillBankChange::Atomic(changes) => {
                changes.iter().any(|change| Self::mentions(change, skill))
            }
        }
    }
}

fn diff_folder(
    name: &SkillName,
    parent: &SkillFolder,
    child: &SkillFolder,
    changes: &mut Vec<SkillBankChange>,
) {
    for path in parent.entries().keys() {
        if !child.entries().contains_key(path) {
            changes.push(SkillBankChange::RemoveFile {
                skill: name.clone(),
                path: path.clone(),
            });
        }
    }

    for (path, child_file) in child.entries() {
        match parent.entries().get(path) {
            None => changes.push(SkillBankChange::WriteFile {
                skill: name.clone(),
                path: path.clone(),
                file: child_file.clone(),
            }),
            Some(parent_file) => diff_file(name, path, parent_file, child_file, changes),
        }
    }
}

fn diff_file(
    name: &SkillName,
    path: &SkillPath,
    parent: &SkillFile,
    child: &SkillFile,
    changes: &mut Vec<SkillBankChange>,
) {
    if parent.bytes() != child.bytes() {
        changes.push(SkillBankChange::WriteFile {
            skill: name.clone(),
            path: path.clone(),
            file: child.clone(),
        });
    } else if parent.permissions() != child.permissions() {
        changes.push(SkillBankChange::SetExecutable {
            skill: name.clone(),
            path: path.clone(),
            executable: child.permissions().executable,
        });
    }
}
