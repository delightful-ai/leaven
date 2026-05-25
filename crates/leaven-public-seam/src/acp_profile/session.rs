use std::collections::BTreeMap;

use crate::PublicSeamError;

use super::{AcpProfileDocument, AcpSessionLifecycle, invalid_acp};

/// ACP authenticate request that resolves a bearer token into a capability document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpAuthenticateRequest {
    token_id: String,
    now: String,
    expected_capability_fingerprint: String,
}

/// Resolved ACP authentication state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpAuthenticatedSession {
    capability_fingerprint: String,
    policy_fingerprint: String,
    subject_fingerprint: String,
    jti: String,
}

/// Profile-derived ACP worker session facts for lifecycle/backpressure validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpWorkerSession {
    pinned_acp_version: String,
    transport: String,
    engine_role: String,
    worker_role: String,
    lifecycle: AcpSessionLifecycle,
}

/// Stdio ACP worker launch environment with a redacted artifact projection.
#[derive(Clone, Eq, PartialEq)]
pub struct AcpStdioWorkerLaunch {
    transport: String,
    engine_role: String,
    worker_role: String,
    token_env: String,
    endpoint_env: String,
    fingerprint_env: String,
    bearer_token: String,
    endpoint: String,
    capability_fingerprint: String,
    worker_env: BTreeMap<String, String>,
}

impl AcpAuthenticateRequest {
    /// Creates an authenticate request from an opaque public-seam token handle.
    pub fn opaque(
        token_id: impl Into<String>,
        now: impl Into<String>,
        expected_capability_fingerprint: impl Into<String>,
    ) -> Self {
        Self {
            token_id: token_id.into(),
            now: now.into(),
            expected_capability_fingerprint: expected_capability_fingerprint.into(),
        }
    }

    pub(super) fn into_parts(self) -> (String, String, String) {
        (
            self.token_id,
            self.now,
            self.expected_capability_fingerprint,
        )
    }
}

impl AcpAuthenticatedSession {
    pub(super) fn new(
        capability_fingerprint: impl Into<String>,
        policy_fingerprint: impl Into<String>,
        subject_fingerprint: impl Into<String>,
        jti: impl Into<String>,
    ) -> Self {
        Self {
            capability_fingerprint: capability_fingerprint.into(),
            policy_fingerprint: policy_fingerprint.into(),
            subject_fingerprint: subject_fingerprint.into(),
            jti: jti.into(),
        }
    }

    /// Capability fingerprint resolved by `authenticate`.
    pub fn capability_fingerprint(&self) -> &str {
        &self.capability_fingerprint
    }

    /// Policy fingerprint carried by the resolved capability.
    pub fn policy_fingerprint(&self) -> &str {
        &self.policy_fingerprint
    }

    /// Subject fingerprint carried by the resolved capability.
    pub fn subject_fingerprint(&self) -> &str {
        &self.subject_fingerprint
    }

    /// JWT id of the resolved capability document.
    pub fn jti(&self) -> &str {
        &self.jti
    }
}

impl AcpWorkerSession {
    /// Starts a public-seam ACP worker session model from a validated profile.
    pub fn start(profile: &AcpProfileDocument) -> Result<Self, PublicSeamError> {
        let transport = profile
            .transports()
            .first()
            .filter(|transport| transport.as_str() == "stdio_jsonrpc")
            .ok_or_else(|| invalid_acp("ACP worker session must start on stdio_jsonrpc transport"))?
            .clone();
        Ok(Self {
            pinned_acp_version: profile.pinned_acp_version().to_owned(),
            transport,
            engine_role: "engine_client".to_owned(),
            worker_role: "worker_agent".to_owned(),
            lifecycle: AcpSessionLifecycle::from_profile(profile)?,
        })
    }

    /// Pinned ACP version used for this session.
    pub fn pinned_acp_version(&self) -> &str {
        &self.pinned_acp_version
    }

    /// Transport binding used to start this session.
    pub fn transport(&self) -> &str {
        &self.transport
    }

    /// ACP role of the Leaven engine.
    pub fn engine_role(&self) -> &str {
        &self.engine_role
    }

    /// ACP role of the external worker.
    pub fn worker_role(&self) -> &str {
        &self.worker_role
    }

    /// Lifecycle and progress-update state for this session.
    pub const fn lifecycle(&self) -> &AcpSessionLifecycle {
        &self.lifecycle
    }

    /// Mutable lifecycle and progress-update state for this session.
    pub fn lifecycle_mut(&mut self) -> &mut AcpSessionLifecycle {
        &mut self.lifecycle
    }
}

impl AcpStdioWorkerLaunch {
    /// Builds the stdio launch environment for a validated ACP worker session.
    pub fn new(
        profile: &AcpProfileDocument,
        session: &AcpWorkerSession,
        bearer_token: impl Into<String>,
        endpoint: impl Into<String>,
        capability_fingerprint: impl Into<String>,
    ) -> Result<Self, PublicSeamError> {
        if session.transport() != "stdio_jsonrpc" {
            return Err(invalid_acp(
                "ACP stdio worker launch requires stdio_jsonrpc transport",
            ));
        }
        let bearer_token = bearer_token.into();
        let endpoint = endpoint.into();
        let capability_fingerprint = capability_fingerprint.into();
        if bearer_token.trim().is_empty() {
            return Err(invalid_acp(
                "ACP stdio worker launch requires a non-empty bearer token",
            ));
        }
        if endpoint.trim().is_empty() {
            return Err(invalid_acp(
                "ACP stdio worker launch requires a non-empty endpoint",
            ));
        }
        if capability_fingerprint.trim().is_empty() {
            return Err(invalid_acp(
                "ACP stdio worker launch requires a capability fingerprint",
            ));
        }
        let mut worker_env = BTreeMap::new();
        worker_env.insert(profile.token_env().to_owned(), bearer_token.clone());
        worker_env.insert(profile.endpoint_env().to_owned(), endpoint.clone());
        worker_env.insert(
            profile.fingerprint_env().to_owned(),
            capability_fingerprint.clone(),
        );
        Ok(Self {
            transport: session.transport().to_owned(),
            engine_role: session.engine_role().to_owned(),
            worker_role: session.worker_role().to_owned(),
            token_env: profile.token_env().to_owned(),
            endpoint_env: profile.endpoint_env().to_owned(),
            fingerprint_env: profile.fingerprint_env().to_owned(),
            bearer_token,
            endpoint,
            capability_fingerprint,
            worker_env,
        })
    }

    /// Transport used by the worker launch.
    pub fn transport(&self) -> &str {
        &self.transport
    }

    /// ACP role of the Leaven engine.
    pub fn engine_role(&self) -> &str {
        &self.engine_role
    }

    /// ACP role of the external worker.
    pub fn worker_role(&self) -> &str {
        &self.worker_role
    }

    /// Environment passed to the worker process.
    pub fn worker_env(&self) -> &BTreeMap<String, String> {
        &self.worker_env
    }

    /// Artifact-safe launch facts. The bearer token is intentionally omitted.
    pub fn artifact_env(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            (self.endpoint_env.clone(), self.endpoint.clone()),
            (
                self.fingerprint_env.clone(),
                self.capability_fingerprint.clone(),
            ),
        ])
    }

    /// Rejects persisted launch facts that still contain the bearer token.
    pub fn validate_artifact_env(
        &self,
        artifact_env: &BTreeMap<String, String>,
    ) -> Result<(), PublicSeamError> {
        if artifact_env.contains_key(&self.token_env) {
            Err(invalid_acp(
                "ACP worker launch artifacts must not persist LEAVEN_CAPABILITY_TOKEN",
            ))
        } else if artifact_env
            .values()
            .any(|value| value.contains(&self.bearer_token))
        {
            Err(invalid_acp(
                "ACP worker launch artifacts must not persist the bearer secret value",
            ))
        } else {
            Ok(())
        }
    }
}

impl std::fmt::Debug for AcpStdioWorkerLaunch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut worker_env = self.worker_env.clone();
        if let Some(token) = worker_env.get_mut(&self.token_env) {
            "<redacted>".clone_into(token);
        }
        formatter
            .debug_struct("AcpStdioWorkerLaunch")
            .field("transport", &self.transport)
            .field("engine_role", &self.engine_role)
            .field("worker_role", &self.worker_role)
            .field("token_env", &self.token_env)
            .field("endpoint_env", &self.endpoint_env)
            .field("fingerprint_env", &self.fingerprint_env)
            .field("bearer_token", &"<redacted>")
            .field("endpoint", &self.endpoint)
            .field("capability_fingerprint", &self.capability_fingerprint)
            .field("worker_env", &worker_env)
            .finish()
    }
}
