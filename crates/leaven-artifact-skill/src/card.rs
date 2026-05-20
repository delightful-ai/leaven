//! Catalog projections over validated skill folders.

use crate::{SkillDescription, SkillFolder, SkillMetadata, SkillName};

/// Routing catalog entry derived from one validated Agent Skill folder.
///
/// A card carries the validated frontmatter used to decide whether a skill is
/// relevant. It deliberately does not include the skill body, files, utility
/// scores, trigger counts, or optimizer policy state.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillCard {
    name: SkillName,
    description: SkillDescription,
    metadata: SkillMetadata,
}

impl SkillCard {
    /// Builds a card from validated frontmatter fields.
    pub fn new(name: SkillName, description: SkillDescription, metadata: SkillMetadata) -> Self {
        Self {
            name,
            description,
            metadata,
        }
    }

    /// Projects a routing card from an already validated skill folder.
    pub fn from_folder(folder: &SkillFolder) -> Self {
        let manifest = folder.manifest();
        Self::new(
            manifest.name.clone(),
            manifest.description.clone(),
            manifest.metadata.clone(),
        )
    }

    /// Returns the skill name.
    pub fn name(&self) -> &SkillName {
        &self.name
    }

    /// Returns the retrieval and usage description.
    pub fn description(&self) -> &SkillDescription {
        &self.description
    }

    /// Returns the generic frontmatter metadata bag.
    pub fn metadata(&self) -> &SkillMetadata {
        &self.metadata
    }
}
