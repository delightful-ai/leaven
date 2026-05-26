//! Agent analyst call and fanout evidence.

use std::collections::BTreeSet;

use leaven_core::Evidence;
use serde::{Deserialize, Serialize};

use crate::OutputRecord;

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
