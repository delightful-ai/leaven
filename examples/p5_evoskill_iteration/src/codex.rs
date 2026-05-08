use std::path::PathBuf;

use leaven_agent_codex_cli::{CodexCliApproval, CodexCliConfig, CodexCliRuntime};

pub type LiveCodexRuntime = CodexCliRuntime;

pub fn live_codex_runtime(_developer_instructions: String) -> LiveCodexRuntime {
    let mut config = CodexCliConfig::new(bun_codex_bin().to_string_lossy().into_owned());
    "gpt-5.4-mini".clone_into(&mut config.model);
    config.approval = CodexCliApproval::BypassSandboxAndApprovals;
    CodexCliRuntime::new(config)
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
