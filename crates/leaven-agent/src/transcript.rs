//! Provider-neutral transcript records.

use leaven_workspace::{Command, CommandOutput, WorkspacePath};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AgentTranscript {
    pub events: Vec<TranscriptEvent>,
}

impl AgentTranscript {
    pub fn push_message(&mut self, role: TranscriptRole, content: impl Into<String>) {
        self.events.push(TranscriptEvent::Message {
            role,
            content: content.into(),
        });
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranscriptEvent {
    Message {
        role: TranscriptRole,
        content: String,
    },
    ToolCall {
        record: ToolCallRecord,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolCallRecord {
    pub name: String,
    pub input: String,
    pub output: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandRecord {
    pub command: Command,
    pub output: CommandOutput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawProviderEvent {
    pub kind: String,
    pub payload: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceReadRecord {
    pub path: WorkspacePath,
    pub bytes: usize,
}
