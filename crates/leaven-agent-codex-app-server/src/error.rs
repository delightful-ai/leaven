//! Codex provider errors.

pub type Result<T> = std::result::Result<T, CodexAppServerError>;

#[derive(Debug, thiserror::Error)]
pub enum CodexAppServerError {
    #[error("codex app-server feature is disabled")]
    AppServerFeatureDisabled,

    #[error("codex app-server I/O failed: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "app-server")]
    #[error("codex app-server JSON failed: {0}")]
    Json(#[from] serde_json::Error),

    #[cfg(feature = "app-server")]
    #[error("codex app-server failed to decode `{method}` response: {source}; payload: {payload}")]
    ResponseDecode {
        method: String,
        payload: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("codex app-server closed the connection")]
    ConnectionClosed,

    #[error("codex app-server protocol error: {0}")]
    Protocol(String),

    #[error("codex app-server request {id} failed: {message}")]
    JsonRpc {
        id: String,
        code: i64,
        message: String,
        data: Option<String>,
    },

    #[error("codex app-server requested unsupported client method `{method}`")]
    UnsupportedServerRequest { method: String },

    #[error("codex app-server requested approval while approval mode is Error")]
    ApprovalRequested,
}
