//! Deterministic runtime for contract tests and examples.

use leaven_kernel::{AgentRuntimeId, Cost, Fingerprint, Metered};
use leaven_workspace::{Command, WorkspacePath, WorkspaceView};

use crate::{
    AgentRunContext, AgentRunRequest, AgentRuntime, AgentRuntimeCapabilities, AgentRuntimeError,
    AgentSession, AgentStatus, CommandRecord, OutputContract, RawProviderEvent, TranscriptRole,
    WorkspaceAccessMode,
};

#[derive(Clone, Debug)]
pub struct FakeAgentRuntime {
    id: AgentRuntimeId,
    fingerprint: Fingerprint,
    capabilities: AgentRuntimeCapabilities,
    actions: Vec<FakeAgentAction>,
    cost: Cost,
}

impl FakeAgentRuntime {
    #[must_use]
    pub fn new(actions: Vec<FakeAgentAction>) -> Self {
        Self {
            id: AgentRuntimeId::new_const("fake"),
            fingerprint: Fingerprint::from_bytes([0xFA; 32]),
            capabilities: AgentRuntimeCapabilities::default(),
            actions,
            cost: Cost::zero(),
        }
    }

    #[must_use]
    pub fn requiring_local_mount(actions: Vec<FakeAgentAction>) -> Self {
        Self::new(actions).with_capabilities(AgentRuntimeCapabilities {
            workspace_access: WorkspaceAccessMode::RequiresLocalMount,
            ..AgentRuntimeCapabilities::default()
        })
    }

    #[must_use]
    pub fn with_id(mut self, id: AgentRuntimeId) -> Self {
        self.id = id;
        self
    }

    #[must_use]
    pub fn with_cost(mut self, cost: Cost) -> Self {
        self.cost = cost;
        self
    }

    #[must_use]
    pub fn with_capabilities(mut self, capabilities: AgentRuntimeCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }
}

#[derive(Clone, Debug)]
pub enum FakeAgentAction {
    AssistantMessage(String),
    WriteFile { path: WorkspacePath, bytes: Vec<u8> },
    ReadFile { path: WorkspacePath },
    RunCommand(Command),
    RawProviderEvent { kind: String, payload: String },
    Status(AgentStatus),
}

impl AgentRuntime for FakeAgentRuntime {
    fn id(&self) -> AgentRuntimeId {
        self.id.clone()
    }

    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    fn capabilities(&self) -> AgentRuntimeCapabilities {
        self.capabilities.clone()
    }

    async fn run_session(
        &self,
        workspace: &mut WorkspaceView<'_>,
        request: AgentRunRequest,
        ctx: AgentRunContext<'_>,
    ) -> Result<Metered<AgentSession>, AgentRuntimeError> {
        if self.capabilities.workspace_access == WorkspaceAccessMode::RequiresLocalMount
            && workspace.local_mount().is_none()
        {
            return Err(AgentRuntimeError::LocalMountRequired { runtime: self.id() });
        }

        let mut session = AgentSession::succeeded(ctx.session_id());
        if ctx.cancellation().is_cancelled() {
            session.status = AgentStatus::Cancelled;
            return Ok(Metered::new(session, self.cost.clone()));
        }

        if let Some(system) = request.instructions.system {
            session
                .transcript
                .push_message(TranscriptRole::System, system);
        }
        session
            .transcript
            .push_message(TranscriptRole::User, request.instructions.task);

        for action in &self.actions {
            match action {
                FakeAgentAction::AssistantMessage(message) => {
                    session
                        .transcript
                        .push_message(TranscriptRole::Assistant, message.clone());
                }
                FakeAgentAction::WriteFile { path, bytes } => {
                    workspace.write_file(path, bytes)?;
                    session.output_files.push(path.clone());
                }
                FakeAgentAction::ReadFile { path } => {
                    let bytes = workspace.read_file(path)?;
                    session.transcript.push_message(
                        TranscriptRole::Tool,
                        format!("read {} byte(s) from {}", bytes.len(), path.as_str()),
                    );
                }
                FakeAgentAction::RunCommand(command) => {
                    let output = workspace.run_command(command.clone())?;
                    session.commands.push(CommandRecord {
                        command: command.clone(),
                        output,
                    });
                }
                FakeAgentAction::RawProviderEvent { kind, payload } => {
                    session.raw_provider_events.push(RawProviderEvent {
                        kind: kind.clone(),
                        payload: payload.clone(),
                    });
                }
                FakeAgentAction::Status(status) => {
                    session.status = status.clone();
                }
            }
        }

        validate_output_contract(workspace, &request.output_contract, &session)?;

        Ok(Metered::new(session, self.cost.clone()))
    }
}

fn validate_output_contract(
    workspace: &WorkspaceView<'_>,
    contract: &OutputContract,
    session: &AgentSession,
) -> Result<(), AgentRuntimeError> {
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
            Ok(())
        }
        OutputContract::JsonFile { path, schema: _ } => {
            workspace.read_file(path).map_err(|source| {
                AgentRuntimeError::with_source(
                    format!("required JSON output `{}` was not readable", path.as_str()),
                    source,
                )
            })?;
            Ok(())
        }
        OutputContract::FinalMessage => {
            let has_assistant_message = session.transcript.events.iter().any(|event| {
                matches!(
                    event,
                    crate::TranscriptEvent::Message {
                        role: TranscriptRole::Assistant,
                        ..
                    }
                )
            });
            if has_assistant_message {
                Ok(())
            } else {
                Err(AgentRuntimeError::OutputContract(
                    "final assistant message was required".to_owned(),
                ))
            }
        }
        OutputContract::WorkspaceDiff { roots: _ } => Ok(()),
    }
}
