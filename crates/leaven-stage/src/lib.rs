//! Optimizer-owned agentic stage workspace setup and query support.
//!
//! This crate helps Leaven optimizer stages give an agent a bounded workspace
//! and read back a typed decision. It is not a user task-package framework.

pub mod agent_backed;
pub mod artifact;
pub mod bootstrap;
pub mod entry;
pub mod error;
pub mod media;
pub mod output;
pub mod parser;
pub mod plan;
pub mod query;
pub mod read_authority;
pub mod receipt;
pub mod receipt_store;
pub mod setup;
pub mod slots;
pub mod tool;

pub use agent_backed::{AgentBacked, AgentBackedPolicy, ParseFailurePolicy, ReceiptSinkPolicy};
pub use artifact::{
    ArtifactReadbackError, MaterializableArtifact, MaterializationReport, ReconstructibleArtifact,
};
pub use bootstrap::AgentStageBootstrap;
pub use error::{
    Diagnostic, DiagnosticSeverity, StageBootstrapError, StageOutputContractError,
    StageOutputParseError, StageQueryError, StageReadError, WorkspaceSetupError,
};
pub use media::MediaType;
pub use output::{OutputEntry, OutputEntryId, OutputRole, OutputSchema, StageOutputContract};
pub use parser::StageOutputParser;
pub use plan::{AgentStageCallContext, AgentStagePlan, StageDirective};
pub use query::{AllowedQuerySet, StageQuery, StageQueryKind, StageQueryPolicy};
pub use slots::{ProposerSlot, SlotMarker};

pub use leaven_kernel::StageRole;

pub use entry::{
    EntryAccess, EntryProjection, EntrySource, EntrySourceRef, Placement, WorkspaceEntry,
    WorkspaceEntryRole,
};
pub use read_authority::{QueryEffect, QueryResult, StageReadAuthority};
pub use setup::{StageAttemptReceiptBuilder, setup_stage_workspace};

pub use receipt::{
    OutputEntryReceipt, OutputEntryStatus, ParseReceipt, ParseStatus, QueryRecord,
    QueryRecordEffect, QueryTiming, StageAttemptReceipt, WorkspaceEntryReceipt,
    WorkspaceSetupReceipt,
};

pub mod prelude {
    pub use crate::{
        AgentBacked, AgentBackedPolicy, AgentStageBootstrap, AgentStagePlan,
        MaterializableArtifact, OutputEntry, OutputRole, ProposerSlot, SlotMarker, StageDirective,
        StageOutputContract, StageOutputParser, StageQueryPolicy,
    };
    pub use leaven_kernel::StageRole;
}
