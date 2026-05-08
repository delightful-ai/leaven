//! Generic skill metadata.

use std::collections::BTreeMap;

use crate::SkillParseError;

/// Generic metadata attached to an Agent Skill.
pub type SkillMetadata = BTreeMap<String, SkillMetadataValue>;

/// YAML-compatible metadata value preserved from `SKILL.md` frontmatter.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum SkillMetadataValue {
    /// YAML null.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Number rendered in canonical string form.
    Number(String),
    /// String value.
    String(String),
    /// Sequence value.
    Sequence(Vec<Self>),
    /// Mapping with string keys.
    Mapping(SkillMetadata),
}

impl SkillMetadataValue {
    pub(crate) fn from_yaml(value: serde_yml::Value) -> Result<Self, SkillParseError> {
        match value {
            serde_yml::Value::Null => Ok(Self::Null),
            serde_yml::Value::Bool(value) => Ok(Self::Bool(value)),
            serde_yml::Value::Number(value) => Ok(Self::Number(value.to_string())),
            serde_yml::Value::String(value) => Ok(Self::String(value)),
            serde_yml::Value::Sequence(values) => values
                .into_iter()
                .map(Self::from_yaml)
                .collect::<Result<Vec<_>, _>>()
                .map(Self::Sequence),
            serde_yml::Value::Mapping(mapping) => {
                let mut out = BTreeMap::new();
                for (key, value) in mapping {
                    let serde_yml::Value::String(key) = key else {
                        return Err(SkillParseError::NonStringMetadataKey);
                    };
                    out.insert(key, Self::from_yaml(value)?);
                }
                Ok(Self::Mapping(out))
            }
            serde_yml::Value::Tagged(tagged) => Self::from_yaml(tagged.value),
        }
    }

    pub(crate) fn to_yaml(&self) -> serde_yml::Value {
        match self {
            Self::Null => serde_yml::Value::Null,
            Self::Bool(value) => serde_yml::Value::Bool(*value),
            Self::Number(value) | Self::String(value) => serde_yml::Value::String(value.clone()),
            Self::Sequence(values) => {
                serde_yml::Value::Sequence(values.iter().map(Self::to_yaml).collect())
            }
            Self::Mapping(values) => {
                let mapping = values
                    .iter()
                    .map(|(key, value)| {
                        (serde_yml::Value::String(key.clone()), Self::to_yaml(value))
                    })
                    .collect();
                serde_yml::Value::Mapping(mapping)
            }
        }
    }
}
