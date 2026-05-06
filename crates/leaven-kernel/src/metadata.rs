//! Operational metadata.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::BlobRef;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MetadataKey(pub String);

impl MetadataKey {
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for MetadataKey {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for MetadataKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum MetadataValue {
    String(String),
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    Json(serde_json::Value),
    BlobRef(BlobRef),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MetadataBag {
    fields: BTreeMap<MetadataKey, MetadataValue>,
}

impl MetadataBag {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: impl Into<MetadataKey>, value: MetadataValue) -> &mut Self {
        self.fields.insert(key.into(), value);
        self
    }

    #[must_use]
    pub fn get(&self, key: &MetadataKey) -> Option<&MetadataValue> {
        self.fields.get(key)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&MetadataKey, &MetadataValue)> {
        self.fields.iter()
    }
}
