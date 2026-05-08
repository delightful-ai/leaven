use leaven_agent::{
    AgentRunContext, AgentRunRequest, AgentRuntime, AgentRuntimeCapabilities, AgentRuntimeError,
    AgentSession,
};
use leaven_agent_command::CommandAgentRuntime;
use leaven_kernel::{AgentRuntimeId, Fingerprint, Metered};
use leaven_workspace::WorkspaceView;

use crate::{CodexCliConfig, CodexCliSessionParser};

#[derive(Clone, Debug)]
pub struct CodexCliRuntime {
    inner: CommandAgentRuntime<CodexCliSessionParser>,
}

impl CodexCliRuntime {
    #[must_use]
    pub fn new(config: CodexCliConfig) -> Self {
        let command_config = config.command_config();
        let parser = CodexCliSessionParser {
            last_message_path: config.last_message_path,
        };
        Self {
            inner: CommandAgentRuntime::new(command_config, parser),
        }
    }
}

impl AgentRuntime for CodexCliRuntime {
    fn id(&self) -> AgentRuntimeId {
        self.inner.id()
    }

    fn fingerprint(&self) -> Fingerprint {
        self.inner.fingerprint()
    }

    fn capabilities(&self) -> AgentRuntimeCapabilities {
        self.inner.capabilities()
    }

    async fn run_session(
        &self,
        workspace: &mut WorkspaceView<'_>,
        request: AgentRunRequest,
        ctx: AgentRunContext<'_>,
    ) -> Result<Metered<AgentSession>, AgentRuntimeError> {
        self.inner.run_session(workspace, request, ctx).await
    }
}
