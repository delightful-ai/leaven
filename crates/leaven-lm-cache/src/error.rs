/// Error returned by LM response-cache backends.
#[derive(Debug, thiserror::Error)]
pub enum LmCacheError {
    /// Cache key or entry codec failed.
    #[error("lm cache codec failed: {message}")]
    Codec {
        /// Codec failure message.
        message: String,
    },
    /// Backend refused a cache operation.
    #[error("lm cache backend failed during {operation}: {message}")]
    Backend {
        /// Operation being attempted.
        operation: &'static str,
        /// Backend failure message.
        message: String,
    },
}

impl LmCacheError {
    /// Builds a codec error.
    pub fn codec(message: impl Into<String>) -> Self {
        Self::Codec {
            message: message.into(),
        }
    }
}
