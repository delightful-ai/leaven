use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::Value;

/// Typed resource selector value used by capability grants and requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityResourceValue {
    /// One exact resource id/name.
    One(String),
    /// A closed set of accepted resource ids/names.
    Many(BTreeSet<String>),
    /// Caller supplied a non-resource-shaped value.
    Invalid,
}

impl CapabilityResourceValue {
    pub(super) fn one(value: impl Into<String>) -> Self {
        Self::One(value.into())
    }

    pub(super) fn from_json(value: Value) -> Self {
        match value {
            Value::String(value) => Self::One(value),
            Value::Array(values) => {
                let mut items = BTreeSet::new();
                for value in values {
                    let Value::String(value) = value else {
                        return Self::Invalid;
                    };
                    items.insert(value);
                }
                Self::Many(items)
            }
            _ => Self::Invalid,
        }
    }

    pub(super) fn allows(&self, requested: &Self) -> bool {
        match (self, requested) {
            (Self::One(allowed), Self::One(requested)) => allowed == requested,
            (Self::Many(allowed), Self::One(requested)) => allowed.contains(requested),
            (Self::Many(allowed), Self::Many(requested)) => requested.is_subset(allowed),
            _ => false,
        }
    }
}

impl<'de> Deserialize<'de> for CapabilityResourceValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(Self::from_json)
    }
}

/// Per-operation usage checked against grant limits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CapabilityLimitUsage {
    /// Requested spend in USD micro-units.
    pub usd_micro: Option<u64>,
    /// Requested call count.
    pub calls: Option<u64>,
    /// Requested concurrent calls.
    pub concurrent: Option<u64>,
    /// Requested timeout in seconds.
    pub timeout_s: Option<u64>,
    /// Requested row count.
    pub rows: Option<u64>,
    /// Requested materialized bytes.
    pub materialized_bytes: Option<u64>,
}

/// Authorized grant facts surfaced to later permission decisions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedGrant {
    pub(super) capability_fingerprint: String,
    pub(super) policy_fingerprint: String,
    pub(super) grant_action: String,
    pub(super) max_usd_micro: Option<u64>,
    pub(super) max_calls: Option<u64>,
    pub(super) max_concurrent: Option<u64>,
    pub(super) timeout_s: Option<u64>,
    pub(super) max_rows: Option<u64>,
    pub(super) max_materialized_bytes: Option<u64>,
}

impl AuthorizedGrant {
    /// Capability fingerprint attached to the authorization.
    pub fn capability_fingerprint(&self) -> &str {
        &self.capability_fingerprint
    }

    /// Policy fingerprint attached to the authorization.
    pub fn policy_fingerprint(&self) -> &str {
        &self.policy_fingerprint
    }

    /// Grant action that authorized the request.
    pub fn grant_action(&self) -> &str {
        &self.grant_action
    }

    /// Grant maximum spend in USD micro-units.
    pub fn max_usd_micro(&self) -> Option<u64> {
        self.max_usd_micro
    }

    /// Grant maximum call count.
    pub fn max_calls(&self) -> Option<u64> {
        self.max_calls
    }

    /// Grant maximum concurrent calls.
    pub fn max_concurrent(&self) -> Option<u64> {
        self.max_concurrent
    }

    /// Grant timeout in seconds.
    pub fn timeout_s(&self) -> Option<u64> {
        self.timeout_s
    }

    /// Grant maximum row count.
    pub fn max_rows(&self) -> Option<u64> {
        self.max_rows
    }

    /// Grant maximum materialized bytes.
    pub fn max_materialized_bytes(&self) -> Option<u64> {
        self.max_materialized_bytes
    }
}
