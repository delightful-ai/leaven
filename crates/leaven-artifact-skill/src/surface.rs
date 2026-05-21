//! Edit surfaces over skill banks.

use leaven_kernel::FingerprintBuilder;
use leaven_surface::{EditSurface, Part, SurfaceError, SurfaceFingerprint};

use crate::{
    ParsedSkillMd, SkillBank, SkillBankChange, SkillBody, SkillFile, SkillFolder, SkillManifest,
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
            file: skill_md_file_with_permissions(folder, bytes),
        })
    }
}

/// `SKILL.md` body-level part identity.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SkillBodyPartId {
    /// Skill whose body is selected.
    pub skill: SkillName,
}

/// `SKILL.md` body-level surface over a [`SkillBank`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SkillBodySurface;

/// `SKILL.md` body-level edit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillBodyEdit {
    /// Replace the markdown body while keeping frontmatter fixed.
    Replace(SkillBody),
}

impl EditSurface<SkillBank> for SkillBodySurface {
    type PartId = SkillBodyPartId;
    type Address = String;
    type View<'a> = &'a SkillBody;
    type Edit = SkillBodyEdit;

    fn fingerprint(&self) -> SurfaceFingerprint {
        fingerprint("leaven.skill-body-surface.v1")
    }

    fn parts<'a>(
        &self,
        artifact: &'a SkillBank,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        Ok(artifact
            .folders()
            .iter()
            .map(|(name, folder)| Part {
                id: SkillBodyPartId {
                    skill: name.clone(),
                },
                address: format!("{name}/SKILL.md#body"),
                view: folder.body(),
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
        let SkillBodyEdit::Replace(body) = edit;
        let parsed = ParsedSkillMd {
            manifest: folder.manifest().clone(),
            body,
        };
        let bytes = parsed
            .to_skill_md_bytes()
            .map_err(|error| SurfaceError::Message(error.to_string()))?;
        Ok(SkillBankChange::WriteFile {
            skill: id.skill,
            path: SkillPath::skill_md(),
            file: skill_md_file_with_permissions(folder, bytes),
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

/// Reference-module part identity.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SkillReferencePartId {
    /// Skill containing the reference module.
    pub skill: SkillName,
    /// Direct `references/*.md` path inside the skill folder.
    pub path: SkillPath,
}

/// Reference-module surface over direct `references/*.md` files.
#[derive(Clone, Copy, Debug, Default)]
pub struct SkillReferenceSurface;

/// Reference-module edit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillReferenceEdit {
    /// Replace the reference file.
    Replace(SkillFile),
    /// Remove the reference file.
    Remove,
    /// Rename the reference file to another direct `references/*.md` path.
    Rename(SkillPath),
    /// Toggle the executable bit.
    SetExecutable(bool),
}

impl EditSurface<SkillBank> for SkillReferenceSurface {
    type PartId = SkillReferencePartId;
    type Address = String;
    type View<'a> = &'a SkillFile;
    type Edit = SkillReferenceEdit;

    fn fingerprint(&self) -> SurfaceFingerprint {
        fingerprint("leaven.skill-reference-surface.v1")
    }

    fn parts<'a>(
        &self,
        artifact: &'a SkillBank,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        let mut parts = Vec::new();
        for (skill, folder) in artifact.folders() {
            for (path, file) in folder.entries() {
                if is_direct_reference_markdown(path) {
                    parts.push(Part {
                        id: SkillReferencePartId {
                            skill: skill.clone(),
                            path: path.clone(),
                        },
                        address: format!("{skill}/{path}"),
                        view: file,
                    });
                }
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
        if !is_direct_reference_markdown(&id.path) || folder.file(&id.path).is_none() {
            return Err(SurfaceError::UnknownPart);
        }
        Ok(match edit {
            SkillReferenceEdit::Replace(file) => SkillBankChange::WriteFile {
                skill: id.skill,
                path: id.path,
                file,
            },
            SkillReferenceEdit::Remove => SkillBankChange::RemoveFile {
                skill: id.skill,
                path: id.path,
            },
            SkillReferenceEdit::Rename(to) => {
                if !is_direct_reference_markdown(&to) {
                    return Err(SurfaceError::Message(
                        "skill reference surface only accepts direct references/*.md paths"
                            .to_owned(),
                    ));
                }
                SkillBankChange::RenameFile {
                    skill: id.skill,
                    from: id.path,
                    to,
                }
            }
            SkillReferenceEdit::SetExecutable(executable) => SkillBankChange::SetExecutable {
                skill: id.skill,
                path: id.path,
                executable,
            },
        })
    }
}

fn skill_md_file_with_permissions(folder: &SkillFolder, bytes: Vec<u8>) -> SkillFile {
    SkillFile::with_permissions(
        bytes,
        folder
            .file(&SkillPath::skill_md())
            .map(SkillFile::permissions)
            .unwrap_or_default(),
    )
}

fn is_direct_reference_markdown(path: &SkillPath) -> bool {
    let Some(rest) = path.as_str().strip_prefix("references/") else {
        return false;
    };
    let Some(stem) = rest.strip_suffix(".md") else {
        return false;
    };
    !stem.is_empty() && !stem.contains('/')
}

fn fingerprint(name: &str) -> SurfaceFingerprint {
    let mut builder = FingerprintBuilder::new();
    builder.update(name.as_bytes());
    SurfaceFingerprint(builder.finish())
}
