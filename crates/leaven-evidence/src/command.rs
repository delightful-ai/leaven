//! Command, agent trajectory, and analyst evidence.

use std::time::Duration;

use leaven_core::Evidence;
use serde::{Deserialize, Serialize};

use crate::OutputRecord;

mod analyst;
mod merge_tree;
mod trajectory;

pub use analyst::{
    AgentAnalystCallError, AgentAnalystCallEvidence, AgentAnalystCallEvidenceInput,
    AgentAnalystCallStatus, AgentAnalystFanoutError, AgentAnalystFanoutEvidence, AgentAnalystRole,
};

pub use merge_tree::{
    AgentPatchMergeDecision, AgentPatchMergeNode, AgentPatchMergeNodeInput,
    AgentPatchMergeTreeError, AgentPatchMergeTreeEvidence,
};
pub use trajectory::{
    AgentTrajectoryAnalysisKind, AgentTrajectoryAnalysisRecord, AgentTrajectoryCorpusError,
    AgentTrajectoryCorpusEvidence, AgentTrajectoryEvidence, AgentTrajectoryEvidenceInput,
    AgentTrajectoryOutcome,
};

/// One command execution record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommandRecord {
    command: String,
    exit_status: Option<i32>,
    stdout: OutputRecord,
    stderr: OutputRecord,
    duration: Duration,
}

impl CommandRecord {
    /// Build a command execution record.
    #[must_use]
    pub fn new(
        command: impl Into<String>,
        exit_status: Option<i32>,
        stdout: OutputRecord,
        stderr: OutputRecord,
        duration: Duration,
    ) -> Self {
        Self {
            command: command.into(),
            exit_status,
            stdout,
            stderr,
            duration,
        }
    }

    /// Command string as executed or displayed by the stage.
    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    /// Process exit status code, when the backend supplied one.
    #[must_use]
    pub const fn exit_status(&self) -> Option<i32> {
        self.exit_status
    }

    /// Captured stdout.
    #[must_use]
    pub const fn stdout(&self) -> &OutputRecord {
        &self.stdout
    }

    /// Captured stderr.
    #[must_use]
    pub const fn stderr(&self) -> &OutputRecord {
        &self.stderr
    }

    /// Execution duration.
    #[must_use]
    pub const fn duration(&self) -> Duration {
        self.duration
    }
}

/// Evidence made of command execution records.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommandEvidence {
    records: Vec<CommandRecord>,
}

impl CommandEvidence {
    /// Build command evidence from records.
    #[must_use]
    pub fn new(records: Vec<CommandRecord>) -> Self {
        Self { records }
    }

    /// Command execution records.
    #[must_use]
    pub fn records(&self) -> &[CommandRecord] {
        &self.records
    }
}

impl Evidence for CommandEvidence {}
