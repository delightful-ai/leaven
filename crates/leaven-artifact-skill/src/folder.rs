//! Validated skill folders.

use std::collections::BTreeMap;

use crate::{
    ParsedSkillMd, SkillBankError, SkillBody, SkillFile, SkillManifest, SkillName, SkillPath,
};

/// One validated Agent Skill folder.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillFolder {
    name: SkillName,
    manifest: SkillManifest,
    body: SkillBody,
    entries: BTreeMap<SkillPath, SkillFile>,
}

impl SkillFolder {
    /// Builds a skill folder from skill-root-relative entries.
    ///
    /// # Errors
    ///
    /// Returns [`SkillBankError`] when `SKILL.md` is missing, invalid, or
    /// declares a name that does not match `name`.
    pub fn from_entries(
        name: SkillName,
        entries: BTreeMap<SkillPath, SkillFile>,
    ) -> Result<Self, SkillBankError> {
        let parsed = parse_skill_md_for_folder(&name, &entries)?;
        if parsed.manifest.name != name {
            return Err(SkillBankError::NameMismatch {
                folder: name.to_string(),
                manifest_name: parsed.manifest.name.to_string(),
            });
        }
        Ok(Self {
            name,
            manifest: parsed.manifest,
            body: parsed.body,
            entries,
        })
    }

    /// Returns the skill name.
    pub fn name(&self) -> &SkillName {
        &self.name
    }

    /// Returns parsed `SKILL.md` frontmatter.
    pub fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    /// Returns parsed `SKILL.md` markdown body.
    pub fn body(&self) -> &SkillBody {
        &self.body
    }

    /// Returns all files in the folder.
    pub fn entries(&self) -> &BTreeMap<SkillPath, SkillFile> {
        &self.entries
    }

    /// Returns one file in the folder.
    pub fn file(&self, path: &SkillPath) -> Option<&SkillFile> {
        self.entries.get(path)
    }

    /// Checks folder invariants by reparsing `SKILL.md`.
    ///
    /// # Errors
    ///
    /// Returns [`SkillBankError`] if the folder is no longer internally
    /// consistent.
    pub fn validate(&self) -> Result<(), SkillBankError> {
        let parsed = parse_skill_md_for_folder(&self.name, &self.entries)?;
        if parsed.manifest.name != self.name {
            return Err(SkillBankError::NameMismatch {
                folder: self.name.to_string(),
                manifest_name: parsed.manifest.name.to_string(),
            });
        }
        Ok(())
    }

    pub(crate) fn with_entries(
        &self,
        entries: BTreeMap<SkillPath, SkillFile>,
    ) -> Result<Self, SkillBankError> {
        Self::from_entries(self.name.clone(), entries)
    }

    pub(crate) fn rename_to(&self, to: SkillName) -> Result<Self, SkillBankError> {
        let mut parsed = ParsedSkillMd {
            manifest: self.manifest.clone(),
            body: self.body.clone(),
        };
        parsed.manifest.name = to.clone();
        let mut entries = self.entries.clone();
        let skill_md_path = SkillPath::skill_md();
        let old_permissions = entries
            .get(&skill_md_path)
            .map(SkillFile::permissions)
            .ok_or_else(|| SkillBankError::MissingSkillMd {
                skill: self.name.to_string(),
            })?;
        let bytes = parsed
            .to_skill_md_bytes()
            .map_err(SkillBankError::RenderSkillMd)?;
        entries.insert(
            skill_md_path,
            SkillFile::with_permissions(bytes, old_permissions),
        );
        Self::from_entries(to, entries)
    }
}

fn parse_skill_md_for_folder(
    name: &SkillName,
    entries: &BTreeMap<SkillPath, SkillFile>,
) -> Result<ParsedSkillMd, SkillBankError> {
    let skill_md_path = SkillPath::skill_md();
    let file = entries
        .get(&skill_md_path)
        .ok_or_else(|| SkillBankError::MissingSkillMd {
            skill: name.to_string(),
        })?;
    ParsedSkillMd::parse(file.bytes()).map_err(|source| SkillBankError::InvalidSkillMd {
        skill: name.to_string(),
        source,
    })
}
