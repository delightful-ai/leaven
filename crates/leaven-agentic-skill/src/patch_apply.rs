//! Validated skill patch application.

use std::{error::Error, fmt};

use std::collections::{BTreeMap, BTreeSet};

use leaven_artifact_skill::{SkillBank, SkillBankChange, SkillFile};

use crate::{
    SkillBankChangeReport, SkillLineRange, SkillPatchEditKind, SkillPatchFileRef, SkillPatchPlan,
    SkillPatchRange,
};

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
        validate_line_range_preservation(parent, plan, changes)?;
        return Ok(());
    }
    Err(SkillPatchApplicationError::PlanMismatch(format!(
        "plan edits {expected:?} but concrete changes perform {actual:?}"
    )))
}

fn validate_line_range_preservation(
    parent: &SkillBank,
    plan: &SkillPatchPlan,
    changes: &[SkillBankChange],
) -> Result<(), SkillPatchApplicationError> {
    let line_ranges = planned_line_ranges(plan);
    if line_ranges.is_empty() {
        return Ok(());
    }
    let Ok(child) = parent.apply_skill_change(&SkillBankChange::Atomic(changes.to_vec())) else {
        return Ok(());
    };
    for (target, ranges) in line_ranges {
        let Some(parent_file) = parent
            .get(target.skill())
            .and_then(|folder| folder.file(target.path()))
        else {
            return Err(SkillPatchApplicationError::PlanMismatch(format!(
                "line-range edit targets missing parent file {target}"
            )));
        };
        let Some(child_file) = child
            .get(target.skill())
            .and_then(|folder| folder.file(target.path()))
        else {
            return Err(SkillPatchApplicationError::PlanMismatch(format!(
                "line-range edit removed target file {target}"
            )));
        };
        validate_file_preserves_lines(&target, parent_file, child_file, &ranges)?;
    }
    Ok(())
}

fn planned_line_ranges(plan: &SkillPatchPlan) -> BTreeMap<SkillPatchFileRef, Vec<SkillLineRange>> {
    let mut ranges = BTreeMap::new();
    for edit in plan.edits() {
        if let SkillPatchEditKind::Modify {
            range: SkillPatchRange::Lines(range),
        } = edit.kind()
        {
            ranges
                .entry(edit.target().clone())
                .or_insert_with(Vec::new)
                .push(range);
        }
    }
    ranges
}

fn validate_file_preserves_lines(
    target: &SkillPatchFileRef,
    parent_file: &SkillFile,
    child_file: &SkillFile,
    ranges: &[SkillLineRange],
) -> Result<(), SkillPatchApplicationError> {
    if parent_file.permissions() != child_file.permissions() {
        return Err(SkillPatchApplicationError::PlanMismatch(format!(
            "line-range edit changed file permissions for {target}"
        )));
    }

    let child_lines = line_segments(child_file.bytes());
    let mut child_cursor = 0;
    for (line_index, parent_line) in line_segments(parent_file.bytes()).iter().enumerate() {
        let line_number = line_index + 1;
        if line_is_declared(line_number, ranges) {
            continue;
        }
        let Some(relative_match) = child_lines[child_cursor..]
            .iter()
            .position(|child_line| child_line == parent_line)
        else {
            return Err(SkillPatchApplicationError::PlanMismatch(format!(
                "line-range edit for {target} changed parent line {line_number} outside declared ranges"
            )));
        };
        child_cursor += relative_match + 1;
    }
    Ok(())
}

fn line_segments(bytes: &[u8]) -> Vec<&[u8]> {
    bytes.split_inclusive(|byte| *byte == b'\n').collect()
}

fn line_is_declared(line_number: usize, ranges: &[SkillLineRange]) -> bool {
    let line_number = u64::try_from(line_number).unwrap_or(u64::MAX);
    ranges.iter().any(|range| {
        u64::from(range.start()) <= line_number && line_number <= u64::from(range.end())
    })
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

#[allow(clippy::too_many_lines)]
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
            let before = parent
                .get(name)
                .map(leaven_artifact_skill::SkillFolder::entries);
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
