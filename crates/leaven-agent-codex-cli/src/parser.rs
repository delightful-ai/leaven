use leaven_agent::{
    AgentRunRequest, AgentSession, AgentSessionArtifact, AgentSessionArtifactKind, AgentStatus,
    CommandRecord, TranscriptRole,
};
use leaven_agent_command::{CommandAgentError, CommandSessionParser};
use leaven_kernel::AgentSessionId;
use leaven_workspace::{WorkspacePath, WorkspaceView};

#[derive(Clone, Debug)]
pub struct CodexCliSessionParser {
    pub last_message_path: WorkspacePath,
}

impl CommandSessionParser for CodexCliSessionParser {
    fn parse_session(
        &self,
        session_id: AgentSessionId,
        request: &AgentRunRequest,
        setup_records: &[CommandRecord],
        run_record: &CommandRecord,
        workspace: &mut WorkspaceView<'_>,
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

        if let Some(final_message) = read_final_message(workspace, &self.last_message_path) {
            session
                .transcript
                .push_message(TranscriptRole::Assistant, final_message);
            session.artifact_files.push(AgentSessionArtifact {
                kind: AgentSessionArtifactKind::NormalizedTrajectory,
                path: self.last_message_path.clone(),
                media_type: Some("text/plain".to_owned()),
            });
        } else if !run_record.output.stdout.bytes.is_empty() {
            session.transcript.push_message(
                TranscriptRole::Assistant,
                String::from_utf8_lossy(&run_record.output.stdout.bytes).into_owned(),
            );
        }

        if run_record.output.status.code != Some(0) {
            session.status = AgentStatus::Failed {
                reason: format!(
                    "codex cli exited with {:?}: {}",
                    run_record.output.status.code,
                    String::from_utf8_lossy(&run_record.output.stderr.bytes)
                ),
            };
        }

        Ok(session)
    }
}

fn read_final_message(workspace: &WorkspaceView<'_>, path: &WorkspacePath) -> Option<String> {
    let bytes = workspace.read_file(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}
