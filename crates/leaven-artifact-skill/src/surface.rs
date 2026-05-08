//! Edit surfaces over skill banks.

use leaven_kernel::FingerprintBuilder;
use leaven_surface::{EditSurface, Part, SurfaceError, SurfaceFingerprint};

use crate::{
    ParsedSkillMd, SkillBank, SkillBankChange, SkillFile, SkillFolder, SkillManifest,
    SkillMetadata, SkillName, SkillPath,
};

/// Folder-level surface over a [`SkillBank`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SkillFolderSurface;

/// Folder-level edit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillFolderEdit {
    /// Replace the whole folder.
    Replace(SkillFolder),
    /// Remove the folder.
    Remove,
    /// Rename the folder and `SKILL.md` name together.
    Rename(SkillName),
}

impl EditSurface<SkillBank> for SkillFolderSurface {
    type PartId = SkillName;
    type Address = String;
    type View<'a> = &'a SkillFolder;
    type Edit = SkillFolderEdit;

    fn fingerprint(&self) -> SurfaceFingerprint {
        fingerprint("leaven.skill-folder-surface.v1")
    }

    fn parts<'a>(
        &self,
        artifact: &'a SkillBank,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        Ok(artifact
            .folders()
            .iter()
            .map(|(name, folder)| Part {
                id: name.clone(),
                address: name.to_string(),
                view: folder,
            })
            .collect())
    }

    fn change_part(
        &self,
        artifact: &SkillBank,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<SkillBankChange, SurfaceError> {
        if artifact.get(&id).is_none() {
            return Err(SurfaceError::UnknownPart);
        }
        match edit {
            SkillFolderEdit::Replace(folder) => {
                Ok(SkillBankChange::ReplaceSkill { name: id, folder })
            }
            SkillFolderEdit::Remove => Ok(SkillBankChange::RemoveSkill { name: id }),
            SkillFolderEdit::Rename(to) => Ok(SkillBankChange::RenameSkill { from: id, to }),
        }
    }
}

/// Manifest-level part identity.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SkillManifestPartId {
    /// Skill whose manifest is selected.
    pub skill: SkillName,
}

/// Manifest-level surface over a [`SkillBank`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SkillManifestSurface;

/// Manifest-level edit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillManifestEdit {
    /// Replace the description and generic metadata while keeping the skill
    /// name fixed.
    Replace {
        /// Replacement description.
        description: crate::SkillDescription,
        /// Replacement generic metadata bag.
        metadata: SkillMetadata,
    },
}

impl EditSurface<SkillBank> for SkillManifestSurface {
    type PartId = SkillManifestPartId;
    type Address = String;
    type View<'a> = &'a SkillManifest;
    type Edit = SkillManifestEdit;

    fn fingerprint(&self) -> SurfaceFingerprint {
        fingerprint("leaven.skill-manifest-surface.v1")
    }

    fn parts<'a>(
        &self,
        artifact: &'a SkillBank,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        Ok(artifact
            .folders()
            .iter()
            .map(|(name, folder)| Part {
                id: SkillManifestPartId {
                    skill: name.clone(),
                },
                address: format!("{name}/SKILL.md#frontmatter"),
                view: folder.manifest(),
            })
            .collect())
    }

    fn change_part(
        &self,
        artifact: &SkillBank,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<SkillBankChange, SurfaceError> {
        let folder = artifact.get(&id.skill).ok_or(SurfaceError::UnknownPart)?;
        let SkillManifestEdit::Replace {
            description,
            metadata,
        } = edit;
        let parsed = ParsedSkillMd {
            manifest: SkillManifest::new(id.skill.clone(), description, metadata),
            body: folder.body().clone(),
        };
        let bytes = parsed
            .to_skill_md_bytes()
            .map_err(|error| SurfaceError::Message(error.to_string()))?;
        Ok(SkillBankChange::WriteFile {
            skill: id.skill,
            path: SkillPath::skill_md(),
            file: SkillFile::with_permissions(
                bytes,
                folder
                    .file(&SkillPath::skill_md())
                    .map(SkillFile::permissions)
                    .unwrap_or_default(),
            ),
        })
    }
}

/// File-level part identity.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SkillFilePartId {
    /// Skill containing the file.
    pub skill: SkillName,
    /// Path inside the skill folder.
    pub path: SkillPath,
}

/// File-level surface over a [`SkillBank`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SkillFileSurface;

/// File-level edit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillFileEdit {
    /// Replace the file.
    Replace(SkillFile),
    /// Remove the file.
    Remove,
    /// Rename the file.
    Rename(SkillPath),
    /// Toggle the executable bit.
    SetExecutable(bool),
}

impl EditSurface<SkillBank> for SkillFileSurface {
    type PartId = SkillFilePartId;
    type Address = String;
    type View<'a> = &'a SkillFile;
    type Edit = SkillFileEdit;

    fn fingerprint(&self) -> SurfaceFingerprint {
        fingerprint("leaven.skill-file-surface.v1")
    }

    fn parts<'a>(
        &self,
        artifact: &'a SkillBank,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        let mut parts = Vec::new();
        for (skill, folder) in artifact.folders() {
            for (path, file) in folder.entries() {
                parts.push(Part {
                    id: SkillFilePartId {
                        skill: skill.clone(),
                        path: path.clone(),
                    },
                    address: format!("{skill}/{path}"),
                    view: file,
                });
            }
        }
        Ok(parts)
    }

    fn change_part(
        &self,
        artifact: &SkillBank,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<SkillBankChange, SurfaceError> {
        let folder = artifact.get(&id.skill).ok_or(SurfaceError::UnknownPart)?;
        if folder.file(&id.path).is_none() {
            return Err(SurfaceError::UnknownPart);
        }
        Ok(match edit {
            SkillFileEdit::Replace(file) => SkillBankChange::WriteFile {
                skill: id.skill,
                path: id.path,
                file,
            },
            SkillFileEdit::Remove => SkillBankChange::RemoveFile {
                skill: id.skill,
                path: id.path,
            },
            SkillFileEdit::Rename(to) => SkillBankChange::RenameFile {
                skill: id.skill,
                from: id.path,
                to,
            },
            SkillFileEdit::SetExecutable(executable) => SkillBankChange::SetExecutable {
                skill: id.skill,
                path: id.path,
                executable,
            },
        })
    }
}

fn fingerprint(name: &str) -> SurfaceFingerprint {
    let mut builder = FingerprintBuilder::new();
    builder.update(name.as_bytes());
    SurfaceFingerprint(builder.finish())
}
