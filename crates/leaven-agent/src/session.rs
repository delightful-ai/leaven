//! Provider-neutral agent session vocabulary.

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use leaven_kernel::{AgentSessionId, BudgetSnapshot};
use leaven_workspace::{WorkspacePath, WorkspaceView};

use crate::{
    AgentRuntimeError, AgentTranscript, CommandRecord, RawProviderEvent, TranscriptEvent,
    TranscriptRole,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunRequest {
    pub instructions: AgentInstructions,
    pub cwd: WorkspacePath,
    pub output_contract: OutputContract,
    pub env: BTreeMap<String, String>,
    pub tool_policy: AgentToolPolicy,
    pub limits: AgentLimits,
}

impl AgentRunRequest {
    #[must_use]
    pub fn new(instructions: AgentInstructions, output_contract: OutputContract) -> Self {
        Self {
            instructions,
            cwd: WorkspacePath::root(),
            output_contract,
            env: BTreeMap::new(),
            tool_policy: AgentToolPolicy::default(),
            limits: AgentLimits::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentInstructions {
    pub system: Option<String>,
    pub task: String,
    pub context: Vec<AgentContextRef>,
}

impl AgentInstructions {
    #[must_use]
    pub fn task(task: impl Into<String>) -> Self {
        Self {
            system: None,
            task: task.into(),
            context: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentContextRef {
    pub label: String,
    pub path: WorkspacePath,
    pub media_type: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutputContract {
    Files {
        paths: Vec<WorkspacePath>,
    },
    JsonFile {
        path: WorkspacePath,
        schema: Option<JsonSchemaRef>,
    },
    FinalMessage,
    WorkspaceDiff {
        roots: Vec<WorkspacePath>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonSchemaRef {
    pub name: String,
    pub schema: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentToolPolicy {
    pub allow_shell: bool,
    pub allowed_tools: Vec<String>,
}

impl Default for AgentToolPolicy {
    fn default() -> Self {
        Self {
            allow_shell: true,
            allowed_tools: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AgentLimits {
    pub timeout: Option<Duration>,
    pub max_turns: Option<u32>,
    pub max_output_bytes: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceAccessMode {
    BackendNeutral,
    RequiresLocalMount,
    ProviderManaged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRuntimeCapabilities {
    pub workspace_access: WorkspaceAccessMode,
    pub supports_commands: bool,
    pub supports_raw_provider_events: bool,
}

impl Default for AgentRuntimeCapabilities {
    fn default() -> Self {
        Self {
            workspace_access: WorkspaceAccessMode::BackendNeutral,
            supports_commands: true,
            supports_raw_provider_events: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AgentRunContext<'a> {
    session_id: AgentSessionId,
    budget: &'a BudgetSnapshot,
    cancellation: CancellationRef<'a>,
}

impl<'a> AgentRunContext<'a> {
    #[must_use]
    pub fn new(session_id: AgentSessionId, budget: &'a BudgetSnapshot) -> Self {
        Self {
            session_id,
            budget,
            cancellation: CancellationRef::default(),
        }
    }

    #[must_use]
    pub const fn session_id(self) -> AgentSessionId {
        self.session_id
    }

    #[must_use]
    pub const fn budget(self) -> &'a BudgetSnapshot {
        self.budget
    }

    #[must_use]
    pub const fn cancellation(self) -> CancellationRef<'a> {
        self.cancellation
    }

    #[must_use]
    pub const fn with_cancellation(mut self, cancellation: CancellationRef<'a>) -> Self {
        self.cancellation = cancellation;
        self
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CancellationRef<'a> {
    flag: Option<&'a AtomicBool>,
}

impl<'a> CancellationRef<'a> {
    #[must_use]
    pub const fn from_flag(flag: &'a AtomicBool) -> Self {
        Self { flag: Some(flag) }
    }

    #[must_use]
    pub fn is_cancelled(self) -> bool {
        self.flag
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentSession {
    pub session_id: AgentSessionId,
    pub status: AgentStatus,
    pub transcript: AgentTranscript,
    pub commands: Vec<CommandRecord>,
    pub output_files: Vec<WorkspacePath>,
    pub raw_provider_events: Vec<RawProviderEvent>,
}

impl AgentSession {
    #[must_use]
    pub fn succeeded(session_id: AgentSessionId) -> Self {
        Self {
            session_id,
            status: AgentStatus::Succeeded,
            transcript: AgentTranscript::default(),
            commands: Vec::new(),
            output_files: Vec::new(),
            raw_provider_events: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentStatus {
    Succeeded,
    Failed { reason: String },
    Cancelled,
    TimedOut,
    OutputContractViolation { message: String },
}

pub fn validate_output_contract(
    workspace: &WorkspaceView<'_>,
    contract: &OutputContract,
    session: &AgentSession,
) -> Result<Vec<WorkspacePath>, AgentRuntimeError> {
    match contract {
        OutputContract::Files { paths } => {
            for path in paths {
                workspace.read_file(path).map_err(|source| {
                    AgentRuntimeError::with_source(
                        format!("required output file `{}` was not readable", path.as_str()),
                        source,
                    )
                })?;
            }
            Ok(paths.clone())
        }
        OutputContract::JsonFile { path, schema: _ } => {
            let bytes = workspace.read_file(path).map_err(|source| {
                AgentRuntimeError::with_source(
                    format!("required JSON output `{}` was not readable", path.as_str()),
                    source,
                )
            })?;
            serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|source| {
                AgentRuntimeError::with_source(
                    format!(
                        "required JSON output `{}` was not valid JSON",
                        path.as_str()
                    ),
                    source,
                )
            })?;
            Ok(vec![path.clone()])
        }
        OutputContract::FinalMessage => {
            let has_assistant_message = session.transcript.events.iter().any(|event| {
                matches!(
                    event,
                    TranscriptEvent::Message {
                        role: TranscriptRole::Assistant,
                        ..
                    }
                )
            });
            if has_assistant_message {
                Ok(Vec::new())
            } else {
                Err(AgentRuntimeError::OutputContract(
                    "final assistant message was required".to_owned(),
                ))
            }
        }
        OutputContract::WorkspaceDiff { roots: _ } => Ok(Vec::new()),
    }
}
