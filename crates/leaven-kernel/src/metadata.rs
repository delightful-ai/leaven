//! Operational metadata bags.
//!
//! Metadata in Leaven is *operational*: hostnames, worker IDs, blob
//! pointers, diagnostic breadcrumbs. It is explicitly **not** a
//! semantic channel — optimizer logic must not branch on metadata.
//!
//! When stages need typed semantic payloads (reflection notes,
//! behavioral claims, surrogate predictions), those go into the
//! per-`OptimizationProblem` typed annotations defined in `leaven-core`.
//! Keeping the two channels separate prevents stringly-typed
//! metadata-parsing from creeping into optimizer logic.
//!
//! [`MetadataBag`] is a key/value store with a small fixed set of typed
//! [`MetadataValue`] shapes plus a `Json` escape hatch. Keys are owned
//! strings rather than `Cow` because metadata churn is rarer than
//! identity churn and the simplification is worth one allocation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{BlobRef, FiniteF64};

/// Owned-string key into a [`MetadataBag`].
///
/// Wraps `String` rather than `&'static str` so dynamically-built keys
/// (e.g. from a configuration file) can be used; static keys go through
/// the `From<&str>` conversion.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MetadataKey(pub String);

impl MetadataKey {
    /// Constructs a key from any string-like value.
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Returns the underlying name as a string slice.
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

/// Tagged value stored in a [`MetadataBag`].
///
/// The variants cover the small set of shapes most operational metadata
/// actually wants. `Json` is the escape hatch for arbitrary structured
/// payloads; `BlobRef` is the recommended channel for any value larger
/// than a few hundred bytes — the bag itself should stay light.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum MetadataValue {
    /// Free-form string.
    String(String),
    /// Boolean flag.
    Bool(bool),
    /// Signed integer.
    I64(i64),
    /// Unsigned integer.
    U64(u64),
    /// Finite floating-point number.
    F64(FiniteF64),
    /// Arbitrary JSON document.
    Json(serde_json::Value),
    /// Pointer to a blob held outside the run graph.
    BlobRef(BlobRef),
}

/// Ordered key/value bag attached to most run-graph entities.
///
/// Backed by `BTreeMap` for deterministic iteration order — important
/// when serialized state is fed back through hashing or comparison
/// downstream.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MetadataBag {
    fields: BTreeMap<MetadataKey, MetadataValue>,
}

impl MetadataBag {
    /// Returns an empty bag.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or replaces the value for `key`. Returns `&mut self` so
    /// chained inserts read naturally at the call site.
    pub fn insert(&mut self, key: impl Into<MetadataKey>, value: MetadataValue) -> &mut Self {
        self.fields.insert(key.into(), value);
        self
    }

    /// Returns the value for `key`, or `None` when the key is absent.
    #[must_use]
    pub fn get(&self, key: &MetadataKey) -> Option<&MetadataValue> {
        self.fields.get(key)
    }

    /// Returns the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Returns true when the bag has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Iterates over `(key, value)` pairs in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&MetadataKey, &MetadataValue)> {
        self.fields.iter()
    }
}
