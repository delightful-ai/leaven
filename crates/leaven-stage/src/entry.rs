use leaven_kernel::{AssessmentId, CandidateId, MetadataBag, WorkspaceEntryId};
use leaven_workspace::WorkspacePath;
use serde::{Deserialize, Serialize};

use crate::MediaType;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub id: WorkspaceEntryId,
    pub role: WorkspaceEntryRole,
    pub source: EntrySource,
    pub projection: EntryProjection,
    pub placement: Placement,
    pub access: EntryAccess,
    pub media_type: Option<MediaType>,
    pub max_bytes: Option<u64>,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceEntryRole(String);

impl WorkspaceEntryRole {
    #[must_use]
    pub fn new_static(value: &'static str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn brief() -> Self {
        Self::new_static("brief")
    }

    #[must_use]
    pub fn query_summary() -> Self {
        Self::new_static("query_summary")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EntrySource {
    InlineText(String),
    InlineBytes(Vec<u8>),
    Generated,
    Candidate(CandidateId),
    Assessment(AssessmentId),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EntrySourceRef {
    Inline,
    Generated,
    Candidate(CandidateId),
    Assessment(AssessmentId),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EntryProjection {
    Full,
    Summary,
    Inline,
    Generated,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Placement {
    pub path: WorkspacePath,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EntryAccess {
    InputReadOnly,
    EditableArtifact,
    OutputWritable,
}
