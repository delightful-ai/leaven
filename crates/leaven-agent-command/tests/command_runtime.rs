use std::collections::BTreeMap;

use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, OutputContract,
    TranscriptEvent, TranscriptRole,
};
use leaven_agent_command::{
    CommandAgentConfig, CommandAgentRuntime, CommandPromptMode, CommandSessionLayout,
    CommandTemplate, CommandTemplateArg, StdoutSessionParser,
};
use leaven_kernel::{AgentRuntimeId, AgentSessionId, BudgetSnapshot, Cost};
use leaven_workspace::{CommandLimits, WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn command_runtime_runs_setup_before_prompt_command_and_records_commands() {
    futures::executor::block_on(async {
        let parent = temp_parent("command-runtime-setup");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut runtime = runtime_with_run(CommandTemplate {
            program: "sh".to_owned(),
            args: vec![
                CommandTemplateArg::literal("-c"),
                CommandTemplateArg::literal("test -f provider/config.txt && cat"),
            ],
            cwd: None,
            env: BTreeMap::new(),
            stdin: CommandPromptMode::StdinTask,
            limits: CommandLimits::default(),
            user: None,
        });
        runtime.config.setup.push(CommandTemplate {
            program: "sh".to_owned(),
            args: vec![
                CommandTemplateArg::literal("-c"),
                CommandTemplateArg::literal(
                    "mkdir -p provider && printf ready > provider/config.txt",
                ),
            ],
            cwd: None,
            env: BTreeMap::new(),
            stdin: CommandPromptMode::None,
            limits: CommandLimits::default(),
            user: None,
        });

        let session = runtime
            .run_session(
                &mut workspace.view(),
                AgentRunRequest::new(
                    AgentInstructions::task("hello from task"),
                    OutputContract::FinalMessage,
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();

        assert_eq!(session.cost, Cost::zero());
        assert_eq!(session.value.commands.len(), 2);
        assert_eq!(
            session.value.commands[1].output.stdout.bytes,
            b"hello from task"
        );
        assert!(session.value.transcript.events.iter().any(|event| {
            matches!(
                event,
                TranscriptEvent::Message {
                    role: TranscriptRole::Assistant,
                    content,
                } if content == "hello from task"
            )
        }));

        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn command_runtime_validates_output_contract_after_parser_success() {
    futures::executor::block_on(async {
        let parent = temp_parent("command-runtime-contract");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let runtime = runtime_with_run(CommandTemplate {
            program: "sh".to_owned(),
            args: vec![
                CommandTemplateArg::literal("-c"),
                CommandTemplateArg::literal("true"),
            ],
            cwd: None,
            env: BTreeMap::new(),
            stdin: CommandPromptMode::None,
            limits: CommandLimits::default(),
            user: None,
        });

        let error = runtime
            .run_session(
                &mut workspace.view(),
                AgentRunRequest::new(
                    AgentInstructions::task("write missing output"),
                    OutputContract::JsonFile {
                        path: WorkspacePath::new("output/proposal.json").unwrap(),
                        schema: None,
                    },
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("required JSON output"));

        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn command_runtime_passes_request_env_and_template_args() {
    futures::executor::block_on(async {
        let parent = temp_parent("command-runtime-env");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let runtime = runtime_with_run(CommandTemplate {
            program: "sh".to_owned(),
            args: vec![
                CommandTemplateArg::literal("-c"),
                CommandTemplateArg::literal(
                    "printf '%s:%s' \"$LEAVEN_TEMPLATE\" \"$LEAVEN_REQUEST\"",
                ),
            ],
            cwd: None,
            env: BTreeMap::from([("LEAVEN_TEMPLATE".to_owned(), "template".to_owned())]),
            stdin: CommandPromptMode::None,
            limits: CommandLimits::default(),
            user: None,
        });
        let mut request = AgentRunRequest::new(
            AgentInstructions::task("unused"),
            OutputContract::FinalMessage,
        );
        request
            .env
            .insert("LEAVEN_REQUEST".to_owned(), "request".to_owned());

        let session = runtime
            .run_session(
                &mut workspace.view(),
                request,
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();

        assert_eq!(
            session.value.commands[0].output.stdout.bytes,
            b"template:request"
        );

        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

fn runtime_with_run(run: CommandTemplate) -> CommandAgentRuntime<StdoutSessionParser> {
    CommandAgentRuntime::new(
        CommandAgentConfig {
            id: AgentRuntimeId::new_const("command-test"),
            fingerprint_seed: "command-test-v1".to_owned(),
            setup: Vec::new(),
            run,
            layout: CommandSessionLayout::default(),
            retain_raw_stdout: true,
            retain_raw_stderr: true,
            cost: Cost::zero(),
        },
        StdoutSessionParser,
    )
}

fn temp_parent(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("{label}-{}", uuid_like()));
    remove_dir(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn remove_dir(path: &std::path::Path) {
    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}

fn uuid_like() -> String {
    format!("{}", AgentSessionId::new())
}
