//! Workspace handles.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::future::BoxFuture;
use parking_lot::Mutex;

use leaven_kernel::WorkspaceId;

use crate::{
    Command, CommandOutput, WithWorkspaceError, WorkspaceConfig, WorkspaceError,
    WorkspaceFactory, WorkspaceFactoryContext, WorkspaceFactoryContextError, WorkspacePath,
    WorkspaceSlot, WorkspaceView,
};

pub struct Workspace {
    id: WorkspaceId,
    backend: Arc<Mutex<Box<dyn WorkspaceBackend>>>,
    local_mount: Option<PathBuf>,
    factory_context: WorkspaceFactoryContext,
}

impl Workspace {
    #[must_use]
    pub fn new(root: PathBuf, backend: Box<dyn WorkspaceBackend>) -> Self {
        Self::new_with_context(root, backend, WorkspaceFactoryContext::empty())
    }

    #[must_use]
    pub fn new_with_context(
        _root: PathBuf,
        backend: Box<dyn WorkspaceBackend>,
        factory_context: WorkspaceFactoryContext,
    ) -> Self {
        let local_mount = backend.local_mount().map(Path::to_path_buf);
        Self {
            id: WorkspaceId::new(),
            backend: Arc::new(Mutex::new(backend)),
            local_mount,
            factory_context,
        }
    }

    #[must_use]
    pub const fn id(&self) -> WorkspaceId {
        self.id
    }

    #[must_use]
    pub fn root(&self) -> WorkspacePath {
        WorkspacePath::root()
    }

    #[must_use]
    pub fn local_mount(&self) -> Option<&Path> {
        self.local_mount.as_deref()
    }

    #[must_use]
    pub fn view(&mut self) -> WorkspaceView<'_> {
        WorkspaceView::from_backend(
            self.backend.clone(),
            self.local_mount.clone(),
            WorkspacePath::root(),
            self.factory_context.clone(),
            PhantomData,
        )
    }

    pub fn slot(&mut self, root: WorkspacePath) -> Result<WorkspaceSlot<'_>, WorkspaceError> {
        let view = self.view().subdir(root.clone())?;
        Ok(WorkspaceSlot::new(root, view))
    }

    pub fn factory_context<T>(&self) -> Result<Arc<T>, WorkspaceFactoryContextError>
    where
        T: std::any::Any + Send + Sync + 'static,
    {
        self.factory_context.get::<T>()
    }

    pub async fn cleanup(self) -> Result<(), WorkspaceError> {
        let backend = Arc::try_unwrap(self.backend)
            .map_err(|_| WorkspaceError::Cleanup("workspace views are still live".to_owned()))?
            .into_inner();
        backend.cleanup().await
    }
}

pub trait WorkspaceBackend: Send + Sync {
    fn write_file(&mut self, path: &WorkspacePath, bytes: &[u8]) -> Result<(), WorkspaceError> {
        let _ = (path, bytes);
        Err(WorkspaceError::UnsupportedOperation {
            operation: "write_file",
        })
    }

    fn read_file(&mut self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        let _ = path;
        Err(WorkspaceError::UnsupportedOperation {
            operation: "read_file",
        })
    }

    fn list_files(&mut self, path: &WorkspacePath) -> Result<Vec<WorkspacePath>, WorkspaceError> {
        let _ = path;
        Err(WorkspaceError::UnsupportedOperation {
            operation: "list_files",
        })
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
        let _ = command;
        Err(WorkspaceError::UnsupportedOperation {
            operation: "run_command",
        })
    }

    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>>;

    fn local_mount(&self) -> Option<&Path> {
        None
    }
}

pub async fn with_workspace<Factory, F, T, E>(
    factory: &Factory,
    config: WorkspaceConfig,
    f: F,
) -> Result<T, WithWorkspaceError<E>>
where
    Factory: WorkspaceFactory + ?Sized,
    F: for<'workspace> FnOnce(&'workspace mut Workspace) -> BoxFuture<'workspace, Result<T, E>>,
{
    let mut workspace = factory
        .allocate(config)
        .await
        .map_err(WithWorkspaceError::Allocate)?;
    let stage_result = f(&mut workspace).await;
    let cleanup_result = workspace.cleanup().await;

    match (stage_result, cleanup_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(error)) => Err(WithWorkspaceError::Cleanup(error)),
        (Err(error), Ok(())) => Err(WithWorkspaceError::Stage(error)),
        (Err(stage), Err(cleanup)) => Err(WithWorkspaceError::StageAndCleanup { stage, cleanup }),
    }
}
