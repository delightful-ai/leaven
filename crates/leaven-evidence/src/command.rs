//! Command and agent trajectory evidence.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

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

/// Role assigned to one agent analyst call.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AgentAnalystRole {
    /// Analyst over failed trajectories.
    Error,
    /// Analyst over successful trajectories.
    Success,
    /// Analyst over mixed success/failure evidence.
    Combined,
    /// Domain-specific analyst role.
    Custom(String),
}

/// Durable terminal or resumable state for one analyst call.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AgentAnalystCallStatus {
    /// The call has been declared but has not produced a terminal result.
    Pending,
    /// The call produced a parsed, usable response.
    Succeeded,
    /// The call produced output, but parsing/validation failed.
    ParseFailed {
        /// Parse or validation failure reason.
        reason: String,
        /// Optional artifact containing the raw failure payload or diagnostics.
        artifact: Option<OutputRecord>,
    },
    /// The backend failed before producing a usable response.
    Failed {
        /// Backend or runtime failure reason.
        reason: String,
    },
}

/// Input for one durable analyst-call evidence record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentAnalystCallEvidenceInput {
    /// Stable call id from the caller's fan-out manifest.
    pub call_id: String,
    /// Analyst role for this call.
    pub role: AgentAnalystRole,
    /// Source task ids or trajectory ids this call analyzed.
    pub source_task_ids: Vec<String>,
    /// Prompt sent to the analyst.
    pub prompt: OutputRecord,
    /// Raw response payload, when a backend response exists.
    pub response: Option<OutputRecord>,
    /// Durable call status.
    pub status: AgentAnalystCallStatus,
    /// Number of retries already spent for this call.
    pub retry_count: u32,
    /// Number of trajectory/task observations supporting the call's proposed patch or lesson.
    pub support_count: u32,
}

/// Durable evidence for one agent analyst call.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentAnalystCallEvidence {
    call_id: String,
    role: AgentAnalystRole,
    source_task_ids: Vec<String>,
    prompt: OutputRecord,
    response: Option<OutputRecord>,
    status: AgentAnalystCallStatus,
    retry_count: u32,
    support_count: u32,
}

impl AgentAnalystCallEvidence {
    /// Build one analyst-call evidence record.
    pub fn new(input: AgentAnalystCallEvidenceInput) -> Result<Self, AgentAnalystCallError> {
        if input.call_id.is_empty() {
            return Err(AgentAnalystCallError::EmptyCallId);
        }
        if input.source_task_ids.is_empty() {
            return Err(AgentAnalystCallError::EmptySourceTasks);
        }
        if input.support_count == 0 {
            return Err(AgentAnalystCallError::EmptySupport);
        }
        Ok(Self {
            call_id: input.call_id,
            role: input.role,
            source_task_ids: input.source_task_ids,
            prompt: input.prompt,
            response: input.response,
            status: input.status,
            retry_count: input.retry_count,
            support_count: input.support_count,
        })
    }

    /// Stable call id from the caller's fan-out manifest.
    #[must_use]
    pub fn call_id(&self) -> &str {
        &self.call_id
    }

    /// Analyst role for this call.
    #[must_use]
    pub fn role(&self) -> AgentAnalystRole {
        self.role.clone()
    }

    /// Source task ids or trajectory ids this call analyzed.
    #[must_use]
    pub fn source_task_ids(&self) -> &[String] {
        &self.source_task_ids
    }

    /// Prompt sent to the analyst.
    #[must_use]
    pub const fn prompt(&self) -> &OutputRecord {
        &self.prompt
    }

    /// Raw response payload, when a backend response exists.
    #[must_use]
    pub const fn response(&self) -> Option<&OutputRecord> {
        self.response.as_ref()
    }

    /// Durable call status.
    #[must_use]
    pub const fn status(&self) -> &AgentAnalystCallStatus {
        &self.status
    }

    /// Number of retries already spent for this call.
    #[must_use]
    pub const fn retry_count(&self) -> u32 {
        self.retry_count
    }

    /// Number of trajectory/task observations supporting the proposed patch or lesson.
    #[must_use]
    pub const fn support_count(&self) -> u32 {
        self.support_count
    }
}

impl Evidence for AgentAnalystCallEvidence {}

/// Checkpointable fan-out of independent agent analyst calls.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentAnalystFanoutEvidence {
    expected_call_ids: Vec<String>,
    calls: Vec<AgentAnalystCallEvidence>,
}

impl AgentAnalystFanoutEvidence {
    /// Build an empty fan-out for a caller-declared call manifest.
    pub fn new(
        call_ids: impl IntoIterator<Item = String>,
    ) -> Result<Self, AgentAnalystFanoutError> {
        let mut seen = BTreeSet::new();
        let mut expected_call_ids = Vec::new();
        for call_id in call_ids {
            if call_id.is_empty() {
                return Err(AgentAnalystFanoutError::EmptyCallManifestId);
            }
            if !seen.insert(call_id.clone()) {
                return Err(AgentAnalystFanoutError::DuplicateCallManifest { call_id });
            }
            expected_call_ids.push(call_id);
        }
        Ok(Self {
            expected_call_ids,
            calls: Vec::new(),
        })
    }

    /// Adds or replaces one call result for a known call id.
    pub fn push(&mut self, call: AgentAnalystCallEvidence) -> Result<(), AgentAnalystFanoutError> {
        if !self
            .expected_call_ids
            .iter()
            .any(|call_id| call_id == call.call_id())
        {
            return Err(AgentAnalystFanoutError::UnknownCall {
                call_id: call.call_id().to_owned(),
            });
        }
        if let Some(existing) = self
            .calls
            .iter_mut()
            .find(|existing| existing.call_id() == call.call_id())
        {
            *existing = call;
        } else {
            self.calls.push(call);
        }
        Ok(())
    }

    /// Caller-declared call ids in manifest order.
    #[must_use]
    pub fn expected_call_ids(&self) -> &[String] {
        &self.expected_call_ids
    }

    /// Stored call records in checkpoint order.
    #[must_use]
    pub fn calls(&self) -> &[AgentAnalystCallEvidence] {
        &self.calls
    }

    /// Stored call record for one call id.
    #[must_use]
    pub fn by_call(&self, call_id: &str) -> Option<&AgentAnalystCallEvidence> {
        self.calls.iter().find(|call| call.call_id() == call_id)
    }

    /// Call ids with stored call records, in manifest order.
    #[must_use]
    pub fn completed_call_ids(&self) -> Vec<&str> {
        self.expected_call_ids
            .iter()
            .filter_map(|call_id| {
                self.by_call(call_id)
                    .is_some_and(|call| !matches!(call.status(), AgentAnalystCallStatus::Pending))
                    .then_some(call_id.as_str())
            })
            .collect()
    }

    /// Call ids with no terminal call record yet, in manifest order.
    #[must_use]
    pub fn pending_call_ids(&self) -> Vec<&str> {
        self.expected_call_ids
            .iter()
            .filter_map(|call_id| {
                self.by_call(call_id)
                    .is_none_or(|call| matches!(call.status(), AgentAnalystCallStatus::Pending))
                    .then_some(call_id.as_str())
            })
            .collect()
    }
}

impl Evidence for AgentAnalystFanoutEvidence {}

/// Analyst call construction refused invalid input.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AgentAnalystCallError {
    /// Call id was empty.
    #[error("analyst call id is empty")]
    EmptyCallId,
    /// No source tasks were recorded for this analyst call.
    #[error("analyst call has no source task ids")]
    EmptySourceTasks,
    /// Support count must be positive.
    #[error("analyst call support count must be positive")]
    EmptySupport,
}

/// Fan-out construction or insertion refused invalid input.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AgentAnalystFanoutError {
    /// Call id appeared empty in the caller-declared manifest.
    #[error("analyst fanout manifest contains an empty call id")]
    EmptyCallManifestId,
    /// Call id appeared more than once in the caller-declared manifest.
    #[error("analyst fanout manifest has duplicate call id `{call_id}`")]
    DuplicateCallManifest {
        /// Duplicate call id.
        call_id: String,
    },
    /// Call id was not declared in the fan-out manifest.
    #[error("analyst call id `{call_id}` is not in the fanout manifest")]
    UnknownCall {
        /// Unknown call id.
        call_id: String,
    },
}

/// Decision recorded for one patch merge-tree node.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AgentPatchMergeDecision {
    /// Leaf or merge output accepted as usable provenance.
    Accepted {
        /// Human/model rationale for acceptance.
        rationale: String,
    },
    /// Node merged multiple child patches into a consolidated output.
    Merged {
        /// Note about prevalence, support, or consolidation rationale.
        prevalence_note: String,
    },
    /// Node discarded its inputs.
    Discarded {
        /// Reason the inputs were discarded.
        reason: String,
    },
    /// Node output failed parsing or validation.
    ParseFailed {
        /// Parse or validation failure reason.
        reason: String,
    },
}

/// Input for one patch merge-tree node.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentPatchMergeNodeInput {
    /// Stable node id.
    pub node_id: String,
    /// Merge level; leaves are conventionally level 0.
    pub level: u32,
    /// Input patch or child node ids consumed by this node.
    pub input_patch_ids: Vec<String>,
    /// Patch ids accepted or preserved by this node.
    pub accepted_patch_ids: Vec<String>,
    /// Patch ids discarded by this node.
    pub discarded_patch_ids: Vec<String>,
    /// Total support represented by this node.
    pub support_count: u32,
    /// Merge decision for this node.
    pub decision: AgentPatchMergeDecision,
    /// Prompt sent to the merge operator, when this node used one.
    pub prompt: Option<OutputRecord>,
    /// Raw merge response, when one exists.
    pub response: Option<OutputRecord>,
    /// Parse-failure artifact, when parsing failed.
    pub parse_failure: Option<OutputRecord>,
    /// Parsed or blob-backed output patch for this node.
    pub output_patch: Option<OutputRecord>,
}

/// One node in a hierarchical patch merge tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentPatchMergeNode {
    node_id: String,
    level: u32,
    input_patch_ids: Vec<String>,
    accepted_patch_ids: Vec<String>,
    discarded_patch_ids: Vec<String>,
    support_count: u32,
    decision: AgentPatchMergeDecision,
    prompt: Option<OutputRecord>,
    response: Option<OutputRecord>,
    parse_failure: Option<OutputRecord>,
    output_patch: Option<OutputRecord>,
}

impl AgentPatchMergeNode {
    /// Build one merge-tree node.
    pub fn new(input: AgentPatchMergeNodeInput) -> Result<Self, AgentPatchMergeTreeError> {
        if input.node_id.is_empty() {
            return Err(AgentPatchMergeTreeError::EmptyNodeId);
        }
        if input.support_count == 0 {
            return Err(AgentPatchMergeTreeError::EmptySupport);
        }
        Ok(Self {
            node_id: input.node_id,
            level: input.level,
            input_patch_ids: input.input_patch_ids,
            accepted_patch_ids: input.accepted_patch_ids,
            discarded_patch_ids: input.discarded_patch_ids,
            support_count: input.support_count,
            decision: input.decision,
            prompt: input.prompt,
            response: input.response,
            parse_failure: input.parse_failure,
            output_patch: input.output_patch,
        })
    }

    /// Stable node id.
    #[must_use]
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Merge level; leaves are conventionally level 0.
    #[must_use]
    pub const fn level(&self) -> u32 {
        self.level
    }

    /// Input patch or child node ids consumed by this node.
    #[must_use]
    pub fn input_patch_ids(&self) -> &[String] {
        &self.input_patch_ids
    }

    /// Patch ids accepted or preserved by this node.
    #[must_use]
    pub fn accepted_patch_ids(&self) -> &[String] {
        &self.accepted_patch_ids
    }

    /// Patch ids discarded by this node.
    #[must_use]
    pub fn discarded_patch_ids(&self) -> &[String] {
        &self.discarded_patch_ids
    }

    /// Total support represented by this node.
    #[must_use]
    pub const fn support_count(&self) -> u32 {
        self.support_count
    }

    /// Merge decision for this node.
    #[must_use]
    pub const fn decision(&self) -> &AgentPatchMergeDecision {
        &self.decision
    }

    /// Prompt sent to the merge operator, when this node used one.
    #[must_use]
    pub const fn prompt(&self) -> Option<&OutputRecord> {
        self.prompt.as_ref()
    }

    /// Raw merge response, when one exists.
    #[must_use]
    pub const fn response(&self) -> Option<&OutputRecord> {
        self.response.as_ref()
    }

    /// Parse-failure artifact, when parsing failed.
    #[must_use]
    pub const fn parse_failure(&self) -> Option<&OutputRecord> {
        self.parse_failure.as_ref()
    }

    /// Parsed or blob-backed output patch for this node.
    #[must_use]
    pub const fn output_patch(&self) -> Option<&OutputRecord> {
        self.output_patch.as_ref()
    }
}

/// Evidence for a hierarchical patch merge tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentPatchMergeTreeEvidence {
    nodes: Vec<AgentPatchMergeNode>,
    final_node_id: String,
    final_diff: Option<OutputRecord>,
}

impl AgentPatchMergeTreeEvidence {
    /// Build merge-tree evidence from validated nodes and a final node id.
    pub fn new(
        nodes: Vec<AgentPatchMergeNode>,
        final_node_id: impl Into<String>,
        final_diff: Option<OutputRecord>,
    ) -> Result<Self, AgentPatchMergeTreeError> {
        if nodes.is_empty() {
            return Err(AgentPatchMergeTreeError::EmptyTree);
        }
        let final_node_id = final_node_id.into();
        if final_node_id.is_empty() {
            return Err(AgentPatchMergeTreeError::EmptyFinalNode);
        }
        let mut seen = BTreeSet::new();
        for node in &nodes {
            if !seen.insert(node.node_id.clone()) {
                return Err(AgentPatchMergeTreeError::DuplicateNode {
                    node_id: node.node_id.clone(),
                });
            }
        }
        if !seen.contains(&final_node_id) {
            return Err(AgentPatchMergeTreeError::UnknownFinalNode {
                node_id: final_node_id,
            });
        }
        Ok(Self {
            nodes,
            final_node_id,
            final_diff,
        })
    }

    /// Merge-tree nodes in caller-supplied checkpoint order.
    #[must_use]
    pub fn nodes(&self) -> &[AgentPatchMergeNode] {
        &self.nodes
    }

    /// Final/root node id.
    #[must_use]
    pub fn final_node_id(&self) -> &str {
        &self.final_node_id
    }

    /// Final/root node.
    #[must_use]
    pub fn final_node(&self) -> &AgentPatchMergeNode {
        self.nodes
            .iter()
            .find(|node| node.node_id() == self.final_node_id)
            .expect("constructor verifies final node exists")
    }

    /// Final applied diff or diff artifact, when available.
    #[must_use]
    pub const fn final_diff(&self) -> Option<&OutputRecord> {
        self.final_diff.as_ref()
    }

    /// Sorted merge levels present in this tree.
    #[must_use]
    pub fn levels(&self) -> Vec<u32> {
        self.nodes
            .iter()
            .map(AgentPatchMergeNode::level)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Nodes at one merge level in checkpoint order.
    #[must_use]
    pub fn nodes_at_level(&self, level: u32) -> Vec<&AgentPatchMergeNode> {
        self.nodes
            .iter()
            .filter(|node| node.level() == level)
            .collect()
    }

    /// Unique accepted patch ids in first-seen order.
    #[must_use]
    pub fn accepted_patch_ids(&self) -> Vec<&str> {
        unique_patch_ids(
            self.nodes
                .iter()
                .flat_map(AgentPatchMergeNode::accepted_patch_ids),
        )
    }

    /// Unique discarded patch ids in first-seen order.
    #[must_use]
    pub fn discarded_patch_ids(&self) -> Vec<&str> {
        unique_patch_ids(
            self.nodes
                .iter()
                .flat_map(AgentPatchMergeNode::discarded_patch_ids),
        )
    }

    /// Nodes with parse-failure decisions.
    #[must_use]
    pub fn parse_failed_nodes(&self) -> Vec<&AgentPatchMergeNode> {
        self.nodes
            .iter()
            .filter(|node| matches!(node.decision(), AgentPatchMergeDecision::ParseFailed { .. }))
            .collect()
    }

    /// Sum of support by merge level.
    #[must_use]
    pub fn support_by_level(&self) -> BTreeMap<u32, u32> {
        let mut support = BTreeMap::new();
        for node in &self.nodes {
            *support.entry(node.level()).or_insert(0) += node.support_count();
        }
        support
    }
}

impl Evidence for AgentPatchMergeTreeEvidence {}

fn unique_patch_ids<'a>(ids: impl IntoIterator<Item = &'a String>) -> Vec<&'a str> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for id in ids {
        if seen.insert(id.as_str()) {
            unique.push(id.as_str());
        }
    }
    unique
}

/// Merge-tree construction refused invalid input.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AgentPatchMergeTreeError {
    /// Node id was empty.
    #[error("patch merge node id is empty")]
    EmptyNodeId,
    /// Node support count must be positive.
    #[error("patch merge node support count must be positive")]
    EmptySupport,
    /// Tree contained no nodes.
    #[error("patch merge tree is empty")]
    EmptyTree,
    /// Final node id was empty.
    #[error("patch merge tree final node id is empty")]
    EmptyFinalNode,
    /// Node id appeared more than once.
    #[error("patch merge tree has duplicate node id `{node_id}`")]
    DuplicateNode {
        /// Duplicate node id.
        node_id: String,
    },
    /// Final node id was not present in the tree.
    #[error("patch merge tree final node `{node_id}` is not present")]
    UnknownFinalNode {
        /// Missing final node id.
        node_id: String,
    },
}
