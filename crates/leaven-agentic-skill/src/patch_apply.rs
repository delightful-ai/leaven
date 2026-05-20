//! Validated skill patch application.

use std::{error::Error, fmt};

use std::collections::{BTreeMap, BTreeSet};

use leaven_artifact_skill::{SkillBank, SkillBankChange};

use crate::{SkillBankChangeReport, SkillPatchEditKind, SkillPatchFileRef, SkillPatchPlan};

/// Successful application of a validated skill patch plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillPatchApplication {
    parent: SkillBank,
    plan: SkillPatchPlan,
    change: SkillBankChange,
    child: SkillBank,
    report: SkillBankChangeReport,
}

impl SkillPatchApplication {
    /// Applies a validated patch plan as one atomic skill-bank change.
    ///
    /// The plan remains separate from the concrete artifact change because
    /// paper runners own patch parsing and wording. This function owns the
    /// shared mechanical boundary: apply atomically, validate the resulting
    /// bank, and return a report or rollback evidence.
    pub fn apply(
        parent: &SkillBank,
        plan: SkillPatchPlan,
        changes: impl Into<Vec<SkillBankChange>>,
    ) -> Result<Self, SkillPatchApplicationError> {
        let changes = changes.into();
        if changes.is_empty() {
            return Err(SkillPatchApplicationError::PlanMismatch(
                "non-empty patch plan produced no concrete changes".to_owned(),
            ));
        }
        validate_changes_match_plan(parent, &plan, &changes)?;
        let change = SkillBankChange::Atomic(changes);
        let child = parent.apply_skill_change(&change).map_err(|error| {
            SkillPatchApplicationError::RolledBack(Box::new(SkillPatchRollback::new(
                parent.clone(),
                plan.clone(),
                change.clone(),
                error.to_string(),
            )))
        })?;
        let report = SkillBankChangeReport::from_change(parent, &change).map_err(|error| {
            SkillPatchApplicationError::RolledBack(Box::new(SkillPatchRollback::new(
                parent.clone(),
                plan.clone(),
                change.clone(),
                error.to_string(),
            )))
        })?;
        Ok(Self {
            parent: parent.clone(),
            plan,
            change,
            child,
            report,
        })
    }

    /// Parent skill bank before application.
    #[must_use]
    pub const fn parent(&self) -> &SkillBank {
        &self.parent
    }

    /// Validated patch plan that justified the change.
    #[must_use]
    pub const fn plan(&self) -> &SkillPatchPlan {
        &self.plan
    }

    /// Atomic skill-bank change applied to the parent.
    #[must_use]
    pub const fn change(&self) -> &SkillBankChange {
        &self.change
    }

    /// Child skill bank after application.
    #[must_use]
    pub const fn child(&self) -> &SkillBank {
        &self.child
    }

    /// Operation-aware report for the applied change.
    #[must_use]
    pub const fn report(&self) -> &SkillBankChangeReport {
        &self.report
    }
}

fn validate_changes_match_plan(
    parent: &SkillBank,
    plan: &SkillPatchPlan,
    changes: &[SkillBankChange],
) -> Result<(), SkillPatchApplicationError> {
    let expected = plan
        .edits()
        .iter()
        .map(|edit| {
            let kind = match edit.kind() {
                SkillPatchEditKind::Modify { .. } => ConcretePatchKind::Modify,
                SkillPatchEditKind::CreateFile => ConcretePatchKind::CreateFile,
                SkillPatchEditKind::DeleteFile => ConcretePatchKind::DeleteFile,
            };
            (edit.target().clone(), kind)
        })
        .collect::<Vec<_>>();
    let actual = concrete_patch_intents(parent, changes);
    if counted(expected.iter()) == counted(actual.iter()) {
        return Ok(());
    }
    Err(SkillPatchApplicationError::PlanMismatch(format!(
        "plan edits {expected:?} but concrete changes perform {actual:?}"
    )))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum ConcretePatchKind {
    Modify,
    CreateFile,
    DeleteFile,
    SetExecutable,
}

fn counted<'a, T>(items: impl IntoIterator<Item = &'a T>) -> BTreeMap<T, usize>
where
    T: Clone + Ord + 'a,
{
    let mut counts = BTreeMap::new();
    for item in items {
        *counts.entry(item.clone()).or_default() += 1;
    }
    counts
}

fn concrete_patch_intents(
    parent: &SkillBank,
    changes: &[SkillBankChange],
) -> Vec<(SkillPatchFileRef, ConcretePatchKind)> {
    let mut intents = Vec::new();
    for change in changes {
        collect_concrete_patch_intents(parent, change, &mut intents);
    }
    intents
}

fn collect_concrete_patch_intents(
    parent: &SkillBank,
    change: &SkillBankChange,
    intents: &mut Vec<(SkillPatchFileRef, ConcretePatchKind)>,
) {
    match change {
        SkillBankChange::WriteFile { skill, path, .. } => {
            let kind = if parent
                .get(skill)
                .and_then(|folder| folder.file(path))
                .is_some()
            {
                ConcretePatchKind::Modify
            } else {
                ConcretePatchKind::CreateFile
            };
            intents.push((SkillPatchFileRef::new(skill.clone(), path.clone()), kind));
        }
        SkillBankChange::RemoveFile { skill, path } => {
            intents.push((
                SkillPatchFileRef::new(skill.clone(), path.clone()),
                ConcretePatchKind::DeleteFile,
            ));
        }
        SkillBankChange::RenameFile { skill, from, to } => {
            intents.push((
                SkillPatchFileRef::new(skill.clone(), from.clone()),
                ConcretePatchKind::DeleteFile,
            ));
            intents.push((
                SkillPatchFileRef::new(skill.clone(), to.clone()),
                ConcretePatchKind::CreateFile,
            ));
        }
        SkillBankChange::SetExecutable { skill, path, .. } => {
            intents.push((
                SkillPatchFileRef::new(skill.clone(), path.clone()),
                ConcretePatchKind::SetExecutable,
            ));
        }
        SkillBankChange::CreateSkill { folder } => {
            for path in folder.entries().keys() {
                intents.push((
                    SkillPatchFileRef::new(folder.name().clone(), path.clone()),
                    ConcretePatchKind::CreateFile,
                ));
            }
        }
        SkillBankChange::RemoveSkill { name } => {
            if let Some(folder) = parent.get(name) {
                for path in folder.entries().keys() {
                    intents.push((
                        SkillPatchFileRef::new(name.clone(), path.clone()),
                        ConcretePatchKind::DeleteFile,
                    ));
                }
            }
        }
        SkillBankChange::ReplaceSkill { name, folder } => {
            let before = parent.get(name).map(|folder| folder.entries());
            let after = folder.entries();
            let paths = before
                .into_iter()
                .flat_map(|entries| entries.keys())
                .chain(after.keys())
                .collect::<BTreeSet<_>>();
            for path in paths {
                let before_file = before.and_then(|entries| entries.get(path));
                let after_file = after.get(path);
                let kind = match (before_file, after_file) {
                    (None, Some(_)) => Some(ConcretePatchKind::CreateFile),
                    (Some(_), None) => Some(ConcretePatchKind::DeleteFile),
                    (Some(old), Some(new)) if old.bytes() != new.bytes() => {
                        Some(ConcretePatchKind::Modify)
                    }
                    (Some(old), Some(new)) if old.permissions() != new.permissions() => {
                        Some(ConcretePatchKind::SetExecutable)
                    }
                    _ => None,
                };
                if let Some(kind) = kind {
                    intents.push((SkillPatchFileRef::new(name.clone(), path.clone()), kind));
                }
            }
        }
        SkillBankChange::RenameSkill { from, to } => {
            if let Some(folder) = parent.get(from) {
                for path in folder.entries().keys() {
                    intents.push((
                        SkillPatchFileRef::new(from.clone(), path.clone()),
                        ConcretePatchKind::DeleteFile,
                    ));
                    intents.push((
                        SkillPatchFileRef::new(to.clone(), path.clone()),
                        ConcretePatchKind::CreateFile,
                    ));
                }
            }
        }
        SkillBankChange::Atomic(changes) => {
            for change in changes {
                collect_concrete_patch_intents(parent, change, intents);
            }
        }
    }
}

/// Failed patch application with rollback evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillPatchRollback {
    parent: SkillBank,
    plan: SkillPatchPlan,
    change: SkillBankChange,
    error: String,
}

impl SkillPatchRollback {
    fn new(
        parent: SkillBank,
        plan: SkillPatchPlan,
        change: SkillBankChange,
        error: String,
    ) -> Self {
        Self {
            parent,
            plan,
            change,
            error,
        }
    }

    /// Parent skill bank preserved after rollback.
    #[must_use]
    pub const fn parent(&self) -> &SkillBank {
        &self.parent
    }

    /// Validated patch plan that authorized the failed change.
    #[must_use]
    pub const fn plan(&self) -> &SkillPatchPlan {
        &self.plan
    }

    /// Atomic skill-bank change that failed.
    #[must_use]
    pub const fn change(&self) -> &SkillBankChange {
        &self.change
    }

    /// Application or validation failure text.
    #[must_use]
    pub fn error(&self) -> &str {
        &self.error
    }
}

/// Skill patch application failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillPatchApplicationError {
    /// Concrete artifact changes do not match the validated patch plan.
    PlanMismatch(String),
    /// The atomic skill-bank change failed and the parent bank was preserved.
    RolledBack(Box<SkillPatchRollback>),
}

impl fmt::Display for SkillPatchApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlanMismatch(reason) => {
                write!(
                    formatter,
                    "skill patch application does not match plan: {reason}"
                )
            }
            Self::RolledBack(rollback) => {
                write!(
                    formatter,
                    "skill patch application rolled back: {}",
                    rollback.error()
                )
            }
        }
    }
}

impl Error for SkillPatchApplicationError {}
