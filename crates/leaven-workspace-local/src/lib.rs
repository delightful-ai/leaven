//! Local filesystem workspace backend.

use std::path::{Path, PathBuf};

use futures::future::{BoxFuture, FutureExt};
use leaven_kernel::RunId;
use leaven_workspace::{
    FactoryError, Workspace, WorkspaceBackend, WorkspaceConfig, WorkspaceError, WorkspaceFactory,
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
