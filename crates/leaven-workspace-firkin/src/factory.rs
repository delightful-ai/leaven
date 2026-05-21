use std::sync::Arc;

use futures::future::{BoxFuture, FutureExt};
use leaven_workspace::{
    CapturedOutput, Command, CommandOutput, CommandStdin, FactoryError, Workspace,
    WorkspaceBackend, WorkspaceConfig, WorkspaceError, WorkspaceFactory, WorkspaceFactoryContext,
    WorkspacePath,
};

use crate::{
    FirkinCommandRequest, FirkinCommandResult, FirkinContainerId, FirkinGuestPath, FirkinImageRef,
    FirkinProductPodId, FirkinRuntimeError, FirkinWorkspaceAllocation, FirkinWorkspaceContext,
    FirkinWorkspaceRuntime,
};

#[derive(Clone)]
pub struct FirkinWorkspaceFactory<R> {
    runtime: Arc<R>,
    product_pod_id: FirkinProductPodId,
    workspace_root: FirkinGuestPath,
    image: FirkinImageRef,
}

impl<R> FirkinWorkspaceFactory<R>
where
    R: FirkinWorkspaceRuntime,
{
    #[must_use]
    pub fn new(
        runtime: Arc<R>,
        product_pod_id: FirkinProductPodId,
        workspace_root: FirkinGuestPath,
        image: FirkinImageRef,
    ) -> Self {
        Self {
            runtime,
            product_pod_id,
            workspace_root,
            image,
        }
    }
}

impl<R> WorkspaceFactory for FirkinWorkspaceFactory<R>
where
    R: FirkinWorkspaceRuntime,
{
    async fn allocate(&self, _config: WorkspaceConfig) -> Result<Workspace, FactoryError> {
        let allocation = FirkinWorkspaceAllocation::new(
            self.product_pod_id.clone(),
            self.workspace_root.clone(),
            self.image.clone(),
        );
        let container_id = self
            .runtime
            .allocate_container(allocation)
            .map_err(factory_error("allocate Firkin workspace container"))?;
        let context = FirkinWorkspaceContext::new(
            self.product_pod_id.clone(),
            container_id.clone(),
            self.workspace_root.clone(),
            self.image.clone(),
        );
        let mut builder = WorkspaceFactoryContext::builder();
        builder
            .insert(Arc::new(context))
            .map_err(|error| FactoryError::Allocate(error.to_string()))?;
        Ok(Workspace::new_with_context(
            std::path::PathBuf::new(),
            Box::new(FirkinWorkspaceBackend {
                runtime: self.runtime.clone(),
                container_id,
                workspace_root: self.workspace_root.clone(),
            }),
            builder.build(),
        ))
    }
}

struct FirkinWorkspaceBackend<R> {
    runtime: Arc<R>,
    container_id: FirkinContainerId,
    workspace_root: FirkinGuestPath,
}

impl<R> WorkspaceBackend for FirkinWorkspaceBackend<R>
where
    R: FirkinWorkspaceRuntime,
{
    fn write_file(&mut self, path: &WorkspacePath, bytes: &[u8]) -> Result<(), WorkspaceError> {
        self.runtime
            .write_file(&self.container_id, &self.guest_path(path), bytes)
            .map_err(workspace_io_error("write Firkin workspace file"))
    }

    fn read_file(&mut self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        self.runtime
            .read_file(&self.container_id, &self.guest_path(path))
            .map_err(workspace_io_error("read Firkin workspace file"))
    }

    fn list_files(&mut self, path: &WorkspacePath) -> Result<Vec<WorkspacePath>, WorkspaceError> {
        self.runtime
            .list_files(&self.container_id, &self.guest_path(path))
            .map_err(workspace_io_error("list Firkin workspace files"))
    }

    fn set_executable(
        &mut self,
        path: &WorkspacePath,
        executable: bool,
    ) -> Result<(), WorkspaceError> {
        let _ = (path, executable);
        Err(WorkspaceError::UnsupportedOperation {
            operation: "set_executable",
        })
    }

    fn is_executable(&mut self, path: &WorkspacePath) -> Result<bool, WorkspaceError> {
        let _ = path;
        Err(WorkspaceError::UnsupportedOperation {
            operation: "is_executable",
        })
    }

    fn run_command(&mut self, command: Command) -> Result<CommandOutput, WorkspaceError> {
        let cwd = command
            .cwd
            .as_ref()
            .map_or_else(|| self.workspace_root.clone(), |path| self.guest_path(path));
        let max_stdout_bytes = command.limits.max_stdout_bytes;
        let max_stderr_bytes = command.limits.max_stderr_bytes;
        let request = FirkinCommandRequest {
            program: command.program,
            args: command.args,
            cwd,
            env: command.env,
            stdin: match command.stdin {
                CommandStdin::Empty => Vec::new(),
                CommandStdin::Bytes(bytes) => bytes,
            },
            timeout: command.limits.timeout,
            max_stdout_bytes,
            max_stderr_bytes,
            user: command.user,
        };
        let output = self
            .runtime
            .run_command(&self.container_id, request)
            .map_err(workspace_command_error("run Firkin workspace command"))?;
        Ok(command_output(output, max_stdout_bytes, max_stderr_bytes))
    }

    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>> {
        async move {
            self.runtime
                .remove_container(self.container_id)
                .map_err(workspace_io_error("remove Firkin workspace container"))
        }
        .boxed()
    }
}

impl<R> FirkinWorkspaceBackend<R>
where
    R: FirkinWorkspaceRuntime,
{
    fn guest_path(&self, path: &WorkspacePath) -> FirkinGuestPath {
        self.workspace_root.join_workspace_path(path)
    }
}

fn command_output(
    output: FirkinCommandResult,
    max_stdout_bytes: Option<u64>,
    max_stderr_bytes: Option<u64>,
) -> CommandOutput {
    CommandOutput {
        status: output.status,
        stdout: limit_output(output.stdout, max_stdout_bytes),
        stderr: limit_output(output.stderr, max_stderr_bytes),
        duration: output.duration,
    }
}

fn limit_output(output: CapturedOutput, limit: Option<u64>) -> CapturedOutput {
    let runtime_truncated = output.truncated;
    let mut limited = CapturedOutput::new(output.bytes, limit);
    limited.truncated |= runtime_truncated;
    limited
}

fn factory_error(
    operation: &'static str,
) -> impl FnOnce(FirkinRuntimeError) -> FactoryError + 'static {
    move |error| FactoryError::Allocate(format!("{operation}: {error}"))
}

fn workspace_io_error(
    operation: &'static str,
) -> impl FnOnce(FirkinRuntimeError) -> WorkspaceError + 'static {
    move |error| WorkspaceError::Io(format!("{operation}: {error}"))
}

fn workspace_command_error(
    operation: &'static str,
) -> impl FnOnce(FirkinRuntimeError) -> WorkspaceError + 'static {
    move |error| WorkspaceError::Command(format!("{operation}: {error}"))
}
