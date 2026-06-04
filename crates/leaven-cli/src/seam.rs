use std::path::PathBuf;

use leaven_public_seam::{PublicSeamPackage, SchemaFingerprint};
use leaven_seam_runtime::{SeamRuntime, SeamRuntimeError};
use leaven_seam_service::{ConfiguredSeamService, ConfiguredSeamServiceError, SeamServiceConfig};
use leaven_seam_stdio::{SeamStdioError, serve_inherited_stdio};
use serde_json::{Value, json};

#[derive(Debug)]
pub struct SeamProfileCommand {
    pub root: PathBuf,
}

impl SeamProfileCommand {
    pub fn run(self) -> Result<String, SeamCommandError> {
        let package = PublicSeamPackage::active_from_repo(&self.root)?;
        let profile = package.locked_acp_profile_document()?;
        let methods = profile
            .extension_methods()
            .iter()
            .map(|method| {
                let params_fingerprint = schema_fingerprint(&package, method.params_schema())?;
                let result_fingerprint = schema_fingerprint(&package, method.result_schema())?;
                Ok(json!({
                    "method": method.method(),
                    "params_schema": method.params_schema(),
                    "result_schema": method.result_schema(),
                    "required_action": method.required_action(),
                    "params_schema_fingerprint": params_fingerprint,
                    "result_schema_fingerprint": result_fingerprint,
                    "produces_receipt": method.produces_receipt()
                }))
            })
            .collect::<Result<Vec<_>, SeamCommandError>>()?;
        serde_json::to_string_pretty(&json!({
            "schema_version": "leaven.seam_profile_export.v1",
            "source": "leaven-public-seam locked ACP profile",
            "extension_methods": methods,
        }))
        .map(|mut output| {
            output.push('\n');
            output
        })
        .map_err(SeamCommandError::from)
    }
}

fn schema_fingerprint(
    package: &PublicSeamPackage,
    schema: &str,
) -> Result<String, SeamCommandError> {
    let value: Value = package.schema_json(schema)?;
    Ok(SchemaFingerprint::for_json_value(&value)?
        .as_str()
        .to_owned())
}

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
    PublicSeam(#[from] leaven_public_seam::PublicSeamError),
    #[error(transparent)]
    Service(#[from] ConfiguredSeamServiceError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Stdio(#[from] SeamStdioError),
}
