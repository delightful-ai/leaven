use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use futures::future::{BoxFuture, FutureExt};
use leaven_kernel::RunId;
use leaven_workspace::{
    Command, CommandOutput, ExitStatus, FactoryError, Workspace, WorkspaceBackend, WorkspaceConfig,
    WorkspaceError, WorkspaceFactory, WorkspacePath,
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
        let cwd = command
            .cwd
            .as_ref()
            .map_or_else(|| self.root.clone(), |path| self.host_path(path));
        let output = std::process::Command::new(&command.program)
            .args(&command.args)
            .current_dir(cwd)
            .output()
            .map_err(|err| WorkspaceError::Command(err.to_string()))?;
        Ok(CommandOutput {
            status: ExitStatus {
                code: output.status.code(),
            },
            stdout: output.stdout,
            stderr: output.stderr,
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
