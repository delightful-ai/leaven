//! Borrowed workspace view.

use std::path::{Path, PathBuf};

use crate::{Command, CommandOutput, WorkspaceError, WorkspacePath};

pub struct WorkspaceView<'a> {
    root: &'a Path,
    prefix: WorkspacePath,
}

impl<'a> WorkspaceView<'a> {
    #[must_use]
    pub fn new(root: &'a Path) -> Self {
        Self {
            root,
            prefix: WorkspacePath::root(),
        }
    }

    #[must_use]
    pub fn root(&self) -> &WorkspacePath {
        &self.prefix
    }

    pub fn subdir(&self, path: WorkspacePath) -> Result<Self, WorkspaceError> {
        Ok(Self {
            root: self.root,
            prefix: if self.prefix.as_str().is_empty() {
                path
            } else {
                self.prefix.join(path.as_str())?
            },
        })
    }

    pub fn write_file(&mut self, path: &WorkspacePath, bytes: &[u8]) -> Result<(), WorkspaceError> {
        let path = self.host_path(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| WorkspaceError::Io(err.to_string()))?;
        }
        std::fs::write(path, bytes).map_err(|err| WorkspaceError::Io(err.to_string()))
    }

    pub fn read_file(&self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        std::fs::read(self.host_path(path)).map_err(|err| WorkspaceError::Io(err.to_string()))
    }

    pub fn run_command(&mut self, _command: Command) -> Result<CommandOutput, WorkspaceError> {
        Err(WorkspaceError::Command(
            "workspace backend is not attached".to_owned(),
        ))
    }

    fn host_path(&self, path: &WorkspacePath) -> PathBuf {
        self.root
            .join(self.prefix.to_host_relative())
            .join(path.to_host_relative())
    }
}
