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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_values_round_trip_to_yaml_shape() {
        let mut nested = BTreeMap::new();
        nested.insert("flag".to_owned(), SkillMetadataValue::Bool(true));
        let value = SkillMetadataValue::Mapping(BTreeMap::from([
            ("nothing".to_owned(), SkillMetadataValue::Null),
            (
                "count".to_owned(),
                SkillMetadataValue::Number("3".to_owned()),
            ),
            (
                "name".to_owned(),
                SkillMetadataValue::String("alpha".to_owned()),
            ),
            (
                "items".to_owned(),
                SkillMetadataValue::Sequence(vec![SkillMetadataValue::Mapping(nested)]),
            ),
        ]));

        let yaml = value.to_yaml();
        let round_tripped = SkillMetadataValue::from_yaml(yaml).unwrap();

        assert!(matches!(
            round_tripped,
            SkillMetadataValue::Mapping(values)
                if values.get("count") == Some(&SkillMetadataValue::String("3".to_owned()))
                    && matches!(
                        values.get("items"),
                        Some(SkillMetadataValue::Sequence(items))
                            if matches!(
                                items.first(),
                                Some(SkillMetadataValue::Mapping(nested))
                                    if nested.get("flag") == Some(&SkillMetadataValue::Bool(true))
                            )
                    )
                    && values.get("name") == Some(&SkillMetadataValue::String("alpha".to_owned()))
                    && values.get("nothing") == Some(&SkillMetadataValue::Null)
        ));
    }

    #[test]
    fn metadata_from_yaml_rejects_non_string_mapping_keys() {
        let yaml = serde_yml::Value::Mapping(
            std::iter::once((serde_yml::Value::Bool(true), serde_yml::Value::Null)).collect(),
        );

        assert!(matches!(
            SkillMetadataValue::from_yaml(yaml),
            Err(SkillParseError::NonStringMetadataKey)
        ));
    }

    #[test]
    fn metadata_from_yaml_ignores_yaml_tag_wrappers() {
        let yaml = serde_yml::Value::Tagged(Box::new(serde_yml::value::TaggedValue {
            tag: serde_yml::value::Tag::new("!skill"),
            value: serde_yml::Value::String("tagged".to_owned()),
        }));

        assert_eq!(
            SkillMetadataValue::from_yaml(yaml).unwrap(),
            SkillMetadataValue::String("tagged".to_owned())
        );
    }
}
