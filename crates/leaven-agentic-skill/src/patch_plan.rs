//! Guardrails for agent-authored skill patch plans.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use leaven_artifact_skill::{SkillBank, SkillName, SkillPath};

/// One file inside a named skill folder.
#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct SkillPatchFileRef {
    skill: SkillName,
    path: SkillPath,
}

impl SkillPatchFileRef {
    /// Builds a validated skill file reference.
    #[must_use]
    pub const fn new(skill: SkillName, path: SkillPath) -> Self {
        Self { skill, path }
    }

    /// Returns the target skill.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Returns the skill-relative file path.
    #[must_use]
    pub const fn path(&self) -> &SkillPath {
        &self.path
    }
}

impl fmt::Display for SkillPatchFileRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.skill, self.path)
    }
}

/// One-based inclusive line range inside a skill file.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
pub struct SkillLineRange {
    start: u32,
    end: u32,
}

impl SkillLineRange {
    /// Builds a one-based inclusive line range.
    ///
    /// # Errors
    ///
    /// Returns [`SkillPatchPlanError::InvalidLineRange`] when `start` is zero
    /// or when `end` is before `start`.
    pub const fn new(start: u32, end: u32) -> Result<Self, SkillPatchPlanError> {
        if start == 0 || end < start {
            return Err(SkillPatchPlanError::InvalidLineRange { start, end });
        }
        Ok(Self { start, end })
    }

    /// Returns the first one-based line covered by the range.
    #[must_use]
    pub const fn start(self) -> u32 {
        self.start
    }

    /// Returns the final one-based line covered by the range.
    #[must_use]
    pub const fn end(self) -> u32 {
        self.end
    }

    const fn overlaps(self, other: Self) -> bool {
        self.start <= other.end && other.start <= self.end
    }
}

impl fmt::Display for SkillLineRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..={}", self.start, self.end)
    }
}

/// File region touched by a patch edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
pub enum SkillPatchRange {
    /// The edit is file-wide or the precise changed lines are not known.
    WholeFile,
    /// The edit targets a concrete one-based line range.
    Lines(SkillLineRange),
}

impl SkillPatchRange {
    const fn conflicts_with(self, other: Self) -> bool {
        match (self, other) {
            (Self::WholeFile, _) | (_, Self::WholeFile) => true,
            (Self::Lines(left), Self::Lines(right)) => left.overlaps(right),
        }
    }
}

impl fmt::Display for SkillPatchRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WholeFile => f.write_str("whole file"),
            Self::Lines(range) => write!(f, "lines {range}"),
        }
    }
}

/// Number of independent patch observations supporting an edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
pub struct SkillPatchSupport {
    count: u32,
}

impl SkillPatchSupport {
    /// Builds a support-count record.
    ///
    /// # Errors
    ///
    /// Returns [`SkillPatchPlanError::EmptySupport`] when `count` is zero.
    pub const fn new(count: u32) -> Result<Self, SkillPatchPlanError> {
        if count == 0 {
            return Err(SkillPatchPlanError::EmptySupport);
        }
        Ok(Self { count })
    }

    /// Returns the number of independent supporting observations.
    #[must_use]
    pub const fn count(self) -> u32 {
        self.count
    }
}

/// Operation an agent patch wants to perform against a skill file.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
pub enum SkillPatchEditKind {
    /// Modify an existing file.
    Modify { range: SkillPatchRange },
    /// Create a new file inside an existing skill folder.
    CreateFile,
    /// Delete an existing file.
    DeleteFile,
}

impl SkillPatchEditKind {
    const fn conflict_range(self) -> SkillPatchRange {
        match self {
            Self::Modify { range } => range,
            Self::CreateFile | Self::DeleteFile => SkillPatchRange::WholeFile,
        }
    }
}

/// One proposed edit inside a skill patch plan.
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
pub struct SkillPatchPlanEdit {
    target: SkillPatchFileRef,
    kind: SkillPatchEditKind,
    support: SkillPatchSupport,
}

impl SkillPatchPlanEdit {
    /// Builds a modification edit.
    #[must_use]
    pub const fn modify(
        target: SkillPatchFileRef,
        range: SkillPatchRange,
        support: SkillPatchSupport,
    ) -> Self {
        Self {
            target,
            kind: SkillPatchEditKind::Modify { range },
            support,
        }
    }

    /// Builds a create-file edit.
    #[must_use]
    pub const fn create_file(target: SkillPatchFileRef, support: SkillPatchSupport) -> Self {
        Self {
            target,
            kind: SkillPatchEditKind::CreateFile,
            support,
        }
    }

    /// Builds a delete-file edit.
    #[must_use]
    pub const fn delete_file(target: SkillPatchFileRef, support: SkillPatchSupport) -> Self {
        Self {
            target,
            kind: SkillPatchEditKind::DeleteFile,
            support,
        }
    }

    /// Returns the target file.
    #[must_use]
    pub const fn target(&self) -> &SkillPatchFileRef {
        &self.target
    }

    /// Returns the edit kind.
    #[must_use]
    pub const fn kind(&self) -> SkillPatchEditKind {
        self.kind
    }

    /// Returns the support count record.
    #[must_use]
    pub const fn support(&self) -> SkillPatchSupport {
        self.support
    }
}

/// A patch plan validated against a parent [`SkillBank`].
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillPatchPlan {
    edits: Vec<SkillPatchPlanEdit>,
}

impl SkillPatchPlan {
    /// Validates a patch plan against the parent skill bank.
    ///
    /// The validation is intentionally policy-light: it proves the mechanical
    /// guardrails shared by many agent-authored skill-edit systems and leaves
    /// paper-specific merge, deduplication, and prevalence policy to callers.
    ///
    /// # Errors
    ///
    /// Returns [`SkillPatchPlanError`] when the plan is empty, targets an
    /// invalid parent state, has zero-support edits, or contains conflicting
    /// edits to the same file range.
    pub fn validate(
        parent: &SkillBank,
        edits: impl Into<Vec<SkillPatchPlanEdit>>,
    ) -> Result<Self, SkillPatchPlanError> {
        let edits = edits.into();
        if edits.is_empty() {
            return Err(SkillPatchPlanError::EmptyPlan);
        }

        let mut touched: BTreeMap<SkillPatchFileRef, Vec<(usize, SkillPatchRange)>> =
            BTreeMap::new();
        for (index, edit) in edits.iter().enumerate() {
            if edit.support.count == 0 {
                return Err(SkillPatchPlanError::EmptySupportAt { edit_index: index });
            }
            let folder = parent.get(edit.target.skill()).ok_or_else(|| {
                SkillPatchPlanError::MissingSkill {
                    edit_index: index,
                    skill: edit.target.skill().clone(),
                }
            })?;
            let exists = folder.file(edit.target.path()).is_some();
            match edit.kind {
                SkillPatchEditKind::Modify { .. } | SkillPatchEditKind::DeleteFile => {
                    if !exists {
                        return Err(SkillPatchPlanError::MissingFile {
                            edit_index: index,
                            target: edit.target.clone(),
                        });
                    }
                }
                SkillPatchEditKind::CreateFile => {
                    if exists {
                        return Err(SkillPatchPlanError::CreateOverwritesExisting {
                            edit_index: index,
                            target: edit.target.clone(),
                        });
                    }
                }
            }

            let range = edit.kind.conflict_range();
            if let Some(existing) = touched.get(edit.target()) {
                for (other_index, other_range) in existing {
                    if range.conflicts_with(*other_range) {
                        return Err(SkillPatchPlanError::LineRangeConflict {
                            first_index: *other_index,
                            second_index: index,
                            target: edit.target.clone(),
                            first_range: *other_range,
                            second_range: range,
                        });
                    }
                }
            }
            touched
                .entry(edit.target.clone())
                .or_default()
                .push((index, range));
        }

        Ok(Self { edits })
    }

    /// Returns the validated edits.
    #[must_use]
    pub fn edits(&self) -> &[SkillPatchPlanEdit] {
        &self.edits
    }
}

/// Validation failure for a skill patch plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillPatchPlanError {
    /// Patch plan contained no edits.
    EmptyPlan,
    /// Support counts must be positive.
    EmptySupport,
    /// A deserialized edit carried an invalid zero support count.
    EmptySupportAt {
        /// Zero-based edit index.
        edit_index: usize,
    },
    /// Line ranges are one-based and inclusive.
    InvalidLineRange {
        /// Requested start line.
        start: u32,
        /// Requested end line.
        end: u32,
    },
    /// The target skill does not exist in the parent bank.
    MissingSkill {
        /// Zero-based edit index.
        edit_index: usize,
        /// Missing skill name.
        skill: SkillName,
    },
    /// The target file does not exist in the parent bank.
    MissingFile {
        /// Zero-based edit index.
        edit_index: usize,
        /// Missing file target.
        target: SkillPatchFileRef,
    },
    /// Create-file edits must not replace an existing file.
    CreateOverwritesExisting {
        /// Zero-based edit index.
        edit_index: usize,
        /// Existing file target.
        target: SkillPatchFileRef,
    },
    /// Two edits target overlapping regions of the same file.
    LineRangeConflict {
        /// Earlier zero-based edit index.
        first_index: usize,
        /// Later zero-based edit index.
        second_index: usize,
        /// Shared file target.
        target: SkillPatchFileRef,
        /// Earlier range.
        first_range: SkillPatchRange,
        /// Later range.
        second_range: SkillPatchRange,
    },
}

impl fmt::Display for SkillPatchPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPlan => f.write_str("skill patch plan must contain at least one edit"),
            Self::EmptySupport => f.write_str("skill patch edit support count must be positive"),
            Self::EmptySupportAt { edit_index } => {
                write!(f, "skill patch edit {edit_index} has zero support")
            }
            Self::InvalidLineRange { start, end } => write!(
                f,
                "skill patch line range must be one-based and inclusive, got {start}..={end}"
            ),
            Self::MissingSkill { edit_index, skill } => write!(
                f,
                "skill patch edit {edit_index} targets missing skill {skill}"
            ),
            Self::MissingFile { edit_index, target } => write!(
                f,
                "skill patch edit {edit_index} targets missing file {target}"
            ),
            Self::CreateOverwritesExisting { edit_index, target } => write!(
                f,
                "skill patch edit {edit_index} would create existing file {target}"
            ),
            Self::LineRangeConflict {
                first_index,
                second_index,
                target,
                first_range,
                second_range,
            } => write!(
                f,
                "skill patch edits {first_index} and {second_index} conflict on {target}: {first_range} overlaps {second_range}"
            ),
        }
    }
}

impl Error for SkillPatchPlanError {}
