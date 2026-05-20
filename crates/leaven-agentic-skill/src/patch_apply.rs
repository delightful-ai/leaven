//! Validated skill patch application.

use std::{error::Error, fmt};

use leaven_artifact_skill::{SkillBank, SkillBankChange};

use crate::{SkillBankChangeReport, SkillPatchPlan};

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
        let change = SkillBankChange::Atomic(changes.into());
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
    /// The atomic skill-bank change failed and the parent bank was preserved.
    RolledBack(Box<SkillPatchRollback>),
}

impl fmt::Display for SkillPatchApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
