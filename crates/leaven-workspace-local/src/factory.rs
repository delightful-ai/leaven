use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use futures::future::{BoxFuture, FutureExt};
use leaven_kernel::RunId;
use leaven_workspace::{
    CapturedOutput, Command, CommandOutput, CommandStdin, ExitStatus, FactoryError, Workspace,
    WorkspaceBackend, WorkspaceConfig, WorkspaceError, WorkspaceFactory, WorkspacePath,
};

/// Allocates local tempdir-backed workspaces.
#[derive(Clone, Debug)]
pub struct LocalWorkspaceFactory {
    root: PathBuf,
}

impl LocalWorkspaceFactory {
    /// Use the process temp directory as the workspace parent.
    #[must_use]
    pub fn temp() -> Self {
        Self {
            root: std::env::temp_dir(),
        }
    }

    /// Use an explicit workspace parent.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl Default for LocalWorkspaceFactory {
    fn default() -> Self {
        Self::temp()
    }
}

impl WorkspaceFactory for LocalWorkspaceFactory {
    async fn allocate(&self, _config: WorkspaceConfig) -> Result<Workspace, FactoryError> {
        let root = self.root.join(format!("leaven-{}", RunId::new()));
        std::fs::create_dir_all(&root).map_err(|err| FactoryError::Allocate(err.to_string()))?;
        Ok(Workspace::new(
            root.clone(),
            Box::new(LocalWorkspaceBackend { root }),
        ))
    }
}

struct LocalWorkspaceBackend {
    root: PathBuf,
}

impl WorkspaceBackend for LocalWorkspaceBackend {
    fn write_file(&mut self, path: &WorkspacePath, bytes: &[u8]) -> Result<(), WorkspaceError> {
        let path = self.host_path(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| WorkspaceError::Io(err.to_string()))?;
        }
        std::fs::write(path, bytes).map_err(|err| WorkspaceError::Io(err.to_string()))
    }

    fn read_file(&mut self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        std::fs::read(self.host_path(path)).map_err(|err| WorkspaceError::Io(err.to_string()))
    }

    fn list_files(&mut self, path: &WorkspacePath) -> Result<Vec<WorkspacePath>, WorkspaceError> {
        let root = self.host_path(path);
        let mut files = Vec::new();
        collect_files(&root, path.clone(), &mut files)?;
        files.sort();
        Ok(files)
    }

    fn set_executable(
        &mut self,
        path: &WorkspacePath,
        executable: bool,
    ) -> Result<(), WorkspaceError> {
        set_host_executable(&self.host_path(path), executable)
    }

    fn is_executable(&mut self, path: &WorkspacePath) -> Result<bool, WorkspaceError> {
        is_host_executable(&self.host_path(path))
    }

    fn run_command(&mut self, command: Command) -> Result<CommandOutput, WorkspaceError> {
        if command.user.is_some() {
            return Err(WorkspaceError::UnsupportedOperation {
                operation: "run_command.user",
            });
        }

        let cwd = command
            .cwd
            .as_ref()
            .map_or_else(|| self.root.clone(), |path| self.host_path(path));

        let start = Instant::now();
        let mut process = std::process::Command::new(&command.program);
        process
            .args(&command.args)
            .current_dir(cwd)
            .envs(&command.env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match &command.stdin {
            CommandStdin::Empty => {
                process.stdin(Stdio::null());
            }
            CommandStdin::Bytes(_) => {
                process.stdin(Stdio::piped());
            }
        }

        let mut child = process
            .spawn()
            .map_err(|err| WorkspaceError::Command(err.to_string()))?;
        if let CommandStdin::Bytes(bytes) = &command.stdin
            && let Some(mut stdin) = child.stdin.take()
        {
            stdin
                .write_all(bytes)
                .map_err(|err| WorkspaceError::Command(err.to_string()))?;
        }

        let output = wait_for_output(child, command.limits.timeout, &command.program, start)?;
        Ok(CommandOutput {
            status: ExitStatus {
                code: output.status.code(),
            },
            stdout: CapturedOutput::new(output.stdout, command.limits.max_stdout_bytes),
            stderr: CapturedOutput::new(output.stderr, command.limits.max_stderr_bytes),
            duration: start.elapsed(),
        })
    }

    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>> {
        async move {
            if self.root.exists() {
                std::fs::remove_dir_all(&self.root)
                    .map_err(|err| WorkspaceError::Cleanup(err.to_string()))?;
            }
            Ok(())
        }
        .boxed()
    }

    fn local_mount(&self) -> Option<&Path> {
        Some(&self.root)
    }
}

fn wait_for_output(
    mut child: std::process::Child,
    timeout: Option<Duration>,
    program: &str,
    start: Instant,
) -> Result<std::process::Output, WorkspaceError> {
    let Some(timeout) = timeout else {
        return child
            .wait_with_output()
            .map_err(|err| WorkspaceError::Command(err.to_string()));
    };

    loop {
        if child
            .try_wait()
            .map_err(|err| WorkspaceError::Command(err.to_string()))?
            .is_some()
        {
            return child
                .wait_with_output()
                .map_err(|err| WorkspaceError::Command(err.to_string()));
        }

        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(WorkspaceError::CommandTimedOut {
                program: program.to_owned(),
                timeout,
            });
        }

        std::thread::sleep(Duration::from_millis(5));
    }
}

impl LocalWorkspaceBackend {
    fn host_path(&self, path: &WorkspacePath) -> PathBuf {
        self.root.join(path.to_host_relative())
    }
}

fn collect_files(
    host_path: &Path,
    workspace_path: WorkspacePath,
    files: &mut Vec<WorkspacePath>,
) -> Result<(), WorkspaceError> {
    let metadata =
        std::fs::metadata(host_path).map_err(|err| WorkspaceError::Io(err.to_string()))?;
    if metadata.is_file() {
        files.push(workspace_path);
        return Ok(());
    }
    for entry in std::fs::read_dir(host_path).map_err(|err| WorkspaceError::Io(err.to_string()))? {
        let entry = entry.map_err(|err| WorkspaceError::Io(err.to_string()))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| WorkspaceError::Io("workspace path is not UTF-8".to_owned()))?;
        let child_path = if workspace_path.as_str().is_empty() {
            WorkspacePath::new(name)?
        } else {
            workspace_path.join(name)?
        };
        collect_files(&entry.path(), child_path, files)?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_host_executable(path: &Path, executable: bool) -> Result<(), WorkspaceError> {
    let mut permissions = std::fs::metadata(path)
        .map_err(|err| WorkspaceError::Io(err.to_string()))?
        .permissions();
    let mut mode = permissions.mode();
    if executable {
        mode |= 0o111;
    } else {
        mode &= !0o111;
    }
    permissions.set_mode(mode);
    std::fs::set_permissions(path, permissions).map_err(|err| WorkspaceError::Io(err.to_string()))
}

#[cfg(not(unix))]
fn set_host_executable(path: &Path, executable: bool) -> Result<(), WorkspaceError> {
    let _ = (path, executable);
    Err(WorkspaceError::UnsupportedOperation {
        operation: "set_executable",
    })
}

#[cfg(unix)]
fn is_host_executable(path: &Path) -> Result<bool, WorkspaceError> {
    let permissions = std::fs::metadata(path)
        .map_err(|err| WorkspaceError::Io(err.to_string()))?
        .permissions();
    Ok(permissions.mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_host_executable(path: &Path) -> Result<bool, WorkspaceError> {
    let _ = path;
    Err(WorkspaceError::UnsupportedOperation {
        operation: "is_executable",
    })
}
