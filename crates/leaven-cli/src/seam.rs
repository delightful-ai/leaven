use std::path::PathBuf;

use leaven_seam_runtime::{SeamRuntime, SeamRuntimeError};
use leaven_seam_service::{ConfiguredSeamService, ConfiguredSeamServiceError, SeamServiceConfig};
use leaven_seam_stdio::{SeamStdioError, serve_inherited_stdio};

#[derive(Debug)]
pub struct SeamServeCommand {
    pub root: PathBuf,
    pub config: Option<PathBuf>,
}

impl SeamServeCommand {
    pub fn run(self) -> Result<String, SeamCommandError> {
        let config = match self.config {
            Some(path) => serde_json::from_reader(std::fs::File::open(path)?)?,
            None => SeamServiceConfig::default(),
        };
        let service = ConfiguredSeamService::from_repo(&self.root, config)?;
        let runtime = SeamRuntime::from_repo(self.root, service)?;
        serve_inherited_stdio(&runtime)?;
        Ok(String::new())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SeamCommandError {
    #[error(transparent)]
    Runtime(#[from] SeamRuntimeError),
    #[error(transparent)]
    Service(#[from] ConfiguredSeamServiceError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Stdio(#[from] SeamStdioError),
}
