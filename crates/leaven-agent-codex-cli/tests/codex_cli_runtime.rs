use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, OutputContract,
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
