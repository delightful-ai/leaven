use std::collections::BTreeMap;

use leaven_agent::AgentRuntimeCapabilities;
use leaven_kernel::{AgentRuntimeId, Cost, Fingerprint, FingerprintBuilder};
use leaven_workspace::{Command, CommandLimits, CommandStdin, CommandUser, WorkspacePath};

#[derive(Clone, Debug, PartialEq)]
pub struct CommandAgentConfig {
    pub id: AgentRuntimeId,
    pub fingerprint_seed: String,
    pub setup: Vec<CommandTemplate>,
    pub run: CommandTemplate,
    pub layout: CommandSessionLayout,
    pub retain_raw_stdout: bool,
    pub retain_raw_stderr: bool,
    pub cost: Cost,
}

impl CommandAgentConfig {
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        let mut builder = FingerprintBuilder::new();
        builder.update("leaven-agent-command/v1");
        builder.update(format!("{self:?}"));
        builder.finish()
    }

    #[must_use]
    pub fn capabilities(&self) -> AgentRuntimeCapabilities {
        AgentRuntimeCapabilities {
            workspace_access: leaven_agent::WorkspaceAccessMode::BackendNeutral,
            supports_commands: true,
            supports_raw_provider_events: self.retain_raw_stdout || self.retain_raw_stderr,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandTemplate {
    pub program: String,
    pub args: Vec<CommandTemplateArg>,
    pub cwd: Option<WorkspacePath>,
    pub env: BTreeMap<String, String>,
    pub stdin: CommandPromptMode,
    pub limits: CommandLimits,
    pub user: Option<CommandUser>,
}

impl CommandTemplate {
    #[must_use]
    pub fn render(&self, request: &leaven_agent::AgentRunRequest) -> Command {
        let mut command = Command::new(self.program.clone());
        command.args = self.args.iter().map(|arg| arg.render(request)).collect();
        command.cwd = self.cwd.clone().or_else(|| Some(request.cwd.clone()));
        command.env = self.env.clone();
        command.env.extend(request.env.clone());
        command.stdin = match &self.stdin {
            CommandPromptMode::None => CommandStdin::Empty,
            CommandPromptMode::StdinTask => {
                CommandStdin::Bytes(request.instructions.task.as_bytes().to_vec())
            }
            CommandPromptMode::StdinInstructions => {
                CommandStdin::Bytes(render_instructions(request).into_bytes())
            }
        };
        command.limits = merge_limits(&self.limits, &request.limits);
        command.user.clone_from(&self.user);
        command
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandTemplateArg {
    Literal(String),
    Task,
}

impl CommandTemplateArg {
    #[must_use]
    pub fn literal(value: impl Into<String>) -> Self {
        Self::Literal(value.into())
    }

    #[must_use]
    pub fn render(&self, request: &leaven_agent::AgentRunRequest) -> String {
        match self {
            Self::Literal(value) => value.clone(),
            Self::Task => request.instructions.task.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandPromptMode {
    None,
    StdinTask,
    StdinInstructions,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSessionLayout {
    pub artifacts_root: WorkspacePath,
}

fn render_instructions(request: &leaven_agent::AgentRunRequest) -> String {
    let mut rendered = String::new();
    if let Some(system) = &request.instructions.system {
        rendered.push_str("System:\n");
        rendered.push_str(system);
        rendered.push_str("\n\n");
    }

    rendered.push_str("Task:\n");
    rendered.push_str(&request.instructions.task);

    if !request.instructions.context.is_empty() {
        rendered.push_str("\n\nContext:\n");
        for context in &request.instructions.context {
            rendered.push_str("- ");
            rendered.push_str(&context.label);
            rendered.push_str(": ");
            rendered.push_str(context.path.as_str());
            if let Some(media_type) = &context.media_type {
                rendered.push_str(" (");
                rendered.push_str(media_type);
                rendered.push(')');
            }
            rendered.push('\n');
        }
    }

    rendered
}

fn merge_limits(template: &CommandLimits, request: &leaven_agent::AgentLimits) -> CommandLimits {
    let mut limits = template.clone();
    limits.timeout = min_option(limits.timeout, request.timeout);
    if let Some(max_output_bytes) = request.max_output_bytes {
        limits.max_stdout_bytes = min_option(limits.max_stdout_bytes, Some(max_output_bytes));
        limits.max_stderr_bytes = min_option(limits.max_stderr_bytes, Some(max_output_bytes));
        limits.max_output_file_bytes =
            min_option(limits.max_output_file_bytes, Some(max_output_bytes));
    }
    limits
}

fn min_option<T: Ord + Copy>(left: Option<T>, right: Option<T>) -> Option<T> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

impl Default for CommandSessionLayout {
    fn default() -> Self {
        Self {
            artifacts_root: WorkspacePath::new(".leaven/agent-command")
                .expect("default command session layout is a valid workspace path"),
        }
    }
}
