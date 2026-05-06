//! Borrowed workspace view.

use std::path::{Path, PathBuf};

use crate::{Command, CommandOutput, WorkspaceError};

pub struct WorkspaceView<'a> {
    root: &'a Path,
}

impl<'a> WorkspaceView<'a> {
    #[must_use]
    pub fn new(root: &'a Path) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn root(&self) -> &'a Path {
        self.root
    }

    #[must_use]
    pub fn subdir(&self, path: impl Into<PathBuf>) -> PathBuf {
        self.root.join(path.into())
    }

    pub fn write_file(
        &mut self,
        path: impl AsRef<Path>,
        bytes: &[u8],
    ) -> Result<(), WorkspaceError> {
        let path = self.root.join(path.as_ref());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| WorkspaceError::Io(err.to_string()))?;
        }
        std::fs::write(path, bytes).map_err(|err| WorkspaceError::Io(err.to_string()))
    }

    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<Vec<u8>, WorkspaceError> {
        std::fs::read(self.root.join(path.as_ref()))
            .map_err(|err| WorkspaceError::Io(err.to_string()))
    }

    pub fn run_command(&mut self, _command: Command) -> Result<CommandOutput, WorkspaceError> {
        Err(WorkspaceError::Command(
            "workspace backend is not attached".to_owned(),
        ))
    }
}
