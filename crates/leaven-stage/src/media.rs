use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum MediaType {
    Json,
    Markdown,
    Text,
    Diff,
    Binary,
    Custom(String),
}

impl MediaType {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Json => "application/json",
            Self::Markdown => "text/markdown",
            Self::Text => "text/plain",
            Self::Diff => "text/x-diff",
            Self::Binary => "application/octet-stream",
            Self::Custom(value) => value.as_str(),
        }
    }
}
