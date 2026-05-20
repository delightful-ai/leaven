//! Command and agent trajectory evidence.

use std::{collections::BTreeSet, time::Duration};

use leaven_core::Evidence;
use leaven_kernel::{AgentSessionId, BlobRef, CaseId, Fingerprint};
use serde::{Deserialize, Serialize};

/// Command output carried inline or by external blob reference.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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

/// Whether one agent trajectory solved its assigned task.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AgentTrajectoryOutcome {
    /// The trajectory produced an accepted answer/output.
    Success,
    /// The trajectory failed, with the runner/scorer supplied reason.
    Failure {
        /// Failure reason from the runner, scorer, or analyzer.
        reason: String,
    },
}

/// Kind of analysis record derived from an agent trajectory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AgentTrajectoryAnalysisKind {
    /// Analysis of a failed trajectory.
    Error,
    /// Analysis of a successful trajectory.
    Success,
    /// Analysis that combines successful and failed trajectories.
    Combined,
    /// Domain-specific analysis kind.
    Custom(String),
}

/// Parsed or blob-backed analyst record derived from one trajectory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentTrajectoryAnalysisRecord {
    kind: AgentTrajectoryAnalysisKind,
    source_file: String,
    payload: OutputRecord,
}

impl AgentTrajectoryAnalysisRecord {
    /// Build an analysis record from a source artifact and parsed payload.
    #[must_use]
    pub fn new(
        kind: AgentTrajectoryAnalysisKind,
        source_file: impl Into<String>,
        payload: OutputRecord,
    ) -> Self {
        Self {
            kind,
            source_file: source_file.into(),
            payload,
        }
    }

    /// Whether this analysis came from a failed or successful trajectory.
    #[must_use]
    pub fn kind(&self) -> AgentTrajectoryAnalysisKind {
        self.kind.clone()
    }

    /// Source report or parsed-record file.
    #[must_use]
    pub fn source_file(&self) -> &str {
        &self.source_file
    }

    /// Parsed record payload, inline or blob-backed.
    #[must_use]
    pub const fn payload(&self) -> &OutputRecord {
        &self.payload
    }
}

/// Evidence for one agent/session trajectory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentTrajectoryEvidenceInput {
    /// Agent runtime session id.
    pub session_id: AgentSessionId,
    /// Leaven case id, when the trajectory came from a lowered case set.
    pub case_id: Option<CaseId>,
    /// Upstream task id used by the benchmark/reproduction.
    pub task_id: String,
    /// Typed success/failure outcome for selection and analysis.
    pub outcome: AgentTrajectoryOutcome,
    /// Model identifier used by the trajectory runner.
    pub model_id: String,
    /// Fingerprint of behavior-affecting model/runtime configuration.
    pub model_config_fingerprint: Fingerprint,
    /// Transcript or transcript reference.
    pub transcript: OutputRecord,
    /// Commands run during the trajectory.
    pub commands: CommandEvidence,
}

/// Evidence for one agent/session trajectory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentTrajectoryEvidence {
    session_id: AgentSessionId,
    case_id: Option<CaseId>,
    task_id: String,
    outcome: AgentTrajectoryOutcome,
    model_id: String,
    model_config_fingerprint: Fingerprint,
    transcript: OutputRecord,
    commands: CommandEvidence,
    analysis_records: Vec<AgentTrajectoryAnalysisRecord>,
}

impl AgentTrajectoryEvidence {
    /// Build trajectory evidence from task, runtime, transcript, and command records.
    #[must_use]
    pub fn new(input: AgentTrajectoryEvidenceInput) -> Self {
        Self {
            session_id: input.session_id,
            case_id: input.case_id,
            task_id: input.task_id,
            outcome: input.outcome,
            model_id: input.model_id,
            model_config_fingerprint: input.model_config_fingerprint,
            transcript: input.transcript,
            commands: input.commands,
            analysis_records: Vec::new(),
        }
    }

    /// Replaces the derived analysis records for this trajectory.
    #[must_use]
    pub fn with_analysis_records(
        mut self,
        records: impl IntoIterator<Item = AgentTrajectoryAnalysisRecord>,
    ) -> Self {
        self.analysis_records = records.into_iter().collect();
        self
    }

    /// Agent runtime session id.
    #[must_use]
    pub const fn session_id(&self) -> AgentSessionId {
        self.session_id
    }

    /// Leaven case id, when the trajectory came from a lowered case set.
    #[must_use]
    pub const fn case_id(&self) -> Option<CaseId> {
        self.case_id
    }

    /// Upstream task id used by the benchmark/reproduction.
    #[must_use]
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Typed success/failure outcome for selection and analysis.
    #[must_use]
    pub const fn outcome(&self) -> &AgentTrajectoryOutcome {
        &self.outcome
    }

    /// Model identifier used by the trajectory runner.
    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Fingerprint of behavior-affecting model/runtime configuration.
    #[must_use]
    pub const fn model_config_fingerprint(&self) -> Fingerprint {
        self.model_config_fingerprint
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

    /// Parsed or blob-backed analyst records derived from this trajectory.
    #[must_use]
    pub fn analysis_records(&self) -> &[AgentTrajectoryAnalysisRecord] {
        &self.analysis_records
    }
}

impl Evidence for AgentTrajectoryEvidence {}

/// Checkpointable corpus of agent trajectories over a known task manifest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentTrajectoryCorpusEvidence {
    expected_task_ids: Vec<String>,
    trajectories: Vec<AgentTrajectoryEvidence>,
}

impl AgentTrajectoryCorpusEvidence {
    /// Build an empty corpus for a caller-declared task manifest.
    pub fn new(
        task_ids: impl IntoIterator<Item = String>,
    ) -> Result<Self, AgentTrajectoryCorpusError> {
        let mut seen = BTreeSet::new();
        let mut expected_task_ids = Vec::new();
        for task_id in task_ids {
            if !seen.insert(task_id.clone()) {
                return Err(AgentTrajectoryCorpusError::DuplicateTask { task_id });
            }
            expected_task_ids.push(task_id);
        }
        Ok(Self {
            expected_task_ids,
            trajectories: Vec::new(),
        })
    }

    /// Adds one trajectory for a known task.
    pub fn push(
        &mut self,
        trajectory: AgentTrajectoryEvidence,
    ) -> Result<(), AgentTrajectoryCorpusError> {
        if !self
            .expected_task_ids
            .iter()
            .any(|task_id| task_id == trajectory.task_id())
        {
            return Err(AgentTrajectoryCorpusError::UnknownTask {
                task_id: trajectory.task_id().to_owned(),
            });
        }
        self.trajectories.push(trajectory);
        Ok(())
    }

    /// Caller-declared task ids in manifest order.
    #[must_use]
    pub fn expected_task_ids(&self) -> &[String] {
        &self.expected_task_ids
    }

    /// Stored trajectories in append/checkpoint order.
    #[must_use]
    pub fn trajectories(&self) -> &[AgentTrajectoryEvidence] {
        &self.trajectories
    }

    /// Trajectories for one task id in append order.
    #[must_use]
    pub fn by_task(&self, task_id: &str) -> Vec<&AgentTrajectoryEvidence> {
        self.trajectories
            .iter()
            .filter(|trajectory| trajectory.task_id() == task_id)
            .collect()
    }

    /// Task ids with at least one stored trajectory, in manifest order.
    #[must_use]
    pub fn completed_task_ids(&self) -> Vec<&str> {
        self.expected_task_ids
            .iter()
            .filter_map(|task_id| {
                self.trajectories
                    .iter()
                    .any(|trajectory| trajectory.task_id() == task_id)
                    .then_some(task_id.as_str())
            })
            .collect()
    }

    /// Task ids with no stored trajectories yet, in manifest order.
    #[must_use]
    pub fn pending_task_ids(&self) -> Vec<&str> {
        self.expected_task_ids
            .iter()
            .filter_map(|task_id| {
                (!self
                    .trajectories
                    .iter()
                    .any(|trajectory| trajectory.task_id() == task_id))
                .then_some(task_id.as_str())
            })
            .collect()
    }
}

impl Evidence for AgentTrajectoryCorpusEvidence {}

/// Corpus construction or insertion refused invalid input.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AgentTrajectoryCorpusError {
    /// Task id appeared more than once in the caller-declared manifest.
    #[error("trajectory corpus manifest has duplicate task id `{task_id}`")]
    DuplicateTask {
        /// Duplicate task id.
        task_id: String,
    },
    /// Trajectory task id was not declared in the corpus manifest.
    #[error("trajectory task id `{task_id}` is not in the corpus manifest")]
    UnknownTask {
        /// Unknown task id.
        task_id: String,
    },
}
