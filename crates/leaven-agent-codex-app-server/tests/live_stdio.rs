#![cfg(feature = "live-codex-tests")]

use std::time::Duration;

use leaven_agent::{
    AgentInstructions, AgentLimits, AgentRunContext, AgentRunRequest, AgentRuntime, OutputContract,
};
use leaven_agent_codex_app_server::StdioCodexAppServerConnector;
use leaven_agent_codex_app_server::{CodexAppServerConfig, CodexAppServerRuntime};
use leaven_kernel::{AgentSessionId, BudgetSnapshot};
use leaven_workspace::{WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;

#[tokio::test]
#[ignore = "requires local Codex auth and LEAVEN_CODEX_LIVE=1"]
async fn stdio_app_server_writes_required_json_file() {
    if std::env::var("LEAVEN_CODEX_LIVE").as_deref() != Ok("1") {
        eprintln!("skipping live Codex test because LEAVEN_CODEX_LIVE != 1");
        return;
    }

    let runtime = CodexAppServerRuntime::new(
        CodexAppServerConfig::default(),
        StdioCodexAppServerConnector {
            codex_bin: bun_codex_bin(),
            config_overrides: Vec::new(),
        },
    );
    let mut workspace = LocalWorkspaceFactory::temp()
        .allocate(Default::default())
        .await
        .expect("allocate temp workspace");
    let mut view = workspace.view();
    let output = WorkspacePath::new("output/result.json").expect("valid output path");
    let mut request = AgentRunRequest::new(
        AgentInstructions::task(
            "Create or overwrite output/result.json with exactly this JSON object: \
             {\"ok\":true,\"message\":\"codex-live\"}. Do not modify any other files.",
        ),
        OutputContract::JsonFile {
            path: output.clone(),
            schema: None,
        },
    );
    request.limits = AgentLimits {
        timeout: Some(Duration::from_secs(180)),
        ..AgentLimits::default()
    };
    let budget = BudgetSnapshot::default();

    let session = runtime
        .run_session(
            &mut view,
            request,
            AgentRunContext::new(AgentSessionId::new(), &budget),
        )
        .await
        .expect("run Codex app-server session");

    assert!(session.value.output_files.contains(&output));
    let json = view.read_file(&output).expect("read output JSON");
    let value: serde_json::Value = serde_json::from_slice(&json).expect("parse output JSON");
    assert_eq!(value["ok"], true);
    assert_eq!(value["message"], "codex-live");
}

fn bun_codex_bin() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("LEAVEN_CODEX_BIN") {
        return path.into();
    }

    let home = std::env::var_os("HOME").expect("HOME must be set for Bun Codex live test");
    std::path::PathBuf::from(home).join(".bun/bin/codex")
}
