use std::collections::BTreeMap;
use std::time::Duration;

use leaven_agent_command::{
    CommandAgentConfig, CommandPromptMode, CommandSessionLayout, CommandTemplate,
    CommandTemplateArg,
};
use leaven_kernel::{AgentRuntimeId, Cost};
use leaven_workspace::{CommandLimits, WorkspacePath};

use crate::CodexCliSessionParser;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexCliConfig {
    pub codex_bin: String,
    pub model: String,
    pub reasoning_effort: CodexCliReasoningEffort,
    pub approval: CodexCliApproval,
    pub last_message_path: WorkspacePath,
    pub timeout: Option<Duration>,
    pub retain_raw_stdout: bool,
    pub retain_raw_stderr: bool,
    pub codex_home: Option<String>,
}

impl CodexCliConfig {
    #[must_use]
    pub fn new(codex_bin: impl Into<String>) -> Self {
        Self {
            codex_bin: codex_bin.into(),
            model: "gpt-5.4-mini".to_owned(),
            reasoning_effort: CodexCliReasoningEffort::Low,
            approval: CodexCliApproval::Sandbox(CodexCliSandbox::WorkspaceWrite),
            last_message_path: WorkspacePath::new(".leaven/codex-last-message.txt")
                .expect("default Codex last-message path is valid"),
            timeout: None,
            retain_raw_stdout: true,
            retain_raw_stderr: true,
            codex_home: None,
        }
    }

    #[must_use]
    pub fn command_config(&self) -> CommandAgentConfig {
        let mut env = BTreeMap::new();
        if let Some(codex_home) = &self.codex_home {
            env.insert("CODEX_HOME".to_owned(), codex_home.clone());
        }

        CommandAgentConfig {
            id: AgentRuntimeId::new_const("codex-cli"),
            fingerprint_seed: "codex-cli-v1".to_owned(),
            setup: vec![mkdir_leaven_command()],
            run: CommandTemplate {
                program: self.codex_bin.clone(),
                args: self.exec_args(),
                cwd: None,
                env,
                stdin: CommandPromptMode::StdinInstructions,
                limits: CommandLimits {
                    timeout: self.timeout,
                    max_stdout_bytes: Some(4 * 1024 * 1024),
                    max_stderr_bytes: Some(4 * 1024 * 1024),
                    max_output_file_bytes: None,
                },
                user: None,
            },
            layout: CommandSessionLayout::default(),
            retain_raw_stdout: self.retain_raw_stdout,
            retain_raw_stderr: self.retain_raw_stderr,
            cost: Cost::zero(),
        }
    }

    #[must_use]
    pub fn session_parser(&self) -> CodexCliSessionParser {
        CodexCliSessionParser {
            last_message_path: self.last_message_path.clone(),
        }
    }

    #[must_use]
    pub(crate) fn into_runtime_parts(self) -> (CommandAgentConfig, CodexCliSessionParser) {
        let Self {
            codex_bin,
            model,
            reasoning_effort,
            approval,
            last_message_path,
            timeout,
            retain_raw_stdout,
            retain_raw_stderr,
            codex_home,
        } = self;

        let mut env = BTreeMap::new();
        if let Some(codex_home) = codex_home {
            env.insert("CODEX_HOME".to_owned(), codex_home);
        }

        let command_config = CommandAgentConfig {
            id: AgentRuntimeId::new_const("codex-cli"),
            fingerprint_seed: "codex-cli-v1".to_owned(),
            setup: vec![mkdir_leaven_command()],
            run: CommandTemplate {
                program: codex_bin,
                args: codex_exec_args(&model, reasoning_effort, approval, &last_message_path),
                cwd: None,
                env,
                stdin: CommandPromptMode::StdinInstructions,
                limits: CommandLimits {
                    timeout,
                    max_stdout_bytes: Some(4 * 1024 * 1024),
                    max_stderr_bytes: Some(4 * 1024 * 1024),
                    max_output_file_bytes: None,
                },
                user: None,
            },
            layout: CommandSessionLayout::default(),
            retain_raw_stdout,
            retain_raw_stderr,
            cost: Cost::zero(),
        };
        let parser = CodexCliSessionParser { last_message_path };
        (command_config, parser)
    }

    fn exec_args(&self) -> Vec<CommandTemplateArg> {
        codex_exec_args(
            &self.model,
            self.reasoning_effort,
            self.approval,
            &self.last_message_path,
        )
    }
}

fn codex_exec_args(
    model: &str,
    reasoning_effort: CodexCliReasoningEffort,
    approval: CodexCliApproval,
    last_message_path: &WorkspacePath,
) -> Vec<CommandTemplateArg> {
    let mut args = vec![
        CommandTemplateArg::literal("exec"),
        CommandTemplateArg::literal("--json"),
        CommandTemplateArg::literal("--skip-git-repo-check"),
        CommandTemplateArg::literal("--model"),
        CommandTemplateArg::literal(model),
        CommandTemplateArg::literal("--config"),
        CommandTemplateArg::literal(format!(
            "model_reasoning_effort=\"{}\"",
            reasoning_effort.as_wire()
        )),
        CommandTemplateArg::literal("--output-last-message"),
        CommandTemplateArg::literal(last_message_path.as_str()),
    ];
    approval.push_args(&mut args);
    args.push(CommandTemplateArg::literal("-"));
    args
}

impl Default for CodexCliConfig {
    fn default() -> Self {
        Self::new("codex")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexCliReasoningEffort {
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

impl CodexCliReasoningEffort {
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexCliApproval {
    Sandbox(CodexCliSandbox),
    BypassSandboxAndApprovals,
}

impl CodexCliApproval {
    fn push_args(self, args: &mut Vec<CommandTemplateArg>) {
        match self {
            Self::Sandbox(sandbox) => {
                args.push(CommandTemplateArg::literal("--sandbox"));
                args.push(CommandTemplateArg::literal(sandbox.as_wire()));
            }
            Self::BypassSandboxAndApprovals => {
                args.push(CommandTemplateArg::literal(
                    "--dangerously-bypass-approvals-and-sandbox",
                ));
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexCliSandbox {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl CodexCliSandbox {
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
        }
    }
}

fn mkdir_leaven_command() -> CommandTemplate {
    CommandTemplate {
        program: "sh".to_owned(),
        args: vec![
            CommandTemplateArg::literal("-c"),
            CommandTemplateArg::literal("mkdir -p .leaven"),
        ],
        cwd: None,
        env: BTreeMap::new(),
        stdin: CommandPromptMode::None,
        limits: CommandLimits::default(),
        user: None,
    }
}
