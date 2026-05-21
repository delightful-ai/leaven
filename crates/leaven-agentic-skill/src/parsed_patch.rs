//! Parsed skill patch operations lowered into validated plans and changes.

use std::{error::Error, fmt};

use leaven_artifact_skill::{SkillBank, SkillBankChange, SkillFile};

use crate::{
    SkillPatchFileRef, SkillPatchPlan, SkillPatchPlanEdit, SkillPatchPlanError, SkillPatchRange,
    SkillPatchSupport, SkillReferencePath,
};

/// Paper-neutral parsed skill patch document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillParsedPatchDocument {
    operations: Vec<SkillParsedPatchOperation>,
}

impl SkillParsedPatchDocument {
    /// Builds a document from already-parsed patch operations.
    #[must_use]
    pub fn new(operations: impl Into<Vec<SkillParsedPatchOperation>>) -> Self {
        Self {
            operations: operations.into(),
        }
    }

    /// Validates operations against the parent bank and lowers them to changes.
    pub fn validate_against(
        self,
        parent: &SkillBank,
    ) -> Result<SkillParsedPatch, SkillParsedPatchError> {
        let edits = self
            .operations
            .iter()
            .map(SkillParsedPatchOperation::plan_edit)
            .cloned()
            .collect::<Vec<_>>();
        let plan = SkillPatchPlan::validate(parent, edits).map_err(SkillParsedPatchError::Plan)?;
        let changes = self
            .operations
            .into_iter()
            .map(SkillParsedPatchOperation::into_change)
            .collect();
        Ok(SkillParsedPatch { plan, changes })
    }
}

/// One parsed patch operation with enough payload to lower into artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillParsedPatchOperation {
    plan_edit: SkillPatchPlanEdit,
    change: SkillBankChange,
}

impl SkillParsedPatchOperation {
    /// Writes a replacement for an existing file.
    #[must_use]
    pub fn modify_file(
        target: SkillPatchFileRef,
        range: SkillPatchRange,
        support: SkillPatchSupport,
        file: SkillFile,
    ) -> Self {
        let change = SkillBankChange::WriteFile {
            skill: target.skill().clone(),
            path: target.path().clone(),
            file,
        };
        Self {
            plan_edit: SkillPatchPlanEdit::modify(target, range, support),
            change,
        }
    }

    /// Creates a new file.
    #[must_use]
    pub fn create_file(
        target: SkillPatchFileRef,
        support: SkillPatchSupport,
        file: SkillFile,
    ) -> Self {
        let change = SkillBankChange::WriteFile {
            skill: target.skill().clone(),
            path: target.path().clone(),
            file,
        };
        Self {
            plan_edit: SkillPatchPlanEdit::create_file(target, support),
            change,
        }
    }

    /// Deletes an existing file.
    #[must_use]
    pub fn delete_file(target: SkillPatchFileRef, support: SkillPatchSupport) -> Self {
        let change = SkillBankChange::RemoveFile {
            skill: target.skill().clone(),
            path: target.path().clone(),
        };
        Self {
            plan_edit: SkillPatchPlanEdit::delete_file(target, support),
            change,
        }
    }

    /// Records `references/*.md` links inserted by this operation.
    #[must_use]
    pub fn with_reference_links(
        mut self,
        links: impl IntoIterator<Item = SkillReferencePath>,
    ) -> Self {
        self.plan_edit = self.plan_edit.with_reference_links(links);
        self
    }

    /// Extracts and records `references/*.md` links from markdown-ish content.
    #[must_use]
    pub fn with_reference_links_from_text(self, text: &str) -> Self {
        self.with_reference_links(SkillReferencePath::extract_from_text(text))
    }

    /// Returns the plan edit for this operation.
    #[must_use]
    pub const fn plan_edit(&self) -> &SkillPatchPlanEdit {
        &self.plan_edit
    }

    /// Returns the artifact-native change for this operation.
    #[must_use]
    pub const fn change(&self) -> &SkillBankChange {
        &self.change
    }

    fn into_change(self) -> SkillBankChange {
        self.change
    }
}

/// Parsed patch lowered into a validated plan and artifact-native changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillParsedPatch {
    plan: SkillPatchPlan,
    changes: Vec<SkillBankChange>,
}

impl SkillParsedPatch {
    /// Validated patch plan.
    #[must_use]
    pub const fn plan(&self) -> &SkillPatchPlan {
        &self.plan
    }

    /// Concrete skill-bank changes corresponding to the plan.
    #[must_use]
    pub fn changes(&self) -> &[SkillBankChange] {
        &self.changes
    }

    /// Consumes the parsed patch into plan and changes.
    #[must_use]
    pub fn into_parts(self) -> (SkillPatchPlan, Vec<SkillBankChange>) {
        (self.plan, self.changes)
    }
}

/// Parsed patch lowering failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillParsedPatchError {
    /// Plan validation failed.
    Plan(SkillPatchPlanError),
}

impl fmt::Display for SkillParsedPatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plan(error) => write!(formatter, "parsed skill patch plan is invalid: {error}"),
        }
    }
}

impl Error for SkillParsedPatchError {}
