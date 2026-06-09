use std::io::{Read, Write};
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

#[derive(Clone, Debug)]
pub struct GitWorkspaceFactory {
    source: PathBuf,
    checkout: Option<String>,
    root: PathBuf,
}

impl GitWorkspaceFactory {
    #[must_use]
    pub fn local(source: impl Into<PathBuf>) -> Self {
        Self {
            source: source.into(),
            checkout: None,
            root: std::env::temp_dir(),
        }
    }

    #[must_use]
    pub fn with_checkout(mut self, checkout: impl Into<String>) -> Self {
        self.checkout = Some(checkout.into());
        self
    }

    #[must_use]
    pub fn with_workspace_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = root.into();
        self
    }
}

impl WorkspaceFactory for GitWorkspaceFactory {
    async fn allocate(&self, _config: WorkspaceConfig) -> Result<Workspace, FactoryError> {
        let root = self.root.join(format!("leaven-git-{}", RunId::new()));
        run_git_clone(&self.source, &root)?;
        if let Some(checkout) = &self.checkout {
            if let Err(error) = run_git_checkout(&root, checkout) {
                let _ = std::fs::remove_dir_all(&root);
                return Err(error);
            }
        }
        Ok(Workspace::new(
            root.clone(),
            Box::new(GitWorkspaceBackend { root }),
        ))
    }
}

struct GitWorkspaceBackend {
    root: PathBuf,
}

impl WorkspaceBackend for GitWorkspaceBackend {
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

        let child = process
            .spawn()
            .map_err(|err| WorkspaceError::Command(err.to_string()))?;
        let stdin = match &command.stdin {
            CommandStdin::Empty => None,
            CommandStdin::Bytes(bytes) => Some(bytes.clone()),
        };
        let output = wait_for_output(
            child,
            stdin,
            command.limits.timeout,
            &command.program,
            start,
        )?;
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

impl GitWorkspaceBackend {
    fn host_path(&self, path: &WorkspacePath) -> PathBuf {
        self.root.join(path.to_host_relative())
    }
}

fn run_git_clone(source: &Path, root: &Path) -> Result<(), FactoryError> {
    let output = std::process::Command::new("git")
        .arg("clone")
        .arg("--quiet")
        .arg(source)
        .arg(root)
        .output()
        .map_err(|err| FactoryError::Allocate(err.to_string()))?;
    if output.status.success() {
        return Ok(());
    }
    Err(FactoryError::Allocate(command_failure(
        "git clone",
        &output,
    )))
}

fn run_git_checkout(root: &Path, checkout: &str) -> Result<(), FactoryError> {
    let output = std::process::Command::new("git")
        .arg("checkout")
        .arg("--quiet")
        .arg(checkout)
        .current_dir(root)
        .output()
        .map_err(|err| FactoryError::Allocate(err.to_string()))?;
    if output.status.success() {
        return Ok(());
    }
    Err(FactoryError::Allocate(command_failure(
        "git checkout",
        &output,
    )))
}

fn command_failure(program: &str, output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!(
        "{program} failed with status {:?}: {stderr}",
        output.status.code()
    )
}

fn wait_for_output(
    mut child: std::process::Child,
    stdin: Option<Vec<u8>>,
    timeout: Option<Duration>,
    program: &str,
    start: Instant,
) -> Result<std::process::Output, WorkspaceError> {
    let stdout = spawn_output_drain(child.stdout.take());
    let stderr = spawn_output_drain(child.stderr.take());
    let stdin = spawn_stdin_writer(child.stdin.take(), stdin);

    let Some(timeout) = timeout else {
        let status = child
            .wait()
            .map_err(|err| WorkspaceError::Command(err.to_string()))?;
        join_stdin_writer(stdin)?;
        return Ok(std::process::Output {
            status,
            stdout: join_output_drain(stdout)?,
            stderr: join_output_drain(stderr)?,
        });
    };

    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| WorkspaceError::Command(err.to_string()))?
        {
            join_stdin_writer(stdin)?;
            return Ok(std::process::Output {
                status,
                stdout: join_output_drain(stdout)?,
                stderr: join_output_drain(stderr)?,
            });
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

fn spawn_stdin_writer(
    mut writer: Option<std::process::ChildStdin>,
    bytes: Option<Vec<u8>>,
) -> Option<std::thread::JoinHandle<Result<(), std::io::Error>>> {
    let bytes = bytes?;
    Some(std::thread::spawn(move || {
        if let Some(mut writer) = writer.take() {
            writer.write_all(&bytes)?;
        }
        Ok(())
    }))
}

fn spawn_output_drain<R>(
    reader: Option<R>,
) -> std::thread::JoinHandle<Result<Vec<u8>, std::io::Error>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut reader) = reader {
            reader.read_to_end(&mut bytes)?;
        }
        Ok(bytes)
    })
}

fn join_stdin_writer(
    handle: Option<std::thread::JoinHandle<Result<(), std::io::Error>>>,
) -> Result<(), WorkspaceError> {
    let Some(handle) = handle else {
        return Ok(());
    };
    handle
        .join()
        .map_err(|_| WorkspaceError::Command("stdin writer thread panicked".to_owned()))?
        .map_err(|err| WorkspaceError::Command(err.to_string()))
}

fn join_output_drain(
    handle: std::thread::JoinHandle<Result<Vec<u8>, std::io::Error>>,
) -> Result<Vec<u8>, WorkspaceError> {
    handle
        .join()
        .map_err(|_| WorkspaceError::Command("output drain thread panicked".to_owned()))?
        .map_err(|err| WorkspaceError::Command(err.to_string()))
}

fn collect_files(
    host_path: &Path,
    workspace_path: WorkspacePath,
    files: &mut Vec<WorkspacePath>,
) -> Result<(), WorkspaceError> {
    let metadata =
        std::fs::symlink_metadata(host_path).map_err(|err| WorkspaceError::Io(err.to_string()))?;
    let file_type = metadata.file_type();
    if file_type.is_file() || file_type.is_symlink() {
        files.push(workspace_path);
        return Ok(());
    }
    if !file_type.is_dir() {
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
