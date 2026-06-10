use std::collections::BTreeSet;
use std::fs;

use super::PublicSeamPackage;
use super::support::backtick_tokens;
use crate::{LockedMethod, PublicSeamError};

/// Locked V1 runtime scope implied by manifest markers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct V1Scope {
    /// Whether MCP-over-ACP is enabled in V1.
    pub mcp_over_acp_enabled: bool,
    /// Whether `watch.v1` runtime behavior is enabled in V1.
    pub watch_runtime_enabled: bool,
    /// Whether deprecated `worker_protocol.v1` runtime behavior is enabled.
    pub legacy_worker_protocol_enabled: bool,
    /// Worker transport selected by V1.
    pub worker_transport: &'static str,
    allowed_extension_methods: BTreeSet<String>,
}

/// Worker transport family requested by a V1 worker route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerTransportKind {
    /// Locked Leaven ACP profile route.
    AcpProfile,
    /// Explicitly excluded MCP-over-ACP bridge.
    McpOverAcp,
    /// Deprecated pre-ACP worker protocol marker.
    LegacyWorkerProtocol,
}

/// Requested worker transport facts checked against locked V1 scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerTransportRequest {
    kind: WorkerTransportKind,
    extension_methods: Vec<String>,
    watch_runtime_requested: bool,
}

impl WorkerTransportRequest {
    /// Creates a transport request with the advertised extension methods.
    pub fn new<I, S>(kind: WorkerTransportKind, extension_methods: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            kind,
            extension_methods: extension_methods.into_iter().map(Into::into).collect(),
            watch_runtime_requested: false,
        }
    }

    /// Creates a V1 ACP-profile request.
    pub fn acp_profile<I, S>(extension_methods: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::new(WorkerTransportKind::AcpProfile, extension_methods)
    }

    /// Adds one advertised extension method.
    pub fn add_extension_method(&mut self, method: impl Into<String>) {
        self.extension_methods.push(method.into());
    }

    /// Requests V1 watch runtime behavior.
    pub fn enable_watch_runtime(&mut self) {
        self.watch_runtime_requested = true;
    }
}

/// Authorized V1 worker transport facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedWorkerTransport {
    worker_transport: &'static str,
    extension_methods: Vec<String>,
}

impl AuthorizedWorkerTransport {
    /// Authorized worker transport route.
    pub fn worker_transport(&self) -> &str {
        self.worker_transport
    }

    /// ACP extension methods available to the worker.
    pub fn extension_methods(&self) -> Vec<&str> {
        self.extension_methods.iter().map(String::as_str).collect()
    }
}

impl V1Scope {
    /// Authorizes a worker transport request against the locked V1 scope.
    pub fn authorize_worker_transport(
        &self,
        request: WorkerTransportRequest,
    ) -> Result<AuthorizedWorkerTransport, PublicSeamError> {
        match request.kind {
            WorkerTransportKind::AcpProfile if self.worker_transport == "acp_profile" => {}
            WorkerTransportKind::AcpProfile => {
                return Err(PublicSeamError::InvalidScope {
                    message: "V1 worker transport must be acp_profile".to_owned(),
                });
            }
            WorkerTransportKind::McpOverAcp => {
                return Err(PublicSeamError::InvalidScope {
                    message: "MCP-over-ACP is not in V1".to_owned(),
                });
            }
            WorkerTransportKind::LegacyWorkerProtocol => {
                return Err(PublicSeamError::InvalidScope {
                    message: "worker_protocol.v1 is deprecated in favor of ACP".to_owned(),
                });
            }
        }
        if request.watch_runtime_requested || self.watch_runtime_enabled {
            return Err(PublicSeamError::InvalidScope {
                message: "watch.v1 runtime behavior is deferred from V1".to_owned(),
            });
        }
        if self.mcp_over_acp_enabled || self.legacy_worker_protocol_enabled {
            return Err(PublicSeamError::InvalidScope {
                message: "V1 scope cannot enable MCP or legacy worker protocols".to_owned(),
            });
        }
        if request.extension_methods.is_empty() {
            return Err(PublicSeamError::InvalidScope {
                message: "ACP profile must advertise Leaven extension methods".to_owned(),
            });
        }
        for method in &request.extension_methods {
            if !method.starts_with("leaven/") {
                return Err(PublicSeamError::InvalidScope {
                    message: format!("extension method `{method}` is not a Leaven ACP method"),
                });
            }
            if method.to_ascii_lowercase().contains("mcp") {
                return Err(PublicSeamError::InvalidScope {
                    message: format!("extension method `{method}` uses MCP vocabulary"),
                });
            }
            if !self.allowed_extension_methods.contains(method) {
                return Err(PublicSeamError::InvalidScope {
                    message: format!(
                        "extension method `{method}` is not in the locked ACP profile"
                    ),
                });
            }
        }

        Ok(AuthorizedWorkerTransport {
            worker_transport: self.worker_transport,
            extension_methods: request.extension_methods,
        })
    }
}

impl PublicSeamPackage {
    /// Returns the locked V1 scope, refusing manifest drift.
    pub fn v1_scope(&self) -> Result<V1Scope, PublicSeamError> {
        if self.manifest.mcp_status != "not_in_v1" {
            return Err(PublicSeamError::InvalidScope {
                message: "manifest.mcp_status must remain not_in_v1".to_owned(),
            });
        }
        if self.manifest.watch_status != "deferred_to_v1.1" {
            return Err(PublicSeamError::InvalidScope {
                message: "manifest.watch_status must remain deferred_to_v1.1".to_owned(),
            });
        }
        if self.manifest.worker_protocol_status != "deprecated_replaced_by_acp_profile" {
            return Err(PublicSeamError::InvalidScope {
                message: "manifest.worker_protocol_status must remain deprecated".to_owned(),
            });
        }
        Ok(V1Scope {
            mcp_over_acp_enabled: false,
            watch_runtime_enabled: false,
            legacy_worker_protocol_enabled: false,
            worker_transport: "acp_profile",
            allowed_extension_methods: self.acp_extension_methods()?.into_iter().collect(),
        })
    }

    /// Extracts Leaven ACP extension methods from the locked V1 profile.
    pub fn acp_extension_methods(&self) -> Result<Vec<String>, PublicSeamError> {
        let mut methods = BTreeSet::new();
        for profile in &self.manifest.profiles {
            let path = self.root.join("profiles").join(profile);
            let contents = fs::read_to_string(&path).map_err(|source| PublicSeamError::Io {
                path: path.clone(),
                source,
            })?;
            for token in backtick_tokens(&contents) {
                // The worker profile only advertises worker callbacks plus the
                // host->worker stage dispatch. A `leaven/*` token that resolves
                // to a non-worker-profile locked method (such as the
                // client->host `leaven/optimize.run` dispatch) must never be
                // advertised, even when the profile MD mentions it in backticks.
                if token.starts_with("leaven/")
                    && LockedMethod::parse(token).is_none_or(LockedMethod::is_worker_profile_method)
                {
                    methods.insert(token.to_owned());
                }
                if token.to_ascii_lowercase().starts_with("mcp/") {
                    return Err(PublicSeamError::InvalidScope {
                        message: format!("ACP profile advertises MCP method `{token}`"),
                    });
                }
            }
        }
        if methods.is_empty() {
            return Err(PublicSeamError::InvalidScope {
                message: "ACP profile has no Leaven extension methods".to_owned(),
            });
        }
        Ok(methods.into_iter().collect())
    }
}
