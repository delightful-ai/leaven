//! Skill-bank change reports for result inspection.

use leaven_artifact_skill::{SkillBank, SkillBankChange, SkillBankError, SkillName, SkillPath};
use serde::{Deserialize, Serialize};

/// Operation-aware summary of a skill-bank mutation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkillBankChangeReport {
    pub skills_added: Vec<SkillName>,
    pub skills_removed: Vec<SkillName>,
    pub skills_rewritten: Vec<SkillName>,
    pub skills_renamed: Vec<SkillRenameReport>,
    pub descriptions_changed: Vec<SkillDescriptionChange>,
    pub files_changed: Vec<SkillFileChange>,
}

impl SkillBankChangeReport {
    /// Builds a report while verifying that `change` applies to `parent`.
    ///
    /// # Errors
    ///
    /// Returns [`SkillBankError`] if the change cannot be applied or would
    /// produce an invalid skill bank.
    pub fn from_change(
        parent: &SkillBank,
        change: &SkillBankChange,
    ) -> Result<Self, SkillBankError> {
        let mut current = parent.clone();
        let mut report = Self::default();
        record_and_apply(&mut current, change, &mut report)?;
        Ok(report)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkillRenameReport {
    pub from: SkillName,
    pub to: SkillName,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkillDescriptionChange {
    pub skill: SkillName,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkillFileChange {
    pub skill: SkillName,
    pub path: SkillPath,
    pub kind: SkillFileChangeKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SkillFileChangeKind {
    Added,
    Removed,
    Modified,
    Renamed { to: SkillPath },
    ExecutableChanged { executable: bool },
}

fn record_and_apply(
    current: &mut SkillBank,
    change: &SkillBankChange,
    report: &mut SkillBankChangeReport,
) -> Result<(), SkillBankError> {
    match change {
        SkillBankChange::Atomic(changes) => {
            for change in changes {
                record_and_apply(current, change, report)?;
            }
            Ok(())
        }
        _ => {
            let next = current.apply_skill_change(change)?;
            record_transition(current, &next, change, report);
            *current = next;
            Ok(())
        }
    }
}

fn record_transition(
    before: &SkillBank,
    after: &SkillBank,
    change: &SkillBankChange,
    report: &mut SkillBankChangeReport,
) {
    match change {
        SkillBankChange::CreateSkill { folder } => {
            let skill = folder.name().clone();
            report.skills_added.push(skill.clone());
            for path in folder.entries().keys() {
                report.files_changed.push(SkillFileChange {
                    skill: skill.clone(),
                    path: path.clone(),
                    kind: SkillFileChangeKind::Added,
                });
            }
        }
        SkillBankChange::ReplaceSkill { name, .. } => {
            report.skills_rewritten.push(name.clone());
            record_description_delta(before, after, name, report);
            record_file_delta(before, after, name, report);
        }
        SkillBankChange::RemoveSkill { name } => {
            report.skills_removed.push(name.clone());
            if let Some(folder) = before.get(name) {
                for path in folder.entries().keys() {
                    report.files_changed.push(SkillFileChange {
                        skill: name.clone(),
                        path: path.clone(),
                        kind: SkillFileChangeKind::Removed,
                    });
                }
            }
        }
        SkillBankChange::RenameSkill { from, to } => {
            report.skills_renamed.push(SkillRenameReport {
                from: from.clone(),
                to: to.clone(),
            });
        }
        SkillBankChange::WriteFile { skill, path, .. } => {
            record_description_delta(before, after, skill, report);
            let before_file = before.get(skill).and_then(|folder| folder.file(path));
            let after_file = after.get(skill).and_then(|folder| folder.file(path));
            let kind = match (before_file, after_file) {
                (None, Some(_)) => Some(SkillFileChangeKind::Added),
                (Some(old), Some(new)) if old.bytes() != new.bytes() => {
                    Some(SkillFileChangeKind::Modified)
                }
                (Some(old), Some(new)) if old.permissions() != new.permissions() => {
                    Some(SkillFileChangeKind::ExecutableChanged {
                        executable: new.permissions().executable,
                    })
                }
                _ => None,
            };
            if let Some(kind) = kind {
                report.files_changed.push(SkillFileChange {
                    skill: skill.clone(),
                    path: path.clone(),
                    kind,
                });
            }
        }
        SkillBankChange::RemoveFile { skill, path } => {
            report.files_changed.push(SkillFileChange {
                skill: skill.clone(),
                path: path.clone(),
                kind: SkillFileChangeKind::Removed,
            });
        }
        SkillBankChange::RenameFile { skill, from, to } => {
            report.files_changed.push(SkillFileChange {
                skill: skill.clone(),
                path: from.clone(),
                kind: SkillFileChangeKind::Renamed { to: to.clone() },
            });
        }
        SkillBankChange::SetExecutable {
            skill,
            path,
            executable,
        } => {
            report.files_changed.push(SkillFileChange {
                skill: skill.clone(),
                path: path.clone(),
                kind: SkillFileChangeKind::ExecutableChanged {
                    executable: *executable,
                },
            });
        }
        SkillBankChange::Atomic(_) => unreachable!("atomic changes are expanded before reporting"),
    }
}

fn record_description_delta(
    before: &SkillBank,
    after: &SkillBank,
    skill: &SkillName,
    report: &mut SkillBankChangeReport,
) {
    let Some(before) = before.get(skill) else {
        return;
    };
    let Some(after) = after.get(skill) else {
        return;
    };
    let before_description = before.manifest().description.as_str();
    let after_description = after.manifest().description.as_str();
    if before_description != after_description {
        report.descriptions_changed.push(SkillDescriptionChange {
            skill: skill.clone(),
            before: before_description.to_owned(),
            after: after_description.to_owned(),
        });
    }
}

fn record_file_delta(
    before: &SkillBank,
    after: &SkillBank,
    skill: &SkillName,
    report: &mut SkillBankChangeReport,
) {
    let Some(before) = before.get(skill) else {
        return;
    };
    let Some(after) = after.get(skill) else {
        return;
    };

    for path in before.entries().keys() {
        if !after.entries().contains_key(path) {
            report.files_changed.push(SkillFileChange {
                skill: skill.clone(),
                path: path.clone(),
                kind: SkillFileChangeKind::Removed,
            });
        }
    }

    for (path, after_file) in after.entries() {
        match before.entries().get(path) {
            None => report.files_changed.push(SkillFileChange {
                skill: skill.clone(),
                path: path.clone(),
                kind: SkillFileChangeKind::Added,
            }),
            Some(before_file) if before_file.bytes() != after_file.bytes() => {
                report.files_changed.push(SkillFileChange {
                    skill: skill.clone(),
                    path: path.clone(),
                    kind: SkillFileChangeKind::Modified,
                });
            }
            Some(before_file) if before_file.permissions() != after_file.permissions() => {
                report.files_changed.push(SkillFileChange {
                    skill: skill.clone(),
                    path: path.clone(),
                    kind: SkillFileChangeKind::ExecutableChanged {
                        executable: after_file.permissions().executable,
                    },
                });
            }
            Some(_) => {}
        }
    }
}
