use leaven_agent::{AgentRunRequest, AgentSession, AgentStatus, CommandRecord, TranscriptRole};
use leaven_kernel::AgentSessionId;
use leaven_workspace::WorkspaceView;

use crate::CommandAgentError;

pub trait CommandSessionParser: Send + Sync {
    fn parse_session(
        &self,
        session_id: AgentSessionId,
        request: &AgentRunRequest,
        setup_records: &[CommandRecord],
        run_record: &CommandRecord,
        workspace: &mut WorkspaceView<'_>,
    ) -> Result<AgentSession, CommandAgentError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StdoutSessionParser;

impl CommandSessionParser for StdoutSessionParser {
    fn parse_session(
        &self,
        session_id: AgentSessionId,
        request: &AgentRunRequest,
        setup_records: &[CommandRecord],
        run_record: &CommandRecord,
        _workspace: &mut WorkspaceView<'_>,
    ) -> Result<AgentSession, CommandAgentError> {
        let mut session = AgentSession::succeeded(session_id);
        if let Some(system) = &request.instructions.system {
            session
                .transcript
                .push_message(TranscriptRole::System, system.clone());
        }
        session
            .transcript
            .push_message(TranscriptRole::User, request.instructions.task.clone());

        session.commands.extend_from_slice(setup_records);
        session.commands.push(run_record.clone());

        if !run_record.output.stdout.bytes.is_empty() {
            session.transcript.push_message(
                TranscriptRole::Assistant,
                String::from_utf8_lossy(&run_record.output.stdout.bytes).into_owned(),
            );
        }

        if run_record.output.status.code != Some(0) {
            session.status = AgentStatus::Failed {
                reason: format!(
                    "command `{}` exited with {:?}",
                    run_record.command.program, run_record.output.status.code
                ),
            };
        }

        Ok(session)
    }
}
