//! Workspace handles.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::future::BoxFuture;
use parking_lot::Mutex;

use crate::{
    Command, CommandOutput, WithWorkspaceError, WorkspaceConfig, WorkspaceError, WorkspaceFactory,
    WorkspacePath, WorkspaceView,
};

pub struct Workspace {
    backend: Arc<Mutex<Box<dyn WorkspaceBackend>>>,
    local_mount: Option<PathBuf>,
}

impl Workspace {
    #[must_use]
    pub fn new(_root: PathBuf, backend: Box<dyn WorkspaceBackend>) -> Self {
        let local_mount = backend.local_mount().map(Path::to_path_buf);
        Self {
            backend: Arc::new(Mutex::new(backend)),
            local_mount,
        }
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
            PhantomData,
        )
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
