//! Borrowed workspace view.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::{Command, CommandOutput, WorkspaceBackend, WorkspaceError, WorkspacePath};

pub struct WorkspaceView<'a> {
    backend: Arc<Mutex<Box<dyn WorkspaceBackend>>>,
    local_mount: Option<PathBuf>,
    prefix: WorkspacePath,
    marker: PhantomData<&'a mut ()>,
}

impl<'a> WorkspaceView<'a> {
    #[must_use]
    pub(crate) fn from_backend(
        backend: Arc<Mutex<Box<dyn WorkspaceBackend>>>,
        local_mount: Option<PathBuf>,
        prefix: WorkspacePath,
        marker: PhantomData<&'a mut ()>,
    ) -> Self {
        Self {
            backend,
            local_mount,
            prefix,
            marker,
        }
    }

    #[must_use]
    pub fn root(&self) -> &WorkspacePath {
        &self.prefix
    }

    #[must_use]
    pub fn local_mount(&self) -> Option<&Path> {
        self.local_mount.as_deref()
    }

    pub fn subdir(&self, path: WorkspacePath) -> Result<Self, WorkspaceError> {
        Ok(Self {
            backend: self.backend.clone(),
            local_mount: self.local_mount.clone(),
            prefix: if self.prefix.as_str().is_empty() {
                path
            } else {
                self.prefix.join(path.as_str())?
            },
            marker: PhantomData,
        })
    }

    pub fn write_file(&mut self, path: &WorkspacePath, bytes: &[u8]) -> Result<(), WorkspaceError> {
        let path = self.scoped(path)?;
        self.backend.lock().write_file(&path, bytes)
    }

    pub fn read_file(&self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        let path = self.scoped(path)?;
        self.backend.lock().read_file(&path)
    }

    pub fn run_command(&mut self, mut command: Command) -> Result<CommandOutput, WorkspaceError> {
        command.cwd = match command.cwd.as_ref() {
            Some(path) => Some(self.scoped(path)?),
            None if self.prefix.as_str().is_empty() => None,
            None => Some(self.prefix.clone()),
        };
        self.backend.lock().run_command(command)
    }

    fn scoped(&self, path: &WorkspacePath) -> Result<WorkspacePath, WorkspaceError> {
        if self.prefix.as_str().is_empty() {
            Ok(path.clone())
        } else if path.as_str().is_empty() {
            Ok(self.prefix.clone())
        } else {
            Ok(self.prefix.join(path.as_str())?)
        }
    }
}
