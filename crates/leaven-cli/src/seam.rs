use std::path::PathBuf;

use leaven_public_seam::{
    MethodReceiptExpectation, MethodSchema, PublicSeamPackage, SchemaFingerprint,
};
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
                    "method": method.method().as_str(),
                    "params_schema": method.params_schema().schema_file(),
                    "result_schema": method.result_schema().schema_file(),
                    "required_action": method.required_action().as_str(),
                    "primary_kinds": method
                        .method()
                        .primary_kinds()
                        .iter()
                        .map(|kind| kind.as_str())
                        .collect::<Vec<_>>(),
                    "receipt_expectation": receipt_expectation_value(
                        method.method().receipt_expectation()
                    ),
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

fn receipt_expectation_value(expectation: MethodReceiptExpectation) -> Value {
    match expectation {
        MethodReceiptExpectation::StageRun => json!({"kind": "stage_run"}),
        MethodReceiptExpectation::Query => json!({"kind": "query"}),
        MethodReceiptExpectation::Call(call_kind) => json!({
            "kind": "call",
            "call_kind": call_kind,
        }),
        MethodReceiptExpectation::Write(write_kind) => json!({
            "kind": "write",
            "write_kind": write_kind,
        }),
    }
}

fn schema_fingerprint(
    package: &PublicSeamPackage,
    schema: MethodSchema,
) -> Result<String, SeamCommandError> {
    let value: Value = package.schema_json(schema.schema_file())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seam_profile_exports_typed_method_result_facts() {
        let output = SeamProfileCommand {
            root: workspace_root(),
        }
        .run()
        .expect("profile export succeeds");
        let profile: Value = serde_json::from_str(&output).expect("profile export is JSON");
        let methods = profile["extension_methods"]
            .as_array()
            .expect("profile carries extension methods");

        let agent = method(methods, "leaven/agent.run");
        assert_eq!(agent["primary_kinds"], json!(["agent_session"]));
        assert_eq!(
            agent["receipt_expectation"],
            json!({"kind": "call", "call_kind": "agent_run"})
        );

        let apply = method(methods, "leaven/proposal.apply");
        assert_eq!(apply["primary_kinds"], json!(["apply_receipt"]));
        assert_eq!(
            apply["receipt_expectation"],
            json!({"kind": "write", "write_kind": "apply_proposal_batch"})
        );
    }

    fn method<'a>(methods: &'a [Value], name: &str) -> &'a Value {
        methods
            .iter()
            .find(|method| method["method"] == name)
            .expect("locked method is exported")
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate has crates parent")
            .parent()
            .expect("workspace root exists")
            .to_path_buf()
    }
}
