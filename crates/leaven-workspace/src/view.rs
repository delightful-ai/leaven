//! Borrowed workspace view.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::{
    Command, CommandOutput, WorkspaceBackend, WorkspaceError, WorkspaceFactoryContext,
    WorkspaceFactoryContextError, WorkspacePath,
};

pub struct WorkspaceView<'a> {
    backend: Arc<Mutex<Box<dyn WorkspaceBackend>>>,
    local_mount: Option<PathBuf>,
    prefix: WorkspacePath,
    factory_context: WorkspaceFactoryContext,
    marker: PhantomData<&'a mut ()>,
}

impl<'a> WorkspaceView<'a> {
    #[must_use]
    pub(crate) fn from_backend(
        backend: Arc<Mutex<Box<dyn WorkspaceBackend>>>,
        local_mount: Option<PathBuf>,
        prefix: WorkspacePath,
        factory_context: WorkspaceFactoryContext,
        marker: PhantomData<&'a mut ()>,
    ) -> Self {
        Self {
            backend,
            local_mount,
            prefix,
            factory_context,
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
            factory_context: self.factory_context.clone(),
            marker: PhantomData,
        })
    }

    pub fn factory_context<T>(&self) -> Result<Arc<T>, WorkspaceFactoryContextError>
    where
        T: std::any::Any + Send + Sync + 'static,
    {
        self.factory_context.get::<T>()
    }

    pub fn write_file(&mut self, path: &WorkspacePath, bytes: &[u8]) -> Result<(), WorkspaceError> {
        let path = self.scoped(path)?;
        self.backend.lock().write_file(&path, bytes)
    }

    pub fn read_file(&self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        let path = self.scoped(path)?;
        self.backend.lock().read_file(&path)
    }

    pub fn list_files(&self, path: &WorkspacePath) -> Result<Vec<WorkspacePath>, WorkspaceError> {
        let scoped = self.scoped(path)?;
        let files = self.backend.lock().list_files(&scoped)?;
        files
            .into_iter()
            .map(|path| self.unscoped(path))
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn set_executable(
        &mut self,
        path: &WorkspacePath,
        executable: bool,
    ) -> Result<(), WorkspaceError> {
        let path = self.scoped(path)?;
        self.backend.lock().set_executable(&path, executable)
    }

    pub fn is_executable(&self, path: &WorkspacePath) -> Result<bool, WorkspaceError> {
        let path = self.scoped(path)?;
        self.backend.lock().is_executable(&path)
    }

    pub fn run_command(&mut self, mut command: Command) -> Result<CommandOutput, WorkspaceError> {
        command.cwd = match command.cwd.as_ref() {
            Some(path) => Some(self.scoped(path)?),
            None if self.prefix.as_str().is_empty() => None,
            None => Some(self.prefix.clone()),
        };
        command.output_files = command
            .output_files
            .iter()
            .map(|path| self.scoped(path))
            .collect::<Result<Vec<_>, _>>()?;
        let mut output = self.backend.lock().run_command(command)?;
        output.output_files = output
            .output_files
            .into_iter()
            .map(|(path, output)| Ok((self.unscoped(path)?, output)))
            .collect::<Result<_, WorkspaceError>>()?;
        Ok(output)
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

    fn unscoped(&self, path: WorkspacePath) -> Result<WorkspacePath, WorkspaceError> {
        let prefix = self.prefix.as_str();
        if prefix.is_empty() {
            return Ok(path);
        }
        let raw = path.as_str();
        let Some(stripped) = raw.strip_prefix(prefix) else {
            return Err(WorkspaceError::Path(
                crate::WorkspacePathError::OutsideView {
                    path: raw.to_owned(),
                    prefix: prefix.to_owned(),
                },
            ));
        };
        let stripped = stripped.strip_prefix('/').unwrap_or(stripped);
        if stripped.is_empty() {
            Ok(WorkspacePath::root())
        } else {
            Ok(WorkspacePath::new(stripped)?)
        }
    }
}
