//! Content-addressed skill banks.

use std::collections::BTreeMap;

use leaven_core::{Artifact, ArtifactIdentity, CacheIdentity, ContentAddressed};
use leaven_kernel::ContentId;

use crate::{SkillBankChange, SkillBankError, SkillFile, SkillFolder, SkillName, SkillPath};

/// A validated set of Agent Skills.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillBank {
    folders: BTreeMap<SkillName, SkillFolder>,
}

impl SkillBank {
    /// Constructs a skill bank from validated folders.
    ///
    /// # Errors
    ///
    /// Returns [`SkillBankError::DuplicateSkillName`] when two folders have the
    /// same name.
    pub fn from_folders(
        folders: impl IntoIterator<Item = SkillFolder>,
    ) -> Result<Self, SkillBankError> {
        let mut bank = Self::default();
        for folder in folders {
            let name = folder.name().clone();
            if bank.folders.insert(name.clone(), folder).is_some() {
                return Err(SkillBankError::DuplicateSkillName {
                    name: name.to_string(),
                });
            }
        }
        bank.validate()?;
        Ok(bank)
    }

    /// Returns all folders keyed by skill name.
    pub fn folders(&self) -> &BTreeMap<SkillName, SkillFolder> {
        &self.folders
    }

    /// Returns one skill folder.
    pub fn get(&self, name: &SkillName) -> Option<&SkillFolder> {
        self.folders.get(name)
    }

    /// Returns true when no skills are present.
    pub fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }

    /// Checks all bank invariants.
    ///
    /// # Errors
    ///
    /// Returns [`SkillBankError`] when any contained folder is invalid or keyed
    /// under the wrong skill name.
    pub fn validate(&self) -> Result<(), SkillBankError> {
        for (name, folder) in &self.folders {
            if name != folder.name() {
                return Err(SkillBankError::NameMismatch {
                    folder: name.to_string(),
                    manifest_name: folder.name().to_string(),
                });
            }
            folder.validate()?;
        }
        Ok(())
    }

    /// Applies a skill-bank change functionally.
    ///
    /// # Errors
    ///
    /// Returns [`SkillBankError`] when the change cannot be applied or the
    /// resulting bank would be invalid.
    pub fn apply_skill_change(&self, change: &SkillBankChange) -> Result<Self, SkillBankError> {
        let mut next = self.clone();
        next.apply_raw(change)?;
        next.validate()?;
        Ok(next)
    }

    fn apply_raw(&mut self, change: &SkillBankChange) -> Result<(), SkillBankError> {
        match change {
            SkillBankChange::CreateSkill { folder } => self.create_skill(folder),
            SkillBankChange::ReplaceSkill { name, folder } => self.replace_skill(name, folder),
            SkillBankChange::RemoveSkill { name } => self.remove_skill(name),
            SkillBankChange::RenameSkill { from, to } => self.rename_skill(from, to),
            SkillBankChange::WriteFile { skill, path, file } => self.write_file(skill, path, file),
            SkillBankChange::RemoveFile { skill, path } => self.remove_file(skill, path),
            SkillBankChange::RenameFile { skill, from, to } => self.rename_file(skill, from, to),
            SkillBankChange::SetExecutable {
                skill,
                path,
                executable,
            } => self.set_executable(skill, path, *executable),
            SkillBankChange::Atomic(changes) => {
                for change in changes {
                    self.apply_raw(change)?;
                }
                Ok(())
            }
        }
    }

    fn create_skill(&mut self, folder: &SkillFolder) -> Result<(), SkillBankError> {
        let name = folder.name().clone();
        if self.folders.contains_key(&name) {
            return Err(SkillBankError::SkillAlreadyExists {
                name: name.to_string(),
            });
        }
        self.folders.insert(name, folder.clone());
        Ok(())
    }

    fn replace_skill(
        &mut self,
        name: &SkillName,
        folder: &SkillFolder,
    ) -> Result<(), SkillBankError> {
        if folder.name() != name {
            return Err(SkillBankError::NameMismatch {
                folder: name.to_string(),
                manifest_name: folder.name().to_string(),
            });
        }
        let slot = self
            .folders
            .get_mut(name)
            .ok_or_else(|| missing_skill(name))?;
        *slot = folder.clone();
        Ok(())
    }

    fn remove_skill(&mut self, name: &SkillName) -> Result<(), SkillBankError> {
        self.folders
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| missing_skill(name))
    }

    fn rename_skill(&mut self, from: &SkillName, to: &SkillName) -> Result<(), SkillBankError> {
        if self.folders.contains_key(to) {
            return Err(SkillBankError::SkillAlreadyExists {
                name: to.to_string(),
            });
        }
        let folder = self
            .folders
            .remove(from)
            .ok_or_else(|| missing_skill(from))?;
        let renamed = folder.rename_to(to.clone())?;
        self.folders.insert(to.clone(), renamed);
        Ok(())
    }

    fn write_file(
        &mut self,
        skill: &SkillName,
        path: &SkillPath,
        file: &SkillFile,
    ) -> Result<(), SkillBankError> {
        let updated = {
            let folder = self.folder(skill)?;
            let mut entries = folder.entries().clone();
            entries.insert(path.clone(), file.clone());
            folder.with_entries(entries)?
        };
        self.folders.insert(skill.clone(), updated);
        Ok(())
    }

    fn remove_file(&mut self, skill: &SkillName, path: &SkillPath) -> Result<(), SkillBankError> {
        let updated = {
            let folder = self.folder(skill)?;
            let mut entries = folder.entries().clone();
            entries
                .remove(path)
                .ok_or_else(|| missing_file(skill, path))?;
            folder.with_entries(entries)?
        };
        self.folders.insert(skill.clone(), updated);
        Ok(())
    }

    fn rename_file(
        &mut self,
        skill: &SkillName,
        from: &SkillPath,
        to: &SkillPath,
    ) -> Result<(), SkillBankError> {
        let updated = {
            let folder = self.folder(skill)?;
            let mut entries = folder.entries().clone();
            if entries.contains_key(to) {
                return Err(SkillBankError::FileAlreadyExists {
                    skill: skill.to_string(),
                    path: to.to_string(),
                });
            }
            let file = entries
                .remove(from)
                .ok_or_else(|| missing_file(skill, from))?;
            entries.insert(to.clone(), file);
            folder.with_entries(entries)?
        };
        self.folders.insert(skill.clone(), updated);
        Ok(())
    }

    fn set_executable(
        &mut self,
        skill: &SkillName,
        path: &SkillPath,
        executable: bool,
    ) -> Result<(), SkillBankError> {
        let updated = {
            let folder = self.folder(skill)?;
            let mut entries = folder.entries().clone();
            let file = entries
                .get_mut(path)
                .ok_or_else(|| missing_file(skill, path))?;
            file.set_executable(executable);
            folder.with_entries(entries)?
        };
        self.folders.insert(skill.clone(), updated);
        Ok(())
    }

    fn folder(&self, skill: &SkillName) -> Result<&SkillFolder, SkillBankError> {
        self.folders.get(skill).ok_or_else(|| missing_skill(skill))
    }
}

impl Artifact for SkillBank {
    type Change = SkillBankChange;
    type ApplyError = SkillBankError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(self.content_id())
    }

    fn cache_identity(&self) -> Option<CacheIdentity> {
        Some(CacheIdentity::Content(self.content_id()))
    }

    fn validate(&self) -> Result<(), Self::ApplyError> {
        Self::validate(self)
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        self.apply_skill_change(change)
    }
}

impl ContentAddressed for SkillBank {
    fn content_id(&self) -> ContentId {
        let mut hasher = blake3::Hasher::new();
        feed(&mut hasher, b"leaven.skill-bank.v1");
        for (name, folder) in &self.folders {
            feed(&mut hasher, name.as_str().as_bytes());
            for (path, file) in folder.entries() {
                feed(&mut hasher, path.as_str().as_bytes());
                feed(&mut hasher, &[u8::from(file.permissions().executable)]);
                feed(&mut hasher, file.bytes());
            }
        }
        ContentId::from_bytes(*hasher.finalize().as_bytes())
    }
}

fn missing_skill(name: &SkillName) -> SkillBankError {
    SkillBankError::MissingSkill {
        name: name.to_string(),
    }
}

fn missing_file(skill: &SkillName, path: &SkillPath) -> SkillBankError {
    SkillBankError::MissingFile {
        skill: skill.to_string(),
        path: path.to_string(),
    }
}

fn feed(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{SkillFile, SkillFolder};

    fn folder(name: &str) -> SkillFolder {
        let name = SkillName::new(name).unwrap();
        let mut entries = BTreeMap::new();
        entries.insert(
            SkillPath::skill_md(),
            SkillFile::text(format!(
                "---\nname: {name}\ndescription: Use when testing private bank invariants.\n---\nBody.\n"
            )),
        );
        SkillFolder::from_entries(name, entries).unwrap()
    }

    #[test]
    fn validate_rejects_folder_key_that_disagrees_with_manifest_name() {
        let mut folders = BTreeMap::new();
        folders.insert(
            SkillName::new("declared-name").unwrap(),
            folder("actual-name"),
        );
        let bank = SkillBank { folders };

        assert!(matches!(
            bank.validate().unwrap_err(),
            SkillBankError::NameMismatch {
                folder,
                manifest_name
            } if folder == "declared-name" && manifest_name == "actual-name"
        ));
    }
}
