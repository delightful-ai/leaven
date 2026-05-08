use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, AgentRuntimeError,
    AgentSession, AgentStatus, CancellationRef, CommandRecord, OutputContract, TranscriptEvent,
    TranscriptRole,
};
use leaven_agent_command::{
    CommandAgentConfig, CommandAgentError, CommandAgentRuntime, CommandPromptMode,
    CommandSessionLayout, CommandSessionParser, CommandTemplate, CommandTemplateArg,
    StdoutSessionParser,
};
use leaven_kernel::{AgentRuntimeId, AgentSessionId, BudgetSnapshot, Cost};
use leaven_workspace::{
    CommandLimits, CommandStdin, WorkspaceConfig, WorkspaceFactory, WorkspacePath, WorkspaceView,
};
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

#[test]
fn command_runtime_can_pass_rendered_instructions_to_stdin() {
    futures::executor::block_on(async {
        let parent = temp_parent("command-runtime-rendered-instructions");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let runtime = runtime_with_run(CommandTemplate {
            program: "cat".to_owned(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::new(),
            stdin: CommandPromptMode::StdinInstructions,
            limits: CommandLimits::default(),
            user: None,
        });
        let mut instructions = AgentInstructions::task("complete the task");
        instructions.system = Some("developer constraints".to_owned());
        instructions.context.push(leaven_agent::AgentContextRef {
            label: "case".to_owned(),
            path: WorkspacePath::new("task/case.json").unwrap(),
            media_type: Some("application/json".to_owned()),
        });

        let session = runtime
            .run_session(
                &mut workspace.view(),
                AgentRunRequest::new(instructions, OutputContract::FinalMessage),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();
        let stdout = String::from_utf8(session.value.commands[0].output.stdout.bytes.clone())
            .expect("rendered instructions are utf8");

        assert!(stdout.contains("System:\ndeveloper constraints"));
        assert!(stdout.contains("Task:\ncomplete the task"));
        assert!(stdout.contains("- case: task/case.json (application/json)"));

        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn command_runtime_backfills_output_files_and_can_drop_raw_events() {
    futures::executor::block_on(async {
        let parent = temp_parent("command-runtime-files");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut runtime = runtime_with_run(CommandTemplate {
            program: "sh".to_owned(),
            args: vec![
                CommandTemplateArg::literal("-c"),
                CommandTemplateArg::literal(
                    "mkdir -p output && printf file > output/result.txt && printf done",
                ),
            ],
            cwd: None,
            env: BTreeMap::new(),
            stdin: CommandPromptMode::None,
            limits: CommandLimits::default(),
            user: None,
        });
        runtime.config.retain_raw_stdout = false;
        runtime.config.retain_raw_stderr = false;
        let output_path = WorkspacePath::new("output/result.txt").unwrap();

        let session = runtime
            .run_session(
                &mut workspace.view(),
                AgentRunRequest::new(
                    AgentInstructions::task("write file"),
                    OutputContract::Files {
                        paths: vec![output_path.clone()],
                    },
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();

        assert_eq!(session.value.output_files, vec![output_path]);
        assert!(session.value.raw_provider_events.is_empty());

        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn command_runtime_exposes_identity_capabilities_fingerprint_and_respects_cancellation() {
    futures::executor::block_on(async {
        let parent = temp_parent("command-runtime-cancel");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut runtime = runtime_with_run(CommandTemplate {
            program: "sh".to_owned(),
            args: vec![
                CommandTemplateArg::literal("-c"),
                CommandTemplateArg::literal("printf should-not-run"),
            ],
            cwd: None,
            env: BTreeMap::new(),
            stdin: CommandPromptMode::None,
            limits: CommandLimits::default(),
            user: None,
        });
        runtime.config.retain_raw_stdout = false;
        runtime.config.retain_raw_stderr = false;

        let cancelled = AtomicBool::new(true);
        let session = runtime
            .run_session(
                &mut workspace.view(),
                AgentRunRequest::new(
                    AgentInstructions::task("cancel"),
                    OutputContract::WorkspaceDiff { roots: vec![] },
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default())
                    .with_cancellation(CancellationRef::from_flag(&cancelled)),
            )
            .await
            .unwrap();

        assert_eq!(runtime.id(), AgentRuntimeId::new_const("command-test"));
        assert_ne!(format!("{:?}", runtime.fingerprint()), "");
        let capabilities = runtime.capabilities();
        assert!(capabilities.supports_commands);
        assert!(!capabilities.supports_raw_provider_events);
        assert_eq!(session.value.status, AgentStatus::Cancelled);
        assert!(session.value.commands.is_empty());
        assert!(session.value.raw_provider_events.is_empty());

        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn command_template_renders_task_arg_and_request_limits_without_overwriting_template_env() {
    let template = CommandTemplate {
        program: "agent".to_owned(),
        args: vec![
            CommandTemplateArg::literal("--task"),
            CommandTemplateArg::Task,
        ],
        cwd: Some(WorkspacePath::new("agent-root").unwrap()),
        env: BTreeMap::from([
            ("LEAVEN_TEMPLATE".to_owned(), "template".to_owned()),
            ("LEAVEN_SHARED".to_owned(), "template".to_owned()),
        ]),
        stdin: CommandPromptMode::StdinTask,
        limits: CommandLimits {
            timeout: Some(Duration::from_secs(10)),
            max_stdout_bytes: Some(128),
            max_stderr_bytes: None,
        },
        user: None,
    };
    let mut request = AgentRunRequest::new(
        AgentInstructions::task("rendered task"),
        OutputContract::FinalMessage,
    );
    request
        .env
        .insert("LEAVEN_REQUEST".to_owned(), "request".to_owned());
    request
        .env
        .insert("LEAVEN_SHARED".to_owned(), "request".to_owned());
    request.limits.timeout = Some(Duration::from_secs(3));
    request.limits.max_output_bytes = Some(64);

    let command = template.render(&request);

    assert_eq!(command.args, vec!["--task", "rendered task"]);
    assert_eq!(command.cwd, Some(WorkspacePath::new("agent-root").unwrap()));
    assert_eq!(command.env["LEAVEN_TEMPLATE"], "template");
    assert_eq!(command.env["LEAVEN_REQUEST"], "request");
    assert_eq!(command.env["LEAVEN_SHARED"], "request");
    assert_eq!(
        command.stdin,
        CommandStdin::Bytes(b"rendered task".to_vec())
    );
    assert_eq!(command.limits.timeout, Some(Duration::from_secs(3)));
    assert_eq!(command.limits.max_stdout_bytes, Some(64));
    assert_eq!(command.limits.max_stderr_bytes, Some(64));
}

#[test]
fn stdout_session_parser_records_nonzero_exit_as_failed_session() {
    futures::executor::block_on(async {
        let parent = temp_parent("command-runtime-nonzero");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let runtime = runtime_with_run(CommandTemplate {
            program: "sh".to_owned(),
            args: vec![
                CommandTemplateArg::literal("-c"),
                CommandTemplateArg::literal("exit 7"),
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
                    AgentInstructions::task("fail"),
                    OutputContract::WorkspaceDiff { roots: vec![] },
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();

        assert!(matches!(
            session.value.status,
            AgentStatus::Failed { ref reason } if reason.contains("exited with Some(7)")
        ));
        assert!(!session.value.transcript.events.iter().any(|event| {
            matches!(
                event,
                TranscriptEvent::Message {
                    role: TranscriptRole::Assistant,
                    ..
                }
            )
        }));

        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn command_runtime_surfaces_parser_and_workspace_failures_as_runtime_errors() {
    futures::executor::block_on(async {
        let parent = temp_parent("command-runtime-errors");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let runtime = CommandAgentRuntime::new(
            CommandAgentConfig {
                id: AgentRuntimeId::new_const("command-test"),
                fingerprint_seed: "command-test-v1".to_owned(),
                setup: Vec::new(),
                run: CommandTemplate {
                    program: "sh".to_owned(),
                    args: vec![
                        CommandTemplateArg::literal("-c"),
                        CommandTemplateArg::literal("printf ok"),
                    ],
                    cwd: None,
                    env: BTreeMap::new(),
                    stdin: CommandPromptMode::None,
                    limits: CommandLimits::default(),
                    user: None,
                },
                layout: CommandSessionLayout::default(),
                retain_raw_stdout: false,
                retain_raw_stderr: false,
                cost: Cost::zero(),
            },
            AlwaysParseError,
        );

        let error = runtime
            .run_session(
                &mut workspace.view(),
                AgentRunRequest::new(
                    AgentInstructions::task("parse"),
                    OutputContract::WorkspaceDiff { roots: vec![] },
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("parser failed"));

        let workspace_error = AgentRuntimeError::from(CommandAgentError::Workspace(
            leaven_workspace::WorkspaceError::Command("boom".to_owned()),
        ));
        assert!(workspace_error.to_string().contains("boom"));
        let parse_error = AgentRuntimeError::from(CommandAgentError::Parse("bad json".to_owned()));
        assert!(parse_error.to_string().contains("bad json"));

        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[derive(Clone, Copy, Debug)]
struct AlwaysParseError;

impl CommandSessionParser for AlwaysParseError {
    fn parse_session(
        &self,
        _session_id: AgentSessionId,
        _request: &AgentRunRequest,
        _setup_records: &[CommandRecord],
        _run_record: &CommandRecord,
        _workspace: &mut WorkspaceView<'_>,
    ) -> Result<AgentSession, CommandAgentError> {
        Err(CommandAgentError::Parse("parser failed".to_owned()))
    }
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
