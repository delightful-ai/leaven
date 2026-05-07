//! Workspace handles.

use std::path::{Path, PathBuf};

use futures::future::BoxFuture;

use crate::{WorkspaceError, WorkspacePath, WorkspaceView};

pub struct Workspace {
    root: PathBuf,
    backend: Box<dyn WorkspaceBackend>,
}

impl Workspace {
    #[must_use]
    pub fn new(root: PathBuf, backend: Box<dyn WorkspaceBackend>) -> Self {
        Self { root, backend }
    }

    #[must_use]
    pub fn root(&self) -> WorkspacePath {
        WorkspacePath::root()
    }

    #[must_use]
    pub fn local_mount(&self) -> Option<&Path> {
        self.backend.local_mount()
    }

    #[must_use]
    pub fn view(&mut self) -> WorkspaceView<'_> {
        WorkspaceView::new(&self.root)
    }

    pub async fn cleanup(self) -> Result<(), WorkspaceError> {
        self.backend.cleanup().await
    }
}

pub trait WorkspaceBackend: Send + Sync {
    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>>;

    fn local_mount(&self) -> Option<&Path> {
        None
    }
}
