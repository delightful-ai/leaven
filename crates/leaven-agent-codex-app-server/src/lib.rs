//! Codex app-server runtime adapter.
//!
//! This crate owns the app-server protocol/client/runtime surface only. It does
//! not know Leaven candidates, proposals, evaluations, skills, GEPA, or graph
//! state.

pub mod config;
pub mod error;

#[cfg(feature = "app-server")]
mod client;
#[cfg(feature = "app-server")]
mod history;
#[cfg(feature = "app-server")]
pub mod runtime;
#[cfg(feature = "app-server")]
pub mod transport;

pub use config::{
    CodexAppServerApprovalMode, CodexAppServerConfig, CodexAppServerInitializeConfig,
    CodexAppServerThreadConfig, CodexAppServerTurnConfig, CodexApprovalPolicy,
    CodexApprovalsReviewer, CodexRawEventPolicy, CodexReasoningEffort, CodexReasoningSummary,
    CodexSandboxMode,
};
pub use error::CodexAppServerError;

#[cfg(feature = "app-server")]
pub use runtime::CodexAppServerRuntime;

#[cfg(feature = "app-server")]
pub use transport::{CodexAppServerConnection, CodexAppServerConnector, CodexAppServerTransport};

#[cfg(feature = "stdio")]
pub use transport::{StdioCodexAppServerConnector, StdioCodexAppServerTransport};

pub mod prelude {
    pub use crate::{
        CodexAppServerApprovalMode, CodexAppServerConfig, CodexAppServerError,
        CodexAppServerInitializeConfig, CodexAppServerThreadConfig, CodexAppServerTurnConfig,
        CodexApprovalPolicy, CodexApprovalsReviewer, CodexRawEventPolicy, CodexReasoningEffort,
        CodexReasoningSummary, CodexSandboxMode,
    };

    #[cfg(feature = "app-server")]
    pub use crate::{
        CodexAppServerConnection, CodexAppServerConnector, CodexAppServerRuntime,
        CodexAppServerTransport,
    };

    #[cfg(feature = "stdio")]
    pub use crate::{StdioCodexAppServerConnector, StdioCodexAppServerTransport};
}
