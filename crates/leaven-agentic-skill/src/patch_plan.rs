//! Guardrails for agent-authored skill patch plans.

mod reference;

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use leaven_artifact_skill::{SkillBank, SkillName, SkillPath};

pub use reference::SkillReferencePath;

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
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Deserialize, serde::Serialize,
)]
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
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Deserialize, serde::Serialize,
)]
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
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Deserialize, serde::Serialize,
)]
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
    reference_links: Vec<SkillReferencePath>,
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
            reference_links: Vec::new(),
        }
    }

    /// Builds a create-file edit.
    #[must_use]
    pub const fn create_file(target: SkillPatchFileRef, support: SkillPatchSupport) -> Self {
        Self {
            target,
            kind: SkillPatchEditKind::CreateFile,
            support,
            reference_links: Vec::new(),
        }
    }

    /// Builds a delete-file edit.
    #[must_use]
    pub const fn delete_file(target: SkillPatchFileRef, support: SkillPatchSupport) -> Self {
        Self {
            target,
            kind: SkillPatchEditKind::DeleteFile,
            support,
            reference_links: Vec::new(),
        }
    }

    /// Records `references/*.md` links inserted by this edit.
    #[must_use]
    pub fn with_reference_links(
        mut self,
        links: impl IntoIterator<Item = SkillReferencePath>,
    ) -> Self {
        self.reference_links = links.into_iter().collect();
        self
    }

    /// Extracts and records `references/*.md` links from markdown-ish content.
    #[must_use]
    pub fn with_reference_links_from_text(self, text: &str) -> Self {
        self.with_reference_links(SkillReferencePath::extract_from_text(text))
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

    /// Returns the reference links this edit inserts.
    #[must_use]
    pub fn reference_links(&self) -> &[SkillReferencePath] {
        &self.reference_links
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
    #[allow(clippy::too_many_lines)]
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
        let mut created_references: BTreeMap<(SkillName, SkillReferencePath), usize> =
            BTreeMap::new();
        let mut deleted_references: BTreeMap<(SkillName, SkillReferencePath), usize> =
            BTreeMap::new();
        let mut linked_references: BTreeMap<(SkillName, SkillReferencePath), Vec<usize>> =
            BTreeMap::new();
        for (index, edit) in edits.iter().enumerate() {
            validate_deserialized_edit_fields(index, edit)?;
            if edit.support.count == 0 {
                return Err(SkillPatchPlanError::EmptySupportAt { edit_index: index });
            }
            if !edit.reference_links.is_empty() && !edit.target.path().is_skill_md() {
                return Err(SkillPatchPlanError::ReferenceLinkOutsideSkillMd {
                    edit_index: index,
                    target: edit.target.clone(),
                });
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
            if matches!(edit.kind, SkillPatchEditKind::CreateFile) {
                if let Ok(reference) = SkillReferencePath::new(edit.target.path().clone()) {
                    created_references.insert((edit.target.skill().clone(), reference), index);
                }
            }
            if matches!(edit.kind, SkillPatchEditKind::DeleteFile) {
                if let Ok(reference) = SkillReferencePath::new(edit.target.path().clone()) {
                    deleted_references.insert((edit.target.skill().clone(), reference), index);
                }
            }
            for reference in &edit.reference_links {
                let key = (edit.target.skill().clone(), reference.clone());
                linked_references.entry(key).or_default().push(index);
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

        for ((skill, reference), link_indexes) in &linked_references {
            if let Some(delete_index) = deleted_references.get(&(skill.clone(), reference.clone()))
            {
                return Err(SkillPatchPlanError::LinkedReferenceDeleted {
                    link_edit_index: link_indexes[0],
                    delete_edit_index: *delete_index,
                    skill: skill.clone(),
                    path: reference.clone(),
                });
            }
            if parent
                .get(skill)
                .and_then(|folder| folder.file(reference.path()))
                .is_none()
                && !created_references.contains_key(&(skill.clone(), reference.clone()))
            {
                return Err(SkillPatchPlanError::MissingReferenceCreate {
                    edit_index: link_indexes[0],
                    skill: skill.clone(),
                    path: reference.clone(),
                });
            }
        }

        for ((skill, reference), create_index) in &created_references {
            if !linked_references.contains_key(&(skill.clone(), reference.clone())) {
                return Err(SkillPatchPlanError::UnlinkedReferenceCreate {
                    edit_index: *create_index,
                    target: SkillPatchFileRef::new(skill.clone(), reference.path().clone()),
                });
            }
        }

        Ok(Self { edits })
    }

    /// Returns the validated edits.
    #[must_use]
    pub fn edits(&self) -> &[SkillPatchPlanEdit] {
        &self.edits
    }
}

fn validate_deserialized_edit_fields(
    edit_index: usize,
    edit: &SkillPatchPlanEdit,
) -> Result<(), SkillPatchPlanError> {
    if SkillName::new(edit.target.skill().as_str()).is_err() {
        return Err(SkillPatchPlanError::InvalidSkillName {
            edit_index,
            skill: edit.target.skill().clone(),
        });
    }
    if SkillPath::new(edit.target.path().as_str()).is_err() {
        return Err(SkillPatchPlanError::InvalidTargetPath {
            edit_index,
            target: edit.target.clone(),
        });
    }
    if let SkillPatchEditKind::Modify {
        range: SkillPatchRange::Lines(range),
    } = edit.kind
    {
        SkillLineRange::new(range.start(), range.end())?;
    }
    for reference in &edit.reference_links {
        if SkillPath::new(reference.path().as_str()).is_err() {
            return Err(SkillPatchPlanError::InvalidReferencePath {
                path: reference.path().clone(),
            });
        }
        SkillReferencePath::new(reference.path().clone())?;
    }
    Ok(())
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
    /// Reference links must point at `references/*.md`.
    InvalidReferencePath {
        /// Invalid reference path.
        path: SkillPath,
    },
    /// Deserialized target skill names must still satisfy skill-name rules.
    InvalidSkillName {
        /// Zero-based edit index.
        edit_index: usize,
        /// Invalid skill name.
        skill: SkillName,
    },
    /// Deserialized target paths must still satisfy skill-path rules.
    InvalidTargetPath {
        /// Zero-based edit index.
        edit_index: usize,
        /// Invalid file target.
        target: SkillPatchFileRef,
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
    /// Reference links are only valid when inserted by `SKILL.md` edits.
    ReferenceLinkOutsideSkillMd {
        /// Zero-based edit index.
        edit_index: usize,
        /// File edit that declared reference links.
        target: SkillPatchFileRef,
    },
    /// A `SKILL.md` edit links to a missing reference without creating it.
    MissingReferenceCreate {
        /// Zero-based edit index for the link edit.
        edit_index: usize,
        /// Skill containing the link.
        skill: SkillName,
        /// Missing reference path.
        path: SkillReferencePath,
    },
    /// A `references/*.md` create edit is not linked from `SKILL.md`.
    UnlinkedReferenceCreate {
        /// Zero-based edit index for the create edit.
        edit_index: usize,
        /// Unlinked reference file.
        target: SkillPatchFileRef,
    },
    /// A `SKILL.md` edit links to a reference deleted by the same plan.
    LinkedReferenceDeleted {
        /// Zero-based edit index for the link edit.
        link_edit_index: usize,
        /// Zero-based edit index for the delete edit.
        delete_edit_index: usize,
        /// Skill containing the link and deleted reference.
        skill: SkillName,
        /// Deleted reference path.
        path: SkillReferencePath,
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
            Self::InvalidReferencePath { path } => {
                write!(
                    f,
                    "skill reference path must match references/*.md, got {path}"
                )
            }
            Self::InvalidSkillName { edit_index, skill } => write!(
                f,
                "skill patch edit {edit_index} targets invalid skill name {skill}"
            ),
            Self::InvalidTargetPath { edit_index, target } => write!(
                f,
                "skill patch edit {edit_index} targets invalid skill path {target}"
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
            Self::ReferenceLinkOutsideSkillMd { edit_index, target } => write!(
                f,
                "skill patch edit {edit_index} declares reference links outside SKILL.md: {target}"
            ),
            Self::MissingReferenceCreate {
                edit_index,
                skill,
                path,
            } => write!(
                f,
                "skill patch edit {edit_index} links to missing reference {skill}/{path} without a matching create"
            ),
            Self::UnlinkedReferenceCreate { edit_index, target } => write!(
                f,
                "skill patch edit {edit_index} creates unlinked reference file {target}"
            ),
            Self::LinkedReferenceDeleted {
                link_edit_index,
                delete_edit_index,
                skill,
                path,
            } => write!(
                f,
                "skill patch edit {link_edit_index} links to {skill}/{path}, but edit {delete_edit_index} deletes it"
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
