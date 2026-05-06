//! Workspace handles.

use std::path::{Path, PathBuf};

use futures::future::BoxFuture;

use crate::{WorkspaceError, WorkspaceView};

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
    pub fn root(&self) -> &Path {
        &self.root
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
}
