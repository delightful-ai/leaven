//! Codex app-server JSON-RPC transports.

#![cfg(feature = "app-server")]

#[cfg(feature = "stdio")]
use std::collections::BTreeMap;
#[cfg(feature = "stdio")]
use std::ffi::OsString;
use std::future::Future;
#[cfg(feature = "stdio")]
use std::path::Path;
use std::path::PathBuf;
#[cfg(feature = "stdio")]
use std::process::Stdio;

use async_trait::async_trait;
use leaven_agent::{AgentRunRequest, WorkspaceAccessMode};
use leaven_kernel::FingerprintBuilder;
use leaven_workspace::WorkspaceView;
#[cfg(feature = "stdio")]
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
#[cfg(feature = "stdio")]
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

#[cfg(any(feature = "stdio", test))]
use crate::error::CodexAppServerError;
use crate::error::Result;

#[async_trait]
pub trait CodexAppServerTransport: Send {
    async fn write_payload(&mut self, payload: &str) -> Result<()>;
    async fn read_payload(&mut self) -> Result<String>;

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct CodexAppServerConnection<T> {
    pub transport: T,
    pub cwd: PathBuf,
}

pub trait CodexAppServerConnector: Send + Sync {
    type Transport: CodexAppServerTransport;

    fn workspace_access(&self) -> WorkspaceAccessMode;

    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder);

    fn connect<'a>(
        &'a self,
        workspace: &'a WorkspaceView<'_>,
        request: &'a AgentRunRequest,
    ) -> impl Future<Output = Result<CodexAppServerConnection<Self::Transport>>> + Send + 'a;
}

#[cfg(feature = "stdio")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdioCodexAppServerConnector {
    pub codex_bin: PathBuf,
    pub config_overrides: Vec<String>,
}

#[cfg(feature = "stdio")]
impl Default for StdioCodexAppServerConnector {
    fn default() -> Self {
        Self {
            codex_bin: PathBuf::from("codex"),
            config_overrides: Vec::new(),
        }
    }
}

#[cfg(feature = "stdio")]
impl CodexAppServerConnector for StdioCodexAppServerConnector {
    type Transport = StdioCodexAppServerTransport;

    fn workspace_access(&self) -> WorkspaceAccessMode {
        WorkspaceAccessMode::RequiresLocalMount
    }

    fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
        builder.update("connector=stdio");
        builder.update(self.codex_bin.to_string_lossy().as_bytes());
        for config_override in &self.config_overrides {
            builder.update("\0");
            builder.update(config_override.as_bytes());
        }
    }

    async fn connect(
        &self,
        workspace: &WorkspaceView<'_>,
        request: &AgentRunRequest,
    ) -> Result<CodexAppServerConnection<Self::Transport>> {
        let local_mount = workspace
            .local_mount()
            .ok_or_else(|| CodexAppServerError::Protocol("local mount required".to_owned()))?;
        let cwd = local_mount.join(request.cwd.to_host_relative());
        let transport = StdioCodexAppServerTransport::spawn(
            &self.codex_bin,
            &self.config_overrides,
            &cwd,
            &request.env,
        )?;
        Ok(CodexAppServerConnection { transport, cwd })
    }
}

#[cfg(feature = "stdio")]
pub struct StdioCodexAppServerTransport {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
}

#[cfg(feature = "stdio")]
impl StdioCodexAppServerTransport {
    fn spawn(
        codex_bin: impl AsRef<Path>,
        config_overrides: &[String],
        cwd: &Path,
        env: &BTreeMap<String, String>,
    ) -> Result<Self> {
        let codex_bin = codex_bin.as_ref();
        let mut command = Command::new(codex_bin);

        if let Some(codex_bin_parent) = codex_bin.parent()
            && !codex_bin_parent.as_os_str().is_empty()
        {
            let mut path = OsString::from(codex_bin_parent.as_os_str());
            if let Some(existing_path) = std::env::var_os("PATH") {
                path.push(":");
                path.push(existing_path);
            }
            command.env("PATH", path);
        }

        command.current_dir(cwd);
        command.envs(env);

        for config_override in config_overrides {
            command.arg("--config").arg(config_override);
        }

        let mut child = command
            .arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| {
            CodexAppServerError::Protocol("codex app-server child missing stdin".to_owned())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            CodexAppServerError::Protocol("codex app-server child missing stdout".to_owned())
        })?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
        })
    }
}

#[cfg(feature = "stdio")]
#[async_trait]
impl CodexAppServerTransport for StdioCodexAppServerTransport {
    async fn write_payload(&mut self, payload: &str) -> Result<()> {
        self.stdin.write_all(payload.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn read_payload(&mut self) -> Result<String> {
        match self.stdout.next_line().await? {
            Some(line) => Ok(line),
            None => Err(CodexAppServerError::ConnectionClosed),
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        if self.child.try_wait()?.is_none() {
            self.child.start_kill()?;
            let _ = self.child.wait().await;
        }
        Ok(())
    }
}

#[cfg(feature = "stdio")]
impl Drop for StdioCodexAppServerTransport {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.start_kill();
        }
    }
}

#[cfg(test)]
pub mod tests {
    use std::collections::VecDeque;

    use super::*;

    pub struct MockTransport {
        inbound: VecDeque<String>,
        pub written: Vec<String>,
    }

    impl MockTransport {
        pub fn new(inbound: Vec<String>) -> Self {
            Self {
                inbound: inbound.into(),
                written: Vec::new(),
            }
        }
    }

    #[async_trait]
    impl CodexAppServerTransport for MockTransport {
        async fn write_payload(&mut self, payload: &str) -> Result<()> {
            self.written.push(payload.to_owned());
            Ok(())
        }

        async fn read_payload(&mut self) -> Result<String> {
            self.inbound
                .pop_front()
                .ok_or(CodexAppServerError::ConnectionClosed)
        }
    }
}
