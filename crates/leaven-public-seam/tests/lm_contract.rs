use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use leaven_kernel::{Cost, Fingerprint, Metered};
use leaven_lm::{
    Lm, LmError, LmId, LmRequest, LmResponse, Message, MessageContentPart, ModelName, OutputMode,
    Role, TokenUsage,
};
use leaven_public_seam::{
    PlanEmitRunEventOutcome, PlanEmitRunEventRequest, PlanExecutionContext, PlanExecutionHost,
    PlanLmCompleteOutcome, PlanLmCompleteRequest, PublicSeamError, PublicSeamPackage,
    SchemaFingerprint,
};
use serde_json::{Value, json};

#[test]
fn lm_complete_can_execute_through_provider_neutral_lm_trait_and_preserve_cost() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let lm = Arc::new(ScriptedLm::new(scripted_response(Message::assistant(
        "trait ok",
    ))));
    let mut host = LmTraitHost::new(Arc::clone(&lm));

    let report = package
        .execute_plan_document(&lm_plan(), &plan_execution_context(), &mut host)
        .unwrap();

    let recorded = lm.recorded_requests.lock().unwrap();
    let request = recorded.first().expect("LM request should be recorded");
    assert_eq!(request.model, ModelName::new("gpt-4.1-mini"));
    assert!(matches!(
        request.output,
        OutputMode::FinalMessage {
            max_bytes: Some(256)
        }
    ));
    assert_eq!(
        request
            .messages
            .iter()
            .map(leaven_lm::Message::role)
            .collect::<Vec<_>>(),
        vec![Role::Developer, Role::User, Role::Tool]
    );
    assert!(matches!(
        request.messages.as_slice()[2].content_parts(),
        [MessageContentPart::ToolResult {
            tool_call_id,
            content
        }] if tool_call_id == "call_lookup_1" && content == "{\"hint\":\"ok\"}"
    ));
    assert_eq!(request.tools[0].name, "lookup");
    assert_eq!(
        request.provider_hints.values["cache:key"],
        json!("lm-contract")
    );
    assert_eq!(
        request.provider_hints.prompt_cache_key.as_deref(),
        Some("suite-stable")
    );
    assert_eq!(request.provider_hints.store, Some(false));
    assert_eq!(
        request.provider_hints.metadata["split"].as_str(),
        "validation"
    );

    assert_eq!(
        report.value()["values"]["completion"]["message"]["content"][0]["text"],
        "trait ok"
    );
    assert_eq!(
        report.value()["values"]["completion"]["cost"],
        json!({
            "input_tokens": 11,
            "output_tokens": 5,
            "lm_calls": 1
        })
    );
    assert_eq!(
        report.value()["receipts"][0]["cost"],
        report.value()["values"]["completion"]["cost"]
    );
}

#[test]
fn lm_complete_trait_mapping_preserves_forbidden_result_tool_metadata_for_validation() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let message = Message::assistant("trait ok").with_tool_call_id("call_lookup_1");
    let lm = Arc::new(ScriptedLm::new(scripted_response(message)));
    let mut host = LmTraitHost::new(lm);

    let error = package
        .execute_plan_document(&lm_plan(), &plan_execution_context(), &mut host)
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("lm_complete result message must not carry tool_call_id or name"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn lm_complete_json_schema_executes_through_provider_neutral_lm_trait() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let lm = Arc::new(ScriptedLm::new(scripted_response(Message::assistant(
        "{\"answer\":\"ok\"}",
    ))));
    let mut host = LmTraitHost::new(Arc::clone(&lm));
    let plan = lm_json_schema_plan();

    let report = package
        .execute_plan_document(&plan, &plan_execution_context(), &mut host)
        .unwrap();

    let recorded = lm.recorded_requests.lock().unwrap();
    let request = recorded.first().expect("LM request should be recorded");
    assert!(matches!(request.output, OutputMode::JsonSchema(_)));
    assert_eq!(
        report.value()["values"]["completion"]["parsed"],
        json!({"answer": "ok"})
    );
}

#[test]
fn lm_complete_json_schema_rejects_invalid_parsed_provider_payload() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let lm = Arc::new(ScriptedLm::new(scripted_response(Message::assistant(
        "{\"answer\":42}",
    ))));
    let mut host = LmTraitHost::new(Arc::clone(&lm));

    let error = package
        .execute_plan_document(&lm_json_schema_plan(), &plan_execution_context(), &mut host)
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("lm_complete parsed result payload failed json_schema output contract"),
        "unexpected error: {error:?}"
    );
    assert_eq!(lm.recorded_requests.lock().unwrap().len(), 1);
}

#[test]
fn lm_complete_rejects_tool_result_message_id_drift_before_provider_call() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let lm = Arc::new(ScriptedLm::new(scripted_response(Message::assistant(
        "trait ok",
    ))));
    let mut host = LmTraitHost::new(Arc::clone(&lm));
    let mut plan = lm_plan();
    plan["ops"][0]["call"]["messages"][2]["tool_call_id"] = json!("call_other");

    let error = package
        .execute_plan_document(&plan, &plan_execution_context(), &mut host)
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("tool message tool_call_id must match tool_result part"),
        "unexpected error: {error:?}"
    );
    assert!(lm.recorded_requests.lock().unwrap().is_empty());
}

#[test]
fn lm_complete_rejects_typed_provider_hint_shape_drift_before_provider_call() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    for (field, replacement, expected) in [
        (
            "prompt_cache_key",
            json!(["not", "a", "string"]),
            "provider_hints.prompt_cache_key must be a string",
        ),
        (
            "store",
            json!("false"),
            "provider_hints.store must be a boolean",
        ),
        (
            "metadata",
            json!({"split": 1}),
            "provider_hints.metadata values must be strings",
        ),
    ] {
        let lm = Arc::new(ScriptedLm::new(scripted_response(Message::assistant(
            "trait ok",
        ))));
        let mut host = LmTraitHost::new(Arc::clone(&lm));
        let mut plan = lm_plan();
        plan["ops"][0]["call"]["provider_hints"][field] = replacement;

        let error = package
            .execute_plan_document(&plan, &plan_execution_context(), &mut host)
            .unwrap_err();

        assert!(
            error.to_string().contains(expected),
            "unexpected error for {field}: {error:?}"
        );
        assert!(lm.recorded_requests.lock().unwrap().is_empty());
    }
}

struct LmTraitHost {
    lm: Arc<ScriptedLm>,
}

impl LmTraitHost {
    fn new(lm: Arc<ScriptedLm>) -> Self {
        Self { lm }
    }
}

impl PlanExecutionHost for LmTraitHost {
    fn lm_complete(
        &mut self,
        request: PlanLmCompleteRequest<'_>,
    ) -> Result<PlanLmCompleteOutcome, PublicSeamError> {
        let lm_request = request.to_lm_request()?;
        let expects_json_schema = matches!(lm_request.output, OutputMode::JsonSchema(_));
        let response = block_on(self.lm.complete(lm_request)).map_err(|error| {
            PublicSeamError::InvalidPlan {
                message: error.to_string(),
            }
        })?;
        let parsed = if expects_json_schema {
            Some(parsed_lm_json_schema_payload(&response.value.assistant)?)
        } else {
            None
        };
        let mut outcome = PlanLmCompleteOutcome::from_lm_response(response, self.lm.fingerprint());
        if expects_json_schema {
            outcome = outcome.with_parsed(parsed.expect("parsed payload exists"));
        }
        Ok(outcome)
    }

    fn emit_run_event(
        &mut self,
        request: PlanEmitRunEventRequest<'_>,
    ) -> Result<PlanEmitRunEventOutcome, PublicSeamError> {
        Err(PublicSeamError::InvalidPlan {
            message: format!("unexpected write `{}`", request.name()),
        })
    }
}

fn parsed_lm_json_schema_payload(message: &Message) -> Result<Value, PublicSeamError> {
    let [MessageContentPart::Text { text }] = message.content_parts() else {
        return Err(PublicSeamError::InvalidPlan {
            message: "lm_complete json_schema response must carry assistant text".to_owned(),
        });
    };
    serde_json::from_str(text).map_err(|error| PublicSeamError::InvalidPlan {
        message: format!("lm_complete json_schema response was not JSON: {error}"),
    })
}

struct ScriptedLm {
    response: Metered<LmResponse>,
    recorded_requests: Mutex<Vec<LmRequest>>,
}

impl ScriptedLm {
    fn new(response: Metered<LmResponse>) -> Self {
        Self {
            response,
            recorded_requests: Mutex::new(Vec::new()),
        }
    }
}

impl Lm for ScriptedLm {
    fn id(&self) -> LmId {
        LmId::new("scripted")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([42; 32])
    }

    async fn complete(&self, request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
        self.recorded_requests.lock().unwrap().push(request);
        Ok(self.response.clone())
    }
}

fn scripted_response(message: Message) -> Metered<LmResponse> {
    let usage = TokenUsage {
        input_tokens: 11,
        cached_input_tokens: 3,
        output_tokens: 5,
        reasoning_tokens: 2,
    };
    Metered::new(
        LmResponse::new(message, usage).unwrap(),
        Cost {
            llm_calls: 1,
            prompt_tokens: 11,
            completion_tokens: 5,
            ..Cost::zero()
        },
    )
}

fn lm_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planlmcontract001",
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
                "idempotency_key": "lm-contract-0001",
                "call": lm_complete_call()
            }
        ],
        "return": ["completion"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn lm_json_schema_plan() -> Value {
    let mut plan = lm_plan();
    let schema = json!({
        "type": "object",
        "required": ["answer"],
        "properties": {
            "answer": {"type": "string"}
        },
        "additionalProperties": false
    });
    plan["ops"][0]["call"]["output"] = json!({
        "kind": "json_schema",
        "schema": schema,
        "schema_fingerprint": SchemaFingerprint::for_json_value(&schema).unwrap().as_str()
    });
    plan
}

fn lm_complete_call() -> Value {
    json!({
        "kind": "lm_complete",
        "purpose": "test.lm_contract",
        "model": "gpt-4.1-mini",
        "model_role": "reflector",
        "messages": [
            {
                "role": "developer",
                "content": [{"kind": "text", "text": "return the final answer"}]
            },
            {
                "role": "user",
                "content": [{"kind": "text", "text": "solve"}]
            },
            {
                "role": "tool",
                "tool_call_id": "call_lookup_1",
                "content": [
                    {
                        "kind": "tool_result",
                        "tool_call_id": "call_lookup_1",
                        "content": "{\"hint\":\"ok\"}"
                    }
                ]
            }
        ],
        "tools": [
            {
                "name": "lookup",
                "description": "look up case facts",
                "input_schema": {"type": "object"},
                "requires_capability_action": "case.read"
            }
        ],
        "sampling": {
            "temperature": 0.2,
            "max_output_tokens": 128
        },
        "output": {
            "kind": "final_message",
            "max_bytes": 256
        },
        "provider_hints": {
            "cache:key": "lm-contract",
            "prompt_cache_key": "suite-stable",
            "store": false,
            "metadata": {
                "split": "validation"
            }
        },
        "input_classes": ["public"]
    })
}

fn plan_execution_context() -> PlanExecutionContext {
    PlanExecutionContext::new(
        "fp_cap_sha256_lmcontract",
        "fp_policy_sha256_lmcontract",
        "rev_lmcontract_base",
        "2026-05-24T00:00:00Z",
        "2026-05-24T00:00:01Z",
    )
}

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
