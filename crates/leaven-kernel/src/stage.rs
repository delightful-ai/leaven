//! Stage attempt receipt vocabulary shared by engine and stage adapters.

use serde::{Deserialize, Serialize};

use crate::{Fingerprint, StageAttemptReceiptId};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StageRole(String);

impl StageRole {
    pub fn new(value: impl Into<String>) -> Result<Self, StageRoleError> {
        let value = value.into();
        if value.is_empty() || value.contains('/') || value.chars().any(char::is_whitespace) {
            return Err(StageRoleError {
                value,
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn new_static(value: &'static str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn reflect() -> Self {
        Self::new_static("reflect")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("invalid stage role `{value}`")]
pub struct StageRoleError {
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageAttemptReceiptRef {
    pub id: StageAttemptReceiptId,
    pub fingerprint: Option<Fingerprint>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StageAttemptOutcome {
    Completed,
    Failed(StageAttemptFailure),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StageAttemptFailure {
    WorkspaceAllocate,
    WorkspaceSetup,
    Query,
    RuntimeTimeout,
    Runtime,
    OutputContract,
    OutputParse,
    Cleanup,
    StageAndCleanup {
        stage: Box<StageAttemptFailure>,
        cleanup: Box<StageAttemptFailure>,
    },
    Budget,
    Other(String),
}
