//! Skill-use telemetry evidence.

use leaven_artifact_skill::SkillName;
use leaven_core::Evidence;
use leaven_kernel::FiniteF64;
use serde::{Deserialize, Serialize};

use crate::OutputRecord;

/// Capability trait for evidence that records skill-use events.
pub trait SkillUseEvidence: Evidence {
    /// Skill-use events in observed or parser-emitted order.
    fn skill_events(&self) -> &[SkillUseEvent];
}

/// Kind of skill-use event observed in a trajectory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SkillUseKind {
    /// A router or policy selected the skill for the context.
    Retrieved,
    /// The selected skill was injected into the agent context.
    Injected,
    /// Runtime or parser evidence says the skill affected behavior.
    Triggered,
}

/// Source that produced one skill-use event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SkillUseSource {
    /// Router or retrieval instrumentation emitted the event.
    Router,
    /// Runtime telemetry emitted the event.
    RuntimeTelemetry,
    /// Transcript parsing inferred the event.
    TranscriptParser,
    /// Scorer or evaluator instrumentation emitted the event.
    Scorer,
    /// Human-entered or reproduction-specific annotation.
    Manual,
    /// Caller-defined event source.
    Custom(String),
}

/// Confidence level for one skill-use event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SkillUseConfidence {
    /// The event was directly observed by telemetry, instrumentation, or a scorer.
    Observed,
    /// The event was inferred from transcript, filesystem, or outcome evidence.
    Inferred,
    /// The producer did not classify confidence.
    Unknown,
}

/// One retrieved, injected, or triggered skill event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkillUseEvent {
    skill: SkillName,
    kind: SkillUseKind,
    source: SkillUseSource,
    confidence: SkillUseConfidence,
    step_index: Option<u64>,
    evidence: Option<OutputRecord>,
}

impl SkillUseEvent {
    /// Build a skill-use event.
    #[must_use]
    pub const fn new(
        skill: SkillName,
        kind: SkillUseKind,
        source: SkillUseSource,
        confidence: SkillUseConfidence,
    ) -> Self {
        Self {
            skill,
            kind,
            source,
            confidence,
            step_index: None,
            evidence: None,
        }
    }

    /// Attach a zero-based trajectory step index.
    #[must_use]
    pub const fn with_step_index(mut self, step_index: u64) -> Self {
        self.step_index = Some(step_index);
        self
    }

    /// Attach durable supporting output for this event.
    #[must_use]
    pub fn with_evidence(mut self, evidence: OutputRecord) -> Self {
        self.evidence = Some(evidence);
        self
    }

    /// Skill identity associated with this event.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Event kind.
    #[must_use]
    pub const fn kind(&self) -> &SkillUseKind {
        &self.kind
    }

    /// Event producer.
    #[must_use]
    pub const fn source(&self) -> &SkillUseSource {
        &self.source
    }

    /// Event confidence.
    #[must_use]
    pub const fn confidence(&self) -> &SkillUseConfidence {
        &self.confidence
    }

    /// Optional zero-based trajectory step index.
    #[must_use]
    pub const fn step_index(&self) -> Option<u64> {
        self.step_index
    }

    /// Supporting output or blob reference for this event.
    #[must_use]
    pub const fn evidence(&self) -> Option<&OutputRecord> {
        self.evidence.as_ref()
    }
}

/// Skill-use evidence for one task trajectory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillTrajectoryUseEvidence {
    task_id: String,
    trajectory_id: String,
    reward: FiniteF64,
    events: Vec<SkillUseEvent>,
}

impl SkillTrajectoryUseEvidence {
    /// Build skill-use evidence for one task trajectory.
    ///
    /// # Errors
    ///
    /// Returns [`SkillTrajectoryUseEvidenceError`] when task or trajectory
    /// identity is blank after trimming.
    pub fn new(
        task_id: impl Into<String>,
        trajectory_id: impl Into<String>,
        reward: FiniteF64,
        events: Vec<SkillUseEvent>,
    ) -> Result<Self, SkillTrajectoryUseEvidenceError> {
        let task_id = task_id.into();
        if task_id.trim().is_empty() {
            return Err(SkillTrajectoryUseEvidenceError::EmptyTaskId);
        }
        let trajectory_id = trajectory_id.into();
        if trajectory_id.trim().is_empty() {
            return Err(SkillTrajectoryUseEvidenceError::EmptyTrajectoryId);
        }

        Ok(Self {
            task_id,
            trajectory_id,
            reward,
            events,
        })
    }

    /// Upstream task id for this trajectory.
    #[must_use]
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Runner-provided trajectory identity.
    #[must_use]
    pub fn trajectory_id(&self) -> &str {
        &self.trajectory_id
    }

    /// Terminal reward or success indicator for this trajectory.
    #[must_use]
    pub const fn reward(&self) -> FiniteF64 {
        self.reward
    }

    /// Retrieved skills in event order.
    #[must_use]
    pub fn retrieved_skills(&self) -> Vec<&SkillName> {
        self.events
            .iter()
            .filter(|event| matches!(event.kind(), SkillUseKind::Retrieved))
            .map(SkillUseEvent::skill)
            .collect()
    }

    /// Events for one skill in event order.
    #[must_use]
    pub fn events_for_skill(&self, skill: &SkillName) -> Vec<&SkillUseEvent> {
        self.events
            .iter()
            .filter(|event| event.skill() == skill)
            .collect()
    }
}

impl SkillUseEvidence for SkillTrajectoryUseEvidence {
    fn skill_events(&self) -> &[SkillUseEvent] {
        &self.events
    }
}

impl Evidence for SkillTrajectoryUseEvidence {}

/// Refusal reasons for skill trajectory use evidence construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SkillTrajectoryUseEvidenceError {
    /// The upstream task id was blank.
    #[error("skill trajectory use evidence requires a non-empty task id")]
    EmptyTaskId,
    /// The trajectory id was blank.
    #[error("skill trajectory use evidence requires a non-empty trajectory id")]
    EmptyTrajectoryId,
}
