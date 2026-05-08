//! Parsed `SKILL.md` frontmatter and body.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use crate::{
    SkillDescriptionError, SkillMetadata, SkillMetadataValue, SkillNameError, SkillParseError,
};

/// Valid Agent Skill name.
#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(transparent)]
pub struct SkillName(String);

impl SkillName {
    /// Validates and constructs a skill name.
    ///
    /// # Errors
    ///
    /// Returns [`SkillNameError`] when the name violates the Agent Skills
    /// naming rules.
    pub fn new(value: impl Into<String>) -> Result<Self, SkillNameError> {
        let value = value.into();
        validate_skill_name(&value)?;
        Ok(Self(value))
    }

    /// Returns the validated string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SkillName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SkillName {
    type Err = SkillNameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for SkillName {
    type Error = SkillNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Valid Agent Skill description.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct SkillDescription(String);

impl SkillDescription {
    /// Validates and constructs a description.
    ///
    /// # Errors
    ///
    /// Returns [`SkillDescriptionError`] when the description is empty or too
    /// long.
    pub fn new(value: impl Into<String>) -> Result<Self, SkillDescriptionError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(SkillDescriptionError::Empty);
        }
        if value.chars().count() > 1024 {
            return Err(SkillDescriptionError::TooLong);
        }
        Ok(Self(value))
    }

    /// Returns the validated string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SkillDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Non-empty markdown body from `SKILL.md`.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct SkillBody(String);

impl SkillBody {
    /// Validates and constructs a markdown body.
    ///
    /// # Errors
    ///
    /// Returns [`SkillParseError::EmptyBody`] when the body has no non-whitespace
    /// content.
    pub fn new(value: impl Into<String>) -> Result<Self, SkillParseError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(SkillParseError::EmptyBody);
        }
        Ok(Self(value))
    }

    /// Returns the markdown body.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Parsed Agent Skill frontmatter.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillManifest {
    /// Required skill name.
    pub name: SkillName,
    /// Required retrieval and usage description.
    pub description: SkillDescription,
    /// Generic metadata bag for all non-core frontmatter fields.
    pub metadata: SkillMetadata,
}

impl SkillManifest {
    /// Constructs a parsed manifest.
    pub fn new(name: SkillName, description: SkillDescription, metadata: SkillMetadata) -> Self {
        Self {
            name,
            description,
            metadata,
        }
    }
}

/// Parsed `SKILL.md`.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ParsedSkillMd {
    /// Parsed frontmatter.
    pub manifest: SkillManifest,
    /// Markdown body after frontmatter.
    pub body: SkillBody,
}

impl ParsedSkillMd {
    /// Parses a `SKILL.md` byte payload.
    ///
    /// # Errors
    ///
    /// Returns [`SkillParseError`] for invalid UTF-8, malformed frontmatter,
    /// missing required fields, invalid core fields, or an empty body.
    pub fn parse(bytes: &[u8]) -> Result<Self, SkillParseError> {
        let text = std::str::from_utf8(bytes).map_err(SkillParseError::Utf8)?;
        let (frontmatter, body) = split_frontmatter(text)?;
        let manifest = parse_manifest(frontmatter)?;
        let body = SkillBody::new(body.to_owned())?;
        Ok(Self { manifest, body })
    }

    pub(crate) fn to_skill_md_bytes(&self) -> Result<Vec<u8>, serde_yml::Error> {
        let mut mapping = serde_yml::Mapping::new();
        mapping.insert(
            serde_yml::Value::String("name".to_owned()),
            serde_yml::Value::String(self.manifest.name.as_str().to_owned()),
        );
        mapping.insert(
            serde_yml::Value::String("description".to_owned()),
            serde_yml::Value::String(self.manifest.description.as_str().to_owned()),
        );
        for (key, value) in &self.manifest.metadata {
            mapping.insert(serde_yml::Value::String(key.clone()), value.to_yaml());
        }
        let yaml = serde_yml::to_string(&serde_yml::Value::Mapping(mapping))?;
        let mut out = String::with_capacity(yaml.len() + self.body.as_str().len() + 8);
        out.push_str("---\n");
        out.push_str(&yaml);
        out.push_str("---\n");
        out.push_str(self.body.as_str());
        Ok(out.into_bytes())
    }
}

fn validate_skill_name(value: &str) -> Result<(), SkillNameError> {
    if value.is_empty() {
        return Err(SkillNameError::Empty);
    }
    if value.len() > 64 {
        return Err(SkillNameError::TooLong);
    }
    if value.starts_with('-') {
        return Err(SkillNameError::StartsWithHyphen);
    }
    if value.ends_with('-') {
        return Err(SkillNameError::EndsWithHyphen);
    }
    if value.contains("--") {
        return Err(SkillNameError::ConsecutiveHyphen);
    }
    for ch in value.chars() {
        if !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-') {
            return Err(SkillNameError::InvalidCharacter(ch));
        }
    }
    Ok(())
}

fn split_frontmatter(text: &str) -> Result<(&str, &str), SkillParseError> {
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return Err(SkillParseError::MissingFrontmatter);
    };
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            let frontmatter = &rest[..offset];
            let body = &rest[offset + line.len()..];
            return Ok((frontmatter, body));
        }
        offset += line.len();
    }
    Err(SkillParseError::MissingClosingFrontmatter)
}

fn parse_manifest(frontmatter: &str) -> Result<SkillManifest, SkillParseError> {
    let value = serde_yml::from_str(frontmatter).map_err(SkillParseError::Yaml)?;
    let serde_yml::Value::Mapping(mut mapping) = value else {
        return Err(SkillParseError::FrontmatterNotMap);
    };
    let name = take_required_string(&mut mapping, "name")?;
    let description = take_required_string(&mut mapping, "description")?;
    let metadata = parse_metadata(mapping)?;
    Ok(SkillManifest::new(
        SkillName::new(name).map_err(SkillParseError::InvalidName)?,
        SkillDescription::new(description).map_err(SkillParseError::InvalidDescription)?,
        metadata,
    ))
}

fn take_required_string(
    mapping: &mut serde_yml::Mapping,
    field: &'static str,
) -> Result<String, SkillParseError> {
    let Some(value) = mapping.remove(serde_yml::Value::String(field.to_owned())) else {
        return Err(SkillParseError::MissingRequiredField { field });
    };
    let serde_yml::Value::String(value) = value else {
        return Err(SkillParseError::RequiredFieldNotString { field });
    };
    Ok(value)
}

fn parse_metadata(mapping: serde_yml::Mapping) -> Result<SkillMetadata, SkillParseError> {
    let mut metadata = BTreeMap::new();
    for (key, value) in mapping {
        let serde_yml::Value::String(key) = key else {
            return Err(SkillParseError::NonStringMetadataKey);
        };
        metadata.insert(key, SkillMetadataValue::from_yaml(value)?);
    }
    Ok(metadata)
}
