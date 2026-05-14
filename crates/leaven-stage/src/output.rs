use leaven_kernel::Fingerprint;
use leaven_workspace::{WorkspacePath, WorkspacePathError};
use serde::{Deserialize, Serialize};

use crate::{MediaType, StageOutputContractError};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputEntryId(String);

impl OutputEntryId {
    pub fn new(value: impl Into<String>) -> Result<Self, StageOutputContractError> {
        let value = value.into();
        if value.is_empty() || value.contains('/') || value.chars().any(char::is_whitespace) {
            return Err(StageOutputContractError::InvalidEntryId(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn new_static(value: &'static str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputRole(String);

impl OutputRole {
    pub fn new(value: impl Into<String>) -> Result<Self, StageOutputContractError> {
        let value = value.into();
        if value.is_empty() || value.contains('/') || value.chars().any(char::is_whitespace) {
            return Err(StageOutputContractError::InvalidOutputRole(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn new_static(value: &'static str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn proposal_json() -> Self {
        Self::new_static("proposal_json")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OutputSchema {
    pub media_type: MediaType,
    pub schema_text: String,
    pub schema_fingerprint: Option<Fingerprint>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OutputEntry {
    pub id: OutputEntryId,
    pub path: WorkspacePath,
    pub role: OutputRole,
    pub media_type: MediaType,
    pub max_bytes: Option<u64>,
}

impl OutputEntry {
    #[must_use]
    pub fn new(
        id: OutputEntryId,
        path: WorkspacePath,
        role: OutputRole,
        media_type: MediaType,
    ) -> Self {
        Self {
            id,
            path,
            role,
            media_type,
            max_bytes: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageOutputContract {
    pub required: Vec<OutputEntry>,
    pub optional: Vec<OutputEntry>,
    pub schema: Option<OutputSchema>,
}

impl StageOutputContract {
    #[must_use]
    pub fn new(required: Vec<OutputEntry>) -> Self {
        Self {
            required,
            optional: Vec::new(),
            schema: None,
        }
    }

    #[must_use]
    pub fn proposal_json(path: WorkspacePath) -> Self {
        Self::new(vec![OutputEntry::new(
            OutputEntryId::new_static("proposal"),
            path,
            OutputRole::proposal_json(),
            MediaType::Json,
        )])
    }

    pub fn validate(&self) -> Result<(), StageOutputContractError> {
        if self.required.is_empty() {
            return Err(StageOutputContractError::NoRequiredOutputs);
        }
        for entry in self.all_entries() {
            validate_output_path(&entry.path).map_err(|source| {
                StageOutputContractError::InvalidOutputPath {
                    id: entry.id.clone(),
                    path: entry.path.clone(),
                    source,
                }
            })?;
        }
        Ok(())
    }

    pub fn all_entries(&self) -> impl Iterator<Item = &OutputEntry> {
        self.required.iter().chain(self.optional.iter())
    }

    #[must_use]
    pub fn to_agent_output_contract(&self) -> leaven_agent::OutputContract {
        if self.required.len() == 1 && self.optional.is_empty() {
            let entry = &self.required[0];
            if entry.media_type == MediaType::Json {
                return leaven_agent::OutputContract::JsonFile {
                    path: entry.path.clone(),
                    schema: self
                        .schema
                        .as_ref()
                        .map(|schema| leaven_agent::JsonSchemaRef {
                            name: entry.id.as_str().to_owned(),
                            schema: schema.schema_text.clone(),
                        }),
                };
            }
        }
        leaven_agent::OutputContract::Files {
            paths: self.all_entries().map(|entry| entry.path.clone()).collect(),
        }
    }
}

fn validate_output_path(path: &WorkspacePath) -> Result<(), WorkspacePathError> {
    if path.as_str() == "output" || !path.starts_with_component("output") {
        return Err(WorkspacePathError::OutsideView {
            path: path.as_str().to_owned(),
            prefix: "output".to_owned(),
        });
    }
    Ok(())
}
