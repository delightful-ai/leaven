use std::path::{Path, PathBuf};

use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, AgentStatus, OutputContract,
    TranscriptEvent, TranscriptRole,
};
use leaven_agent_codex_cli::{
    CodexCliConfig, CodexCliGoalMode, CodexCliReasoningEffort, CodexCliRuntime,
};
use leaven_kernel::{AgentSessionId, BudgetSnapshot, RunId};
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
#[ignore = "requires local Codex auth and LEAVEN_CODEX_LIVE=1"]
fn live_codex_goal_mode_returns_session_data() {
    if std::env::var("LEAVEN_CODEX_LIVE").as_deref() != Ok("1") {
        eprintln!("skipping live Codex test because LEAVEN_CODEX_LIVE != 1");
        return;
    }

    futures::executor::block_on(async {
        let parent = temp_parent("codex-cli-live-goal-mode");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut view = workspace.view();

        let mut config = CodexCliConfig::new(
            std::env::var("LEAVEN_CODEX_BIN").unwrap_or_else(|_| "codex".to_owned()),
        );
        config.model = "gpt-5.4-mini".to_owned();
        config.reasoning_effort = CodexCliReasoningEffort::Low;
        config.goal_mode = CodexCliGoalMode::Enabled;

        let runtime = CodexCliRuntime::new(config.clone());
        let session = runtime
            .run_session(
                &mut view,
                AgentRunRequest::new(
                    AgentInstructions::task(
                        "Return exactly this JSON object and do not edit files: \
                         {\"leaven_live_goal_mode\":\"ok\",\"model\":\"gpt-5.4-mini\",\"reasoning\":\"low\"}",
                    ),
                    OutputContract::FinalMessage,
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();

        let assistant_text = session
            .value
            .transcript
            .events
            .iter()
            .find_map(|event| match event {
                TranscriptEvent::Message {
                    role: TranscriptRole::Assistant,
                    content,
                } => Some(content.as_str()),
                _ => None,
            })
            .unwrap_or("");

        println!("status={:?}", session.value.status);
        println!("assistant={assistant_text}");
        println!("commands={}", session.value.commands.len());
        println!(
            "raw_provider_events={}",
            session.value.raw_provider_events.len()
        );
        println!(
            "raw_event_kinds={:?}",
            session
                .value
                .raw_provider_events
                .iter()
                .map(|event| event.kind.as_str())
                .collect::<Vec<_>>()
        );

        let command = session
            .value
            .commands
            .iter()
            .find(|record| record.command.args.iter().any(|arg| arg == "exec"))
            .expect("Codex run command is recorded")
            .command
            .clone();
        assert!(has_arg_pair(&command.args, "--enable", "goals"));
        assert!(has_arg_pair(&command.args, "--model", "gpt-5.4-mini"));
        assert!(has_arg_pair(
            &command.args,
            "--config",
            "model_reasoning_effort=\"low\""
        ));
        assert_eq!(session.value.status, AgentStatus::Succeeded);
        assert!(assistant_text.contains("\"leaven_live_goal_mode\":\"ok\""));
        assert!(
            session
                .value
                .raw_provider_events
                .iter()
                .any(|event| event.kind == "command.run.stdout")
        );

        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

fn has_arg_pair(args: &[String], left: &str, right: &str) -> bool {
    args.windows(2).any(|pair| pair == [left, right])
}

fn temp_parent(label: &str) -> PathBuf {
    let parent = std::env::temp_dir().join(format!("leaven-{label}-{}", RunId::new()));
    remove_dir(&parent);
    std::fs::create_dir_all(&parent).unwrap();
    parent
}

fn remove_dir(path: &Path) {
    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}
