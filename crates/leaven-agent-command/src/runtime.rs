use leaven_agent::{
    AgentRunContext, AgentRunRequest, AgentRuntime, AgentRuntimeCapabilities, AgentRuntimeError,
    AgentSession, AgentStatus, CommandRecord, RawProviderEvent, validate_output_contract,
};
use leaven_kernel::{AgentRuntimeId, Fingerprint, Metered};
use leaven_workspace::WorkspaceView;

use crate::{CommandAgentConfig, CommandAgentError, CommandSessionParser, CommandTemplate};

#[derive(Clone, Debug)]
pub struct CommandAgentRuntime<Parser> {
    pub config: CommandAgentConfig,
    parser: Parser,
}

impl<Parser> CommandAgentRuntime<Parser> {
    #[must_use]
    pub fn new(config: CommandAgentConfig, parser: Parser) -> Self {
        Self { config, parser }
    }
}

impl<Parser> AgentRuntime for CommandAgentRuntime<Parser>
where
    Parser: CommandSessionParser,
{
    fn id(&self) -> AgentRuntimeId {
        self.config.id.clone()
    }

    fn fingerprint(&self) -> Fingerprint {
        self.config.fingerprint()
    }

    fn capabilities(&self) -> AgentRuntimeCapabilities {
        self.config.capabilities()
    }

    async fn run_session(
        &self,
        workspace: &mut WorkspaceView<'_>,
        request: AgentRunRequest,
        ctx: AgentRunContext<'_>,
    ) -> Result<Metered<AgentSession>, AgentRuntimeError> {
        if ctx.cancellation().is_cancelled() {
            let mut session = AgentSession::succeeded(ctx.session_id());
            session.status = AgentStatus::Cancelled;
            return Ok(Metered::new(session, self.config.cost.clone()));
        }

        let mut setup_records = Vec::with_capacity(self.config.setup.len());
        for template in &self.config.setup {
            setup_records.push(run_template(workspace, template, &request)?);
        }

        let run_record = run_template(workspace, &self.config.run, &request)?;
        let mut session = self
            .parser
            .parse_session(
                ctx.session_id(),
                &request,
                &setup_records,
                &run_record,
                workspace,
            )
            .map_err(|source| {
                AgentRuntimeError::with_source("command-backed agent parser failed", source)
            })?;

        retain_raw_events(&self.config, &mut session, &setup_records, &run_record);

        for path in validate_output_contract(workspace, &request.output_contract, &session)? {
            if !session.output_files.contains(&path) {
                session.output_files.push(path);
            }
        }

        Ok(Metered::new(session, self.config.cost.clone()))
    }
}

fn run_template(
    workspace: &mut WorkspaceView<'_>,
    template: &CommandTemplate,
    request: &AgentRunRequest,
) -> Result<CommandRecord, CommandAgentError> {
    let command = template.render(request);
    let output = workspace.run_command(command.clone())?;
    Ok(CommandRecord { command, output })
}

fn retain_raw_events(
    config: &CommandAgentConfig,
    session: &mut AgentSession,
    setup_records: &[CommandRecord],
    run_record: &CommandRecord,
) {
    if !config.retain_raw_stdout && !config.retain_raw_stderr {
        return;
    }

    for (index, record) in setup_records.iter().enumerate() {
        retain_record_raw_events(config, session, &format!("setup.{index}"), record);
    }
    retain_record_raw_events(config, session, "run", run_record);
}

fn retain_record_raw_events(
    config: &CommandAgentConfig,
    session: &mut AgentSession,
    label: &str,
    record: &CommandRecord,
) {
    if config.retain_raw_stdout {
        session.raw_provider_events.push(RawProviderEvent {
            kind: format!("command.{label}.stdout"),
            payload: String::from_utf8_lossy(&record.output.stdout.bytes).into_owned(),
        });
    }
    if config.retain_raw_stderr {
        session.raw_provider_events.push(RawProviderEvent {
            kind: format!("command.{label}.stderr"),
            payload: String::from_utf8_lossy(&record.output.stderr.bytes).into_owned(),
        });
    }
}
