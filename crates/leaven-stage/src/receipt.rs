use leaven_kernel::{
    Cost, Fingerprint, MetadataBag, StageAttemptOutcome, StageAttemptReceiptId, StageCallId,
    StageQueryId, StageRole, WorkspaceEntryId, WorkspaceId,
};
use leaven_workspace::{WorkspaceFileFingerprint, WorkspacePath};
use serde::{Deserialize, Serialize};

use crate::entry::EntrySourceRef;
use crate::{
    Diagnostic, EntryAccess, EntryProjection, OutputEntryId, OutputRole, StageQuery,
    WorkspaceEntryRole,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageAttemptReceipt {
    pub receipt_id: StageAttemptReceiptId,
    pub workspace_id: WorkspaceId,
    pub stage_call_id: StageCallId,
    pub role: StageRole,
    pub plan_fingerprint: Fingerprint,
    pub setup: WorkspaceSetupReceipt,
    pub queries: Vec<QueryRecord>,
    pub outputs: Vec<OutputEntryReceipt>,
    pub parse: Option<ParseReceipt>,
    pub cost: Cost,
    pub outcome: StageAttemptOutcome,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkspaceSetupReceipt {
    pub plan_entries: Vec<WorkspaceEntryReceipt>,
    pub diagnostics: Vec<Diagnostic>,
    pub cost: Cost,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryRecord {
    pub query_id: StageQueryId,
    pub timing: QueryTiming,
    pub query: StageQuery,
    pub effect: QueryRecordEffect,
    pub entries: Vec<WorkspaceEntryReceipt>,
    pub cost: Cost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum QueryTiming {
    Prewarm,
    AgentRequested,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QueryRecordEffect {
    WroteEntries(Vec<WorkspaceEntryId>),
    ReturnedSummary(String),
    NotVisible(String),
    NotFound(String),
    PolicyDenied(String),
    Error(Vec<Diagnostic>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceEntryReceipt {
    pub id: WorkspaceEntryId,
    pub path: WorkspacePath,
    pub role: WorkspaceEntryRole,
    pub source: EntrySourceRef,
    pub projection: EntryProjection,
    pub access: EntryAccess,
    pub fingerprint: Fingerprint,
    pub file: Option<WorkspaceFileFingerprint>,
    pub bytes: Option<u64>,
    pub produced_by_query: Option<StageQueryId>,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputEntryReceipt {
    pub id: OutputEntryId,
    pub path: WorkspacePath,
    pub role: OutputRole,
    pub fingerprint: Option<Fingerprint>,
    pub file: Option<WorkspaceFileFingerprint>,
    pub bytes: Option<u64>,
    pub status: OutputEntryStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OutputEntryStatus {
    Present,
    Missing,
    TooLarge,
    InvalidMedia,
    NotRead,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParseReceipt {
    pub status: ParseStatus,
    pub diagnostics: Vec<Diagnostic>,
    pub files_read: Vec<WorkspacePath>,
    pub cost: Cost,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ParseStatus {
    NotAttempted,
    Succeeded,
    Failed,
}
