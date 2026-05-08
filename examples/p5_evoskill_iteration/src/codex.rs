use std::path::PathBuf;

use leaven_agent_codex_app_server::{
    CodexAppServerConfig, CodexAppServerRuntime, CodexReasoningEffort, CodexSandboxMode,
    StdioCodexAppServerConnector,
};

pub type LiveCodexRuntime = CodexAppServerRuntime<StdioCodexAppServerConnector>;

pub fn live_codex_runtime(developer_instructions: String) -> LiveCodexRuntime {
    let mut config = CodexAppServerConfig::default();
    config.thread.model = Some("gpt-5.4-mini".to_owned());
    config.thread.sandbox = Some(CodexSandboxMode::DangerFullAccess);
    config.thread.developer_instructions = Some(developer_instructions);
    config.turn.effort = Some(CodexReasoningEffort::Low);
    CodexAppServerRuntime::new(
        config,
        StdioCodexAppServerConnector {
            codex_bin: bun_codex_bin(),
            config_overrides: vec!["features.unified_exec=false".to_owned()],
        },
    )
}

pub fn require_live_codex() -> crate::error::Result<()> {
    if std::env::var("LEAVEN_CODEX_LIVE").as_deref() == Ok("1") {
        Ok(())
    } else {
        Err(crate::error::msg(
            "p5 requires LEAVEN_CODEX_LIVE=1 because Codex execution is the gate",
        ))
    }
}

fn bun_codex_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("LEAVEN_CODEX_BIN") {
        return path.into();
    }
    let home = std::env::var_os("HOME").expect("HOME must be set for Bun Codex");
    PathBuf::from(home).join(".bun/bin/codex")
}
