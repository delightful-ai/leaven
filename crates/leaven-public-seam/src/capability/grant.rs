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
