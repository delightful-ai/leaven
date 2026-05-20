//! Skill artifact errors.

/// Invalid Agent Skill name.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum SkillNameError {
    /// Names must not be empty.
    #[error("skill name is empty")]
    Empty,
    /// Names are limited to 64 characters.
    #[error("skill name exceeds 64 characters")]
    TooLong,
    /// Names must not start with a hyphen.
    #[error("skill name must not start with '-'")]
    StartsWithHyphen,
    /// Names must not end with a hyphen.
    #[error("skill name must not end with '-'")]
    EndsWithHyphen,
    /// Names must not contain consecutive hyphens.
    #[error("skill name must not contain consecutive hyphens")]
    ConsecutiveHyphen,
    /// Names may contain only ASCII lowercase letters, digits, and hyphens.
    #[error("skill name contains invalid character {0:?}")]
    InvalidCharacter(char),
}

/// Invalid Agent Skill description.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum SkillDescriptionError {
    /// Descriptions must not be empty.
    #[error("skill description is empty")]
    Empty,
    /// Descriptions are limited to 1024 characters.
    #[error("skill description exceeds 1024 characters")]
    TooLong,
}

/// Invalid path inside a skill folder.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum SkillPathError {
    /// Paths must not be empty.
    #[error("skill path is empty")]
    Empty,
    /// Skill paths are portable POSIX-style relative paths.
    #[error("skill path must be relative")]
    Absolute,
    /// Empty path components are not accepted.
    #[error("skill path contains an empty component")]
    EmptyComponent,
    /// Current-directory components are not accepted.
    #[error("skill path contains a current-directory component")]
    CurrentDirectory,
    /// Parent traversal is not accepted.
    #[error("skill path contains parent traversal")]
    ParentTraversal,
    /// Backslashes are rejected to keep skill archives platform-neutral.
    #[error("skill path contains a backslash")]
    Backslash,
    /// NUL bytes are never valid in paths.
    #[error("skill path contains NUL")]
    Nul,
}

/// Invalid skill route pool label.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum SkillRoutePoolError {
    /// Route pool labels must not be empty.
    #[error("skill route pool is empty")]
    Empty,
    /// NUL bytes are never valid in route pool labels.
    #[error("skill route pool contains NUL")]
    Nul,
}

/// Invalid skill route key.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum SkillRouteKeyError {
    /// Route keys must not be empty.
    #[error("skill route key is empty")]
    Empty,
    /// NUL bytes are never valid in route keys.
    #[error("skill route key contains NUL")]
    Nul,
}

/// Failure while parsing `SKILL.md`.
#[derive(Debug, thiserror::Error)]
pub enum SkillParseError {
    /// `SKILL.md` is not valid UTF-8.
    #[error("SKILL.md is not valid UTF-8")]
    Utf8(#[source] std::str::Utf8Error),
    /// The file does not start with a YAML frontmatter block.
    #[error("SKILL.md is missing YAML frontmatter")]
    MissingFrontmatter,
    /// The frontmatter block does not have a closing fence.
    #[error("SKILL.md frontmatter is missing its closing fence")]
    MissingClosingFrontmatter,
    /// The YAML frontmatter could not be parsed.
    #[error("SKILL.md frontmatter is invalid YAML")]
    Yaml(#[source] serde_yml::Error),
    /// The frontmatter root must be a mapping.
    #[error("SKILL.md frontmatter must be a YAML mapping")]
    FrontmatterNotMap,
    /// A required frontmatter field is missing.
    #[error("SKILL.md frontmatter is missing required field {field}")]
    MissingRequiredField {
        /// Missing field name.
        field: &'static str,
    },
    /// A required frontmatter field has the wrong type.
    #[error("SKILL.md frontmatter field {field} must be a string")]
    RequiredFieldNotString {
        /// Field name.
        field: &'static str,
    },
    /// A metadata key is not a string.
    #[error("SKILL.md metadata key is not a string")]
    NonStringMetadataKey,
    /// The parsed `name` field is invalid.
    #[error("SKILL.md name is invalid")]
    InvalidName(#[source] SkillNameError),
    /// The parsed `description` field is invalid.
    #[error("SKILL.md description is invalid")]
    InvalidDescription(#[source] SkillDescriptionError),
    /// The markdown body after frontmatter is empty.
    #[error("SKILL.md body is empty")]
    EmptyBody,
}

/// Skill bank invariant or edit failure.
#[derive(Debug, thiserror::Error)]
pub enum SkillBankError {
    /// A folder lacks its required `SKILL.md` file.
    #[error("skill {skill} is missing SKILL.md")]
    MissingSkillMd {
        /// Skill folder name.
        skill: String,
    },
    /// `SKILL.md` could not be parsed.
    #[error("skill {skill} has invalid SKILL.md")]
    InvalidSkillMd {
        /// Skill folder name.
        skill: String,
        /// Parse failure.
        #[source]
        source: SkillParseError,
    },
    /// The frontmatter name must match the folder name.
    #[error("skill folder {folder} contains SKILL.md name {manifest_name}")]
    NameMismatch {
        /// Folder name.
        folder: String,
        /// Name parsed from `SKILL.md`.
        manifest_name: String,
    },
    /// Duplicate skill names are not allowed in a bank.
    #[error("duplicate skill name {name}")]
    DuplicateSkillName {
        /// Duplicate name.
        name: String,
    },
    /// The requested skill does not exist.
    #[error("skill {name} was not found")]
    MissingSkill {
        /// Missing skill name.
        name: String,
    },
    /// The requested file does not exist.
    #[error("file {path} in skill {skill} was not found")]
    MissingFile {
        /// Skill name.
        skill: String,
        /// File path.
        path: String,
    },
    /// The requested skill already exists.
    #[error("skill {name} already exists")]
    SkillAlreadyExists {
        /// Existing skill name.
        name: String,
    },
    /// The requested file already exists.
    #[error("file {path} in skill {skill} already exists")]
    FileAlreadyExists {
        /// Skill name.
        skill: String,
        /// Existing file path.
        path: String,
    },
    /// `SKILL.md` could not be rendered after a canonicalizing edit.
    #[error("failed to render SKILL.md")]
    RenderSkillMd(#[source] serde_yml::Error),
}

/// Skill route registry invariant failure.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum SkillRouteRegistryError {
    /// A route spec referenced a skill that is not present in the source bank.
    #[error("route spec references unknown skill {skill}")]
    UnknownSkill {
        /// Missing skill.
        skill: crate::SkillName,
    },
    /// A route registry has more than one spec for the same skill.
    #[error("duplicate route spec for skill {skill}")]
    DuplicateSkill {
        /// Duplicate skill.
        skill: crate::SkillName,
    },
}
