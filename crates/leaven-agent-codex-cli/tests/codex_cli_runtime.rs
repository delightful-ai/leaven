use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, AgentStatus, OutputContract,
    TranscriptEvent, TranscriptRole,
};
use leaven_agent_codex_cli::{
    CodexCliApproval, CodexCliConfig, CodexCliReasoningEffort, CodexCliRuntime, CodexCliSandbox,
};
use leaven_agent_command::{CommandPromptMode, CommandTemplateArg};
use leaven_kernel::{AgentSessionId, BudgetSnapshot};
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn codex_cli_config_builds_backend_neutral_exec_template() {
    let mut config = CodexCliConfig::new("codex");
    config.model = "gpt-5.4-mini".to_owned();
    config.reasoning_effort = CodexCliReasoningEffort::Low;
    config.approval = CodexCliApproval::BypassSandboxAndApprovals;

    let command_config = config.command_config();
    let run = command_config.run;

    assert_eq!(run.program, "codex");
    assert_eq!(run.cwd, None);
    assert_eq!(run.stdin, CommandPromptMode::StdinInstructions);
    assert!(run.env.is_empty());
    assert_eq!(
        run.args,
        vec![
            CommandTemplateArg::literal("exec"),
            CommandTemplateArg::literal("--json"),
            CommandTemplateArg::literal("--skip-git-repo-check"),
            CommandTemplateArg::literal("--model"),
            CommandTemplateArg::literal("gpt-5.4-mini"),
            CommandTemplateArg::literal("--config"),
            CommandTemplateArg::literal("model_reasoning_effort=\"low\""),
            CommandTemplateArg::literal("--output-last-message"),
            CommandTemplateArg::literal(".leaven/codex-last-message.txt"),
            CommandTemplateArg::literal("--dangerously-bypass-approvals-and-sandbox"),
            CommandTemplateArg::literal("-"),
        ]
    );
}

#[test]
fn codex_cli_config_leaves_repo_skills_native() {
    let command_config = CodexCliConfig::new("codex").command_config();
    let setup_text = format!("{:?}", command_config.setup);

    assert!(!setup_text.contains(".agents/skills"));
    assert!(!setup_text.contains("cp -R"));
    assert!(setup_text.contains("mkdir"));
    assert!(setup_text.contains(": >"));
}

#[test]
fn codex_cli_config_covers_wire_variants_env_and_parser_construction() {
    assert_eq!(CodexCliReasoningEffort::Minimal.as_wire(), "minimal");
    assert_eq!(CodexCliReasoningEffort::Low.as_wire(), "low");
    assert_eq!(CodexCliReasoningEffort::Medium.as_wire(), "medium");
    assert_eq!(CodexCliReasoningEffort::High.as_wire(), "high");
    assert_eq!(CodexCliReasoningEffort::XHigh.as_wire(), "xhigh");
    assert_eq!(CodexCliSandbox::ReadOnly.as_wire(), "read-only");
    assert_eq!(CodexCliSandbox::WorkspaceWrite.as_wire(), "workspace-write");
    assert_eq!(
        CodexCliSandbox::DangerFullAccess.as_wire(),
        "danger-full-access"
    );

    let default_config = CodexCliConfig::default();
    assert_eq!(default_config.command_config().run.program, "codex");

    let mut config = CodexCliConfig::new("codex");
    config.codex_home = Some("/tmp/leaven-codex-home".to_owned());
    config.reasoning_effort = CodexCliReasoningEffort::XHigh;
    config.approval = CodexCliApproval::Sandbox(CodexCliSandbox::ReadOnly);
    let command_config = config.command_config();
    let parser = config.session_parser();

    assert_eq!(
        command_config.run.env["CODEX_HOME"],
        "/tmp/leaven-codex-home"
    );
    assert!(
        command_config
            .run
            .args
            .contains(&CommandTemplateArg::literal(
                "model_reasoning_effort=\"xhigh\""
            ))
    );
    assert!(
        command_config
            .run
            .args
            .contains(&CommandTemplateArg::literal("read-only"))
    );
    assert_eq!(
        parser.last_message_path,
        WorkspacePath::new(".leaven/codex-last-message.txt").unwrap()
    );
}

#[test]
fn codex_cli_runtime_delegates_identity_fingerprint_and_capabilities() {
    let runtime = CodexCliRuntime::new(CodexCliConfig::new("codex"));

    assert_eq!(runtime.id().as_str(), "codex-cli");
    assert_ne!(format!("{:?}", runtime.fingerprint()), "");
    let capabilities = runtime.capabilities();
    assert!(capabilities.supports_commands);
    assert!(capabilities.supports_raw_provider_events);
}

#[test]
fn codex_cli_runtime_reads_last_message_and_preserves_raw_stdout() {
    futures::executor::block_on(async {
        let parent = temp_parent("codex-cli-runtime");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut view = workspace.view();
        view.write_file(
            &WorkspacePath::new("bin/codex").unwrap(),
            br#"#!/bin/sh
out=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
    shift
    out="$1"
  fi
  shift
done
mkdir -p "$(dirname "$out")"
cat > .leaven/seen-prompt.txt
printf '{"event":"done"}\n'
printf '{"final_answer":"42"}' > "$out"
"#,
        )
        .unwrap();
        view.set_executable(&WorkspacePath::new("bin/codex").unwrap(), true)
            .unwrap();

        let mut config = CodexCliConfig::new("bin/codex");
        config.approval = CodexCliApproval::Sandbox(CodexCliSandbox::DangerFullAccess);
        let runtime = CodexCliRuntime::new(config);
        let mut instructions = AgentInstructions::task("return json");
        instructions.system = Some("developer rules".to_owned());

        let session = runtime
            .run_session(
                &mut view,
                AgentRunRequest::new(instructions, OutputContract::FinalMessage),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();
        let prompt = String::from_utf8(
            view.read_file(&WorkspacePath::new(".leaven/seen-prompt.txt").unwrap())
                .unwrap(),
        )
        .unwrap();

        assert!(prompt.contains("System:\ndeveloper rules"));
        assert!(prompt.contains("Task:\nreturn json"));
        assert!(session.value.raw_provider_events.iter().any(|event| {
            event.kind == "command.run.stdout" && event.payload.contains(r#""done""#)
        }));
        assert!(session.value.transcript.events.iter().any(|event| {
            matches!(
                event,
                TranscriptEvent::Message {
                    role: TranscriptRole::Assistant,
                    content,
                } if content == "{\"final_answer\":\"42\"}"
            )
        }));

        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn codex_cli_parser_falls_back_to_stdout_when_last_message_is_absent() {
    futures::executor::block_on(async {
        let parent = temp_parent("codex-cli-stdout-fallback");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut view = workspace.view();
        view.write_file(
            &WorkspacePath::new("bin/codex").unwrap(),
            br"#!/bin/sh
cat >/dev/null
printf 'assistant from stdout'
",
        )
        .unwrap();
        view.set_executable(&WorkspacePath::new("bin/codex").unwrap(), true)
            .unwrap();

        let runtime = CodexCliRuntime::new(CodexCliConfig::new("bin/codex"));
        let session = runtime
            .run_session(
                &mut view,
                AgentRunRequest::new(
                    AgentInstructions::task("stdout fallback"),
                    OutputContract::FinalMessage,
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();

        assert!(session.value.transcript.events.iter().any(|event| {
            matches!(
                event,
                TranscriptEvent::Message {
                    role: TranscriptRole::Assistant,
                    content,
                } if content == "assistant from stdout"
            )
        }));

        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn codex_cli_parser_reports_nonzero_exit_and_ignores_empty_last_message() {
    futures::executor::block_on(async {
        let parent = temp_parent("codex-cli-nonzero");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut view = workspace.view();
        view.write_file(
            &WorkspacePath::new("bin/codex").unwrap(),
            br#"#!/bin/sh
out=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
    shift
    out="$1"
  fi
  shift
done
mkdir -p "$(dirname "$out")"
: > "$out"
printf 'bad provider state' >&2
exit 9
"#,
        )
        .unwrap();
        view.set_executable(&WorkspacePath::new("bin/codex").unwrap(), true)
            .unwrap();

        let runtime = CodexCliRuntime::new(CodexCliConfig::new("bin/codex"));
        let session = runtime
            .run_session(
                &mut view,
                AgentRunRequest::new(
                    AgentInstructions::task("nonzero"),
                    OutputContract::WorkspaceDiff {
                        roots: vec![],
                        surface_fingerprint: None,
                    },
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();

        assert!(matches!(
            session.value.status,
            AgentStatus::Failed { ref reason }
                if reason.contains("Some(9)") && reason.contains("bad provider state")
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

        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn codex_cli_runtime_clears_stale_last_message_before_each_run() {
    futures::executor::block_on(async {
        let parent = temp_parent("codex-cli-stale-last-message");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut view = workspace.view();
        view.write_file(
            &WorkspacePath::new(".leaven/codex-last-message.txt").unwrap(),
            b"stale assistant message",
        )
        .unwrap();
        view.write_file(
            &WorkspacePath::new("bin/codex").unwrap(),
            br"#!/bin/sh
cat >/dev/null
printf 'mutation failed before final message' >&2
exit 1
",
        )
        .unwrap();
        view.set_executable(&WorkspacePath::new("bin/codex").unwrap(), true)
            .unwrap();

        let runtime = CodexCliRuntime::new(CodexCliConfig::new("bin/codex"));
        let session = runtime
            .run_session(
                &mut view,
                AgentRunRequest::new(
                    AgentInstructions::task("mutate"),
                    OutputContract::WorkspaceDiff {
                        roots: vec![],
                        surface_fingerprint: None,
                    },
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();

        assert!(matches!(
            session.value.status,
            AgentStatus::Failed { ref reason }
                if reason.contains("Some(1)")
                    && reason.contains("mutation failed before final message")
        ));
        assert!(!session.value.transcript.events.iter().any(|event| {
            matches!(
                event,
                TranscriptEvent::Message {
                    role: TranscriptRole::Assistant,
                    content,
                } if content == "stale assistant message"
            )
        }));
        assert_eq!(
            view.read_file(&WorkspacePath::new(".leaven/codex-last-message.txt").unwrap())
                .unwrap(),
            b""
        );

        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

fn temp_parent(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("{label}-{}", AgentSessionId::new()));
    remove_dir(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn remove_dir(path: &std::path::Path) {
    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}
