//! Command and agent trajectory evidence.

use std::time::Duration;

use leaven_core::Evidence;
use leaven_kernel::BlobRef;

/// Command output carried inline or by external blob reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutputRecord {
    /// Bounded inline output text.
    Inline {
        /// Captured output snippet.
        text: String,
        /// Whether the full output was truncated to this snippet.
        truncated: bool,
    },
    /// Output stored outside the graph.
    BlobRef(BlobRef),
}

impl OutputRecord {
    /// Build an untruncated inline output record.
    #[must_use]
    pub fn inline(text: impl Into<String>) -> Self {
        Self::Inline {
            text: text.into(),
            truncated: false,
        }
    }

    /// Build a truncated inline output record.
    #[must_use]
    pub fn truncated(text: impl Into<String>) -> Self {
        Self::Inline {
            text: text.into(),
            truncated: true,
        }
    }

    /// Build a blob-backed output record.
    #[must_use]
    pub const fn blob(reference: BlobRef) -> Self {
        Self::BlobRef(reference)
    }
}

/// One command execution record.
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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

/// Evidence for one agent/session trajectory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentTrajectoryEvidence {
    transcript: OutputRecord,
    commands: CommandEvidence,
}

impl AgentTrajectoryEvidence {
    /// Build trajectory evidence from transcript and commands.
    #[must_use]
    pub const fn new(transcript: OutputRecord, commands: CommandEvidence) -> Self {
        Self {
            transcript,
            commands,
        }
    }

    /// Transcript or transcript reference.
    #[must_use]
    pub const fn transcript(&self) -> &OutputRecord {
        &self.transcript
    }

    /// Commands run during the trajectory.
    #[must_use]
    pub const fn commands(&self) -> &CommandEvidence {
        &self.commands
    }
}

impl Evidence for AgentTrajectoryEvidence {}
