use std::path::PathBuf;

use leaven_seam_runtime::{RejectingSeamService, SeamRuntime, SeamRuntimeError};
use leaven_seam_stdio::{SeamStdioError, serve_inherited_stdio};

#[derive(Debug)]
pub struct SeamServeCommand {
    pub root: PathBuf,
}

impl SeamServeCommand {
    pub fn run(self) -> Result<String, SeamCommandError> {
        let runtime = SeamRuntime::from_repo(self.root, RejectingSeamService)?;
        serve_inherited_stdio(&runtime)?;
        Ok(String::new())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SeamCommandError {
    #[error(transparent)]
    Runtime(#[from] SeamRuntimeError),
    #[error(transparent)]
    Stdio(#[from] SeamStdioError),
}
