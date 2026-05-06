//! Workspace commands.

use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct CommandOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ExitStatus {
    pub code: Option<i32>,
}
