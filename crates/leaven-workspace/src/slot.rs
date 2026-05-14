//! Scoped workspace slots.

use std::any::Any;
use std::sync::Arc;

use crate::{
    Command, CommandOutput, WorkspaceError, WorkspaceFactoryContextError, WorkspacePath,
    WorkspaceView,
};

pub struct WorkspaceSlot<'a> {
    root: WorkspacePath,
    view: WorkspaceView<'a>,
}

impl<'a> WorkspaceSlot<'a> {
    #[must_use]
    pub fn new(root: WorkspacePath, view: WorkspaceView<'a>) -> Self {
        Self { root, view }
    }

    #[must_use]
    pub const fn root(&self) -> &WorkspacePath {
        &self.root
    }

    #[must_use]
    pub const fn view(&self) -> &WorkspaceView<'a> {
        &self.view
    }

    pub fn view_mut(&mut self) -> &mut WorkspaceView<'a> {
        &mut self.view
    }

    pub fn subslot(&self, path: WorkspacePath) -> Result<WorkspaceSlot<'a>, WorkspaceError> {
        let root = self.root.join(path.as_str())?;
        let view = self.view.subdir(path)?;
        Ok(Self { root, view })
    }

    pub fn write_file(
        &mut self,
        path: &WorkspacePath,
        bytes: &[u8],
    ) -> Result<(), WorkspaceError> {
        self.view.write_file(path, bytes)
    }

    pub fn read_file(&self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        self.view.read_file(path)
    }

    pub fn list_files(&self, path: &WorkspacePath) -> Result<Vec<WorkspacePath>, WorkspaceError> {
        self.view.list_files(path)
    }

    pub fn run_command(&mut self, mut command: Command) -> Result<CommandOutput, WorkspaceError> {
        if command.cwd.is_none() {
            command.cwd = Some(WorkspacePath::root());
        }
        self.view.run_command(command)
    }

    pub fn factory_context<T>(&self) -> Result<Arc<T>, WorkspaceFactoryContextError>
    where
        T: Any + Send + Sync + 'static,
    {
        self.view.factory_context::<T>()
    }
}
