//! `MetadataBag` — operational, debug, and tracing extras.
//!
//! Metadata is **non-semantic**: nothing in the cold core branches on
//! its contents. Algorithms read [`crate::proposal::ProposalProvenance`]
//! for causal/informational truth and
//! [`crate::proposal::Proposal::annotations`] for typed semantic
//! payload. Metadata is for the things that don't carve a joint:
//! human-readable notes, model names, prompt fingerprints, blob refs to
//! large debug dumps.
//!
//! Distinguishing metadata from annotations follows the philosophy
//! rule that information should hold its shape until you consciously
//! reshape it: stringly-typed metadata is a one-way ticket to drift.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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

/// External blob reference. The store + key tuple identifies a blob in
/// some configured object store; the cold core does not interpret it.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct BlobRef {
    pub store: String,
    pub key: String,
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
