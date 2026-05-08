use leaven_agent::{AgentSession, AgentStatus, TranscriptEvent, TranscriptRole};
use leaven_core::Evidence;
use leaven_kernel::CandidateId;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum EvoSkillEvidence {
    Evaluation {
        candidate: CandidateId,
        split: String,
        average_score: f64,
        cases: Vec<CaseExecution>,
    },
    AgentRoleSession {
        role: AgentRole,
        developer_instructions: String,
        evidence: StoredAgentSession,
    },
}

impl Evidence for EvoSkillEvidence {}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct CaseExecution {
    pub case_id: String,
    pub question: String,
    pub expected_answer: String,
    pub predicted_answer: String,
    pub score: f64,
    pub passed: bool,
    pub developer_instructions: String,
    pub session: StoredAgentSession,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentRole {
    Executor,
    Proposer,
    SkillBuilder,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct StoredAgentSession {
    pub status: String,
    pub transcript: Vec<StoredTranscriptEvent>,
    pub output_files: Vec<String>,
    pub raw_provider_event_count: usize,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct StoredTranscriptEvent {
    pub role: String,
    pub content: String,
}

impl StoredAgentSession {
    pub fn from_session(session: &AgentSession) -> Self {
        Self {
            status: status(&session.status),
            transcript: session
                .transcript
                .events
                .iter()
                .filter_map(stored_event)
                .collect(),
            output_files: session
                .output_files
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect(),
            raw_provider_event_count: session.raw_provider_events.len(),
        }
    }
}

fn stored_event(event: &TranscriptEvent) -> Option<StoredTranscriptEvent> {
    match event {
        TranscriptEvent::Message { role, content } => Some(StoredTranscriptEvent {
            role: role_name(*role),
            content: content.clone(),
        }),
        TranscriptEvent::ToolCall { .. } => None,
    }
}

fn role_name(role: TranscriptRole) -> String {
    match role {
        TranscriptRole::System => "system",
        TranscriptRole::User => "user",
        TranscriptRole::Assistant => "assistant",
        TranscriptRole::Tool => "tool",
    }
    .to_owned()
}

fn status(status: &AgentStatus) -> String {
    match status {
        AgentStatus::Succeeded => "succeeded".to_owned(),
        AgentStatus::Failed { reason } => format!("failed:{reason}"),
        AgentStatus::Cancelled => "cancelled".to_owned(),
        AgentStatus::TimedOut => "timed-out".to_owned(),
        AgentStatus::OutputContractViolation { message } => {
            format!("output-contract-violation:{message}")
        }
    }
}
