//! Configured executable services behind the Leaven public seam runtime.

use std::path::Path;

use futures::executor::block_on;
use leaven_lm_mock::{MockLm, MockLmScript};
use leaven_public_seam::{
    PlanEmitRunEventOutcome, PlanEmitRunEventRequest, PlanExecutionContext, PlanExecutionHost,
    PlanLmCompleteOutcome, PlanLmCompleteRequest, PublicSeamError, PublicSeamPackage,
};
use leaven_seam_runtime::{SeamPlanRequest, SeamService, SeamServiceError, SeamStageRunRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Configured public-seam service that executes supported Plan IR effects.
#[derive(Clone, Debug)]
pub struct ConfiguredSeamService {
    package: PublicSeamPackage,
    config: SeamServiceConfig,
}

impl ConfiguredSeamService {
    /// Loads the active public-seam package from a repository root.
    pub fn from_repo(
        root: impl AsRef<Path>,
        config: SeamServiceConfig,
    ) -> Result<Self, ConfiguredSeamServiceError> {
        let package = PublicSeamPackage::active_from_repo(root)?;
        Self::from_package(package, config)
    }

    /// Builds a service from an already loaded public-seam package.
    pub fn from_package(
        package: PublicSeamPackage,
        config: SeamServiceConfig,
    ) -> Result<Self, ConfiguredSeamServiceError> {
        config.lm.validate()?;
        Ok(Self { package, config })
    }

    /// Service configuration.
    pub const fn config(&self) -> &SeamServiceConfig {
        &self.config
    }
}

impl SeamService for ConfiguredSeamService {
    fn handle_plan(&self, request: SeamPlanRequest<'_>) -> Result<Value, SeamServiceError> {
        let context = self.config.context.to_execution_context();
        let mut host = ConfiguredPlanHost {
            lm: self.config.lm.to_mock_lm(),
        };
        let report = self
            .package
            .execute_plan_document(request.params(), &context, &mut host)
            .map_err(|error| SeamServiceError::execution(error.to_string()))?;
        extension_result_for_plan_report(request.method(), request.params(), report.value())
            .map_err(|error| SeamServiceError::execution(error.to_string()))
    }

    fn handle_stage_run(
        &self,
        _request: SeamStageRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        Err(SeamServiceError::unavailable("leaven/stage.run"))
    }
}

fn extension_result_for_plan_report(
    method: &str,
    plan: &Value,
    result: &Value,
) -> Result<Value, PublicSeamError> {
    let primary_name = plan
        .get("return")
        .and_then(Value::as_array)
        .and_then(|returns| returns.first())
        .and_then(Value::as_str)
        .ok_or_else(|| PublicSeamError::InvalidPlan {
            message: "public seam method execution requires at least one returned value".to_owned(),
        })?;
    let primary = result
        .get("values")
        .and_then(|values| values.get(primary_name))
        .ok_or_else(|| PublicSeamError::InvalidPlan {
            message: format!("public seam method result missing returned value `{primary_name}`"),
        })?;
    let data_classes = primary
        .get("data_classes")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    Ok(serde_json::json!({
        "method": method,
        "primary": primary,
        "receipts": result.get("receipts").cloned().unwrap_or_else(|| serde_json::json!([])),
        "redactions": result.get("redactions").cloned().unwrap_or_else(|| serde_json::json!([])),
        "capability_fingerprint": result.get("capability_fingerprint").cloned().unwrap_or_else(|| serde_json::json!("fp_cap_sha256_missing")),
        "policy_fingerprint": result.get("policy_fingerprint").cloned().unwrap_or_else(|| serde_json::json!("fp_policy_sha256_missing")),
        "data_classes": data_classes
    }))
}

/// Serve-process configuration for executable public-seam methods.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SeamServiceConfig {
    /// Execution context projected into Plan Result receipts.
    pub context: SeamExecutionContextConfig,
    /// LM provider configuration.
    pub lm: SeamLmConfig,
}

impl Default for SeamServiceConfig {
    fn default() -> Self {
        Self {
            context: SeamExecutionContextConfig::default(),
            lm: SeamLmConfig::default(),
        }
    }
}

/// Stable execution metadata for one local seam service.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SeamExecutionContextConfig {
    /// Capability fingerprint used for receipts.
    pub capability_fingerprint: String,
    /// Policy fingerprint used for receipts.
    pub policy_fingerprint: String,
    /// Base graph revision used for no-write plans.
    pub base_revision: String,
    /// Execution start timestamp.
    pub started_at: String,
    /// Execution completion timestamp.
    pub completed_at: String,
}

impl SeamExecutionContextConfig {
    fn to_execution_context(&self) -> PlanExecutionContext {
        PlanExecutionContext::new(
            &self.capability_fingerprint,
            &self.policy_fingerprint,
            &self.base_revision,
            &self.started_at,
            &self.completed_at,
        )
    }
}

impl Default for SeamExecutionContextConfig {
    fn default() -> Self {
        Self {
            capability_fingerprint: "fp_cap_sha256_leaven_seam_local".to_owned(),
            policy_fingerprint: "fp_policy_sha256_leaven_seam_local".to_owned(),
            base_revision: "rev_leaven_seam_local_base".to_owned(),
            started_at: "2026-01-01T00:00:00Z".to_owned(),
            completed_at: "2026-01-01T00:00:01Z".to_owned(),
        }
    }
}

/// Configured LM provider for public-seam execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SeamLmConfig {
    /// Deterministic local LM script. This is mechanics evidence, not live
    /// provider proof.
    Mock {
        /// Responses consumed in order by executed `lm_complete` calls.
        responses: Vec<MockLmResponseConfig>,
    },
}

impl SeamLmConfig {
    fn validate(&self) -> Result<(), ConfiguredSeamServiceError> {
        match self {
            Self::Mock { responses } if responses.is_empty() => {
                Err(ConfiguredSeamServiceError::EmptyMockLmScript)
            }
            Self::Mock { .. } => Ok(()),
        }
    }

    fn to_mock_lm(&self) -> MockLm {
        match self {
            Self::Mock { responses } => {
                let script = responses
                    .iter()
                    .fold(MockLmScript::new(), |script, response| {
                        script.then_text(
                            response.text.clone(),
                            response.input_tokens,
                            response.output_tokens,
                        )
                    });
                MockLm::new(script)
            }
        }
    }
}

impl Default for SeamLmConfig {
    fn default() -> Self {
        Self::Mock {
            responses: vec![MockLmResponseConfig::default()],
        }
    }
}

/// One deterministic mock LM response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MockLmResponseConfig {
    /// Assistant text returned by the mock LM.
    pub text: String,
    /// Input-token count charged by the response.
    pub input_tokens: u64,
    /// Output-token count charged by the response.
    pub output_tokens: u64,
}

impl Default for MockLmResponseConfig {
    fn default() -> Self {
        Self {
            text: "ok".to_owned(),
            input_tokens: 1,
            output_tokens: 1,
        }
    }
}

struct ConfiguredPlanHost {
    lm: MockLm,
}

impl PlanExecutionHost for ConfiguredPlanHost {
    fn lm_complete(
        &mut self,
        request: PlanLmCompleteRequest<'_>,
    ) -> Result<PlanLmCompleteOutcome, PublicSeamError> {
        block_on(request.execute_with_lm(&self.lm))
    }

    fn emit_run_event(
        &mut self,
        request: PlanEmitRunEventRequest<'_>,
    ) -> Result<PlanEmitRunEventOutcome, PublicSeamError> {
        Err(PublicSeamError::InvalidPlan {
            message: format!(
                "configured seam service cannot emit run event `{}` yet",
                request.name()
            ),
        })
    }
}

/// Error while constructing a configured public-seam service.
#[derive(Debug, thiserror::Error)]
pub enum ConfiguredSeamServiceError {
    /// The public-seam package could not be loaded.
    #[error(transparent)]
    PublicSeam(#[from] PublicSeamError),
    /// A mock LM must include at least one response.
    #[error("mock LM config must include at least one response")]
    EmptyMockLmScript,
}

#[cfg(test)]
mod tests {
    use leaven_seam_runtime::{JsonRpcErrorCode, SeamRuntime};
    use serde_json::{Value, json};

    use super::{ConfiguredSeamService, MockLmResponseConfig, SeamLmConfig, SeamServiceConfig};

    #[test]
    fn seam_runtime_executes_lm_complete_through_configured_service() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service = ConfiguredSeamService::from_package(
            package.clone(),
            SeamServiceConfig {
                lm: SeamLmConfig::Mock {
                    responses: vec![MockLmResponseConfig {
                        text: "configured service ok".to_owned(),
                        input_tokens: 7,
                        output_tokens: 3,
                    }],
                },
                ..SeamServiceConfig::default()
            },
        )
        .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&lm_complete_request());

        assert!(
            !response.is_error(),
            "unexpected error: {:?}",
            response.value()
        );
        assert_eq!(
            response.value()["result"]["primary"]["message"]["content"][0]["text"],
            "configured service ok"
        );
        assert_eq!(
            response.value()["result"]["primary"]["cost"],
            json!({
                "input_tokens": 7,
                "output_tokens": 3,
                "lm_calls": 1
            })
        );
        assert_eq!(
            response.value()["result"]["receipts"][0]["call_kind"],
            "lm_complete"
        );
    }

    #[test]
    fn seam_runtime_reports_provider_execution_failure_distinct_from_unwired_method() {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(repo_root()).unwrap();
        let service =
            ConfiguredSeamService::from_package(package.clone(), SeamServiceConfig::default())
                .unwrap();
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let response = runtime.handle_value(&two_call_request_with_one_mock_response());

        assert!(response.is_error());
        assert_eq!(
            response.value()["error"]["code"],
            JsonRpcErrorCode::ExecutionFailed.code()
        );
        assert!(
            response.value()["error"]["message"]
                .as_str()
                .unwrap()
                .contains("mock script exhausted")
        );
    }

    fn lm_complete_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": "lm-1",
            "method": "leaven/lm.complete",
            "params": lm_plan()
        })
    }

    fn two_call_request_with_one_mock_response() -> Value {
        let mut plan = lm_plan();
        let second = plan["ops"][0].clone();
        plan["ops"].as_array_mut().unwrap().push(second);
        plan["ops"][1]["name"] = json!("completion_2");
        plan["ops"][1]["idempotency_key"] = json!("lm-service-0002");
        plan["return"] = json!(["completion", "completion_2"]);
        json!({
            "jsonrpc": "2.0",
            "id": "lm-2",
            "method": "leaven/lm.complete",
            "params": plan
        })
    }

    fn lm_plan() -> Value {
        json!({
            "schema_version": "leaven.plan.v1",
            "plan_id": "planlmservice001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [
                {
                    "kind": "call",
                    "name": "completion",
                    "idempotency_key": "lm-service-0001",
                    "call": {
                        "kind": "lm_complete",
                        "purpose": "test.seam_service",
                        "model": "gpt-4.1-mini",
                        "messages": [
                            {
                                "role": "developer",
                                "content": [{"kind": "text", "text": "return the final answer"}]
                            },
                            {
                                "role": "user",
                                "content": [{"kind": "text", "text": "solve"}]
                            }
                        ],
                        "output": {
                            "kind": "final_message",
                            "max_bytes": 256
                        },
                        "input_classes": ["public"]
                    }
                }
            ],
            "return": ["completion"],
            "commit": {
                "kind": "no_graph_writes"
            }
        })
    }

    fn repo_root() -> &'static std::path::Path {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .unwrap()
    }
}
