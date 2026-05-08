//! Agent Skills artifact support.

mod bank;
mod change;
mod error;
mod file;
mod folder;
mod manifest;
mod metadata;
mod path;
mod surface;

pub use bank::SkillBank;
pub use change::SkillBankChange;
pub use error::{
    SkillBankError, SkillDescriptionError, SkillNameError, SkillParseError, SkillPathError,
};
pub use file::{SkillFile, SkillFilePermissions};
pub use folder::SkillFolder;
pub use manifest::{ParsedSkillMd, SkillBody, SkillDescription, SkillManifest, SkillName};
pub use metadata::{SkillMetadata, SkillMetadataValue};
pub use path::SkillPath;
pub use surface::{
    SkillFileEdit, SkillFilePartId, SkillFileSurface, SkillFolderEdit, SkillFolderSurface,
    SkillManifestEdit, SkillManifestPartId, SkillManifestSurface,
};
