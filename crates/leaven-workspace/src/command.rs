//! Workspace commands.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::WorkspacePath;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<WorkspacePath>,
    pub env: BTreeMap<String, String>,
    pub stdin: CommandStdin,
    pub limits: CommandLimits,
    pub user: Option<CommandUser>,
}

impl Command {
    #[must_use]
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::new(),
            stdin: CommandStdin::Empty,
            limits: CommandLimits::default(),
            user: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandStdin {
    Empty,
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandLimits {
    pub timeout: Option<Duration>,
    pub max_stdout_bytes: Option<u64>,
    pub max_stderr_bytes: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandUser {
    Name(String),
    Uid(u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandOutput {
    pub status: ExitStatus,
    pub stdout: CapturedOutput,
    pub stderr: CapturedOutput,
    pub duration: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedOutput {
    pub bytes: Vec<u8>,
    pub truncated: bool,
}

impl CapturedOutput {
    #[must_use]
    pub fn new(bytes: Vec<u8>, limit: Option<u64>) -> Self {
        let Some(limit) = limit else {
            return Self {
                bytes,
                truncated: false,
            };
        };
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        if bytes.len() <= limit {
            return Self {
                bytes,
                truncated: false,
            };
        }
        Self {
            bytes: bytes.into_iter().take(limit).collect(),
            truncated: true,
        }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self {
            bytes: Vec::new(),
            truncated: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ExitStatus {
    pub code: Option<i32>,
}
