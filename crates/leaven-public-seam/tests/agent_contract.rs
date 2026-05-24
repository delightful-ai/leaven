use leaven_agent::{AgentSession, CommandRecord};
use leaven_kernel::{AgentSessionId, Cost, Fingerprint, Metered};
use leaven_public_seam::{
    PlanAgentRunOutcome, PlanAgentRunRequest, PlanEmitRunEventOutcome, PlanEmitRunEventRequest,
    PlanExecutionContext, PlanExecutionHost, PlanLmCompleteOutcome, PlanLmCompleteRequest,
    PlanWorkspaceMaterializeOutcome, PlanWorkspaceMaterializeRequest, PublicSeamError,
    PublicSeamPackage, SchemaFingerprint,
};
use leaven_workspace::{CapturedOutput, Command, CommandOutput, ExitStatus};
use serde_json::{Value, json};

#[test]
fn agent_run_can_project_provider_neutral_agent_session_into_plan_result() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = AgentSessionHost::new(scripted_agent_session("codex"));

    let report = package
        .execute_plan_document(
            &agent_run_workspace_plan(),
            &plan_execution_context(),
            &mut host,
        )
        .unwrap();

    assert_eq!(host.calls, vec!["workspace_materialize", "agent"]);
    let request = host.agent_requests.first().unwrap();
    assert_eq!(request.instructions.task, "Inspect the plan output.");
    assert!(!request.tool_policy.allow_shell);
    assert_eq!(request.tool_policy.allowed_tools, vec!["read_file"]);

    let session = &report.value()["values"]["completion"];
    assert_eq!(session["kind"], "agent_session");
    assert_eq!(session["status"], "completed");
    assert_eq!(session["transcript_ref"]["id"], "blob_agent_transcript");
    assert_eq!(session["data_classes"], json!(["public", "transcript.raw"]));
    assert_eq!(
        session["commands"][0],
        json!({
            "argv": ["codex", "exec", "--json"],
            "status": "completed",
            "receipt": "agentrec_completion"
        })
    );
    assert_eq!(session["cost"], json!({"lm_calls": 1}));
    assert_eq!(report.value()["receipts"][1]["cost"], session["cost"]);

    let mut missing_receipt_cost = report.value().clone();
    missing_receipt_cost["receipts"][1]
        .as_object_mut()
        .unwrap()
        .remove("cost");
    let error = package
        .validate_plan_execution_result(
            &agent_run_workspace_plan(),
            &plan_execution_context(),
            &missing_receipt_cost,
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("agent_run result value cost must match call receipt cost"),
        "unexpected error: {error:?}"
    );

    let mut missing_transcript_class = report.value().clone();
    missing_transcript_class["values"]["completion"]["data_classes"] = json!(["public"]);
    rebind_call_result_hash(&mut missing_transcript_class, 1, "completion");
    let error = package
        .validate_plan_execution_result(
            &agent_run_workspace_plan(),
            &plan_execution_context(),
            &missing_transcript_class,
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("data_classes must cover nested visibility data class `transcript.raw`"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn agent_session_projection_preserves_invalid_command_argv_for_validation() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = AgentSessionHost::new(scripted_agent_session(""));

    let error = package
        .execute_plan_document(
            &agent_run_workspace_plan(),
            &plan_execution_context(),
            &mut host,
        )
        .unwrap_err();

    assert!(
        error.to_string().contains("agent_run command argv"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn agent_run_json_schema_executes_through_provider_neutral_agent_contract() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host =
        AgentSessionHost::new(scripted_agent_session("codex")).with_parsed(json!({"answer": "ok"}));
    let plan = agent_run_json_schema_plan();

    let report = package
        .execute_plan_document(&plan, &plan_execution_context(), &mut host)
        .unwrap();

    let request = host.agent_requests.first().unwrap();
    assert!(matches!(
        request.output_contract,
        leaven_agent::OutputContract::JsonSchema { .. }
    ));
    assert_eq!(
        report.value()["values"]["completion"]["parsed"],
        json!({"answer": "ok"})
    );
}

#[test]
fn agent_run_json_schema_rejects_invalid_parsed_provider_payload() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host =
        AgentSessionHost::new(scripted_agent_session("codex")).with_parsed(json!({"answer": 42}));

    let error = package
        .execute_plan_document(
            &agent_run_json_schema_plan(),
            &plan_execution_context(),
            &mut host,
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("agent_run parsed result payload failed json_schema output contract"),
        "unexpected error: {error:?}"
    );
    assert_eq!(host.agent_requests.len(), 1);
}

struct AgentSessionHost {
    session: Metered<AgentSession>,
    parsed: Option<Value>,
    calls: Vec<&'static str>,
    agent_requests: Vec<leaven_agent::AgentRunRequest>,
}

impl AgentSessionHost {
    fn new(session: Metered<AgentSession>) -> Self {
        Self {
            session,
            parsed: None,
            calls: Vec::new(),
            agent_requests: Vec::new(),
        }
    }

    fn with_parsed(mut self, parsed: Value) -> Self {
        self.parsed = Some(parsed);
        self
    }
}

impl PlanExecutionHost for AgentSessionHost {
    fn lm_complete(
        &mut self,
        _request: PlanLmCompleteRequest<'_>,
    ) -> Result<PlanLmCompleteOutcome, PublicSeamError> {
        Err(PublicSeamError::InvalidPlan {
            message: "unexpected lm_complete".to_owned(),
        })
    }

    fn agent_run(
        &mut self,
        request: PlanAgentRunRequest<'_>,
    ) -> Result<PlanAgentRunOutcome, PublicSeamError> {
        assert_eq!(request.live_workspace()?, "ws_planexec_materialized");
        self.agent_requests.push(request.to_agent_run_request()?);
        self.calls.push("agent");
        let mut outcome = PlanAgentRunOutcome::from_agent_session(
            self.session.clone(),
            Fingerprint::from_bytes([77; 32]),
            blob_ref("blob_agent_transcript"),
            "agentrec_completion",
        );
        if let Some(parsed) = self.parsed.clone() {
            outcome = outcome.with_parsed(parsed);
        }
        Ok(outcome)
    }

    fn workspace_materialize(
        &mut self,
        request: PlanWorkspaceMaterializeRequest<'_>,
    ) -> Result<PlanWorkspaceMaterializeOutcome, PublicSeamError> {
        self.calls.push("workspace_materialize");
        Ok(PlanWorkspaceMaterializeOutcome::new(
            "ws_planexec_materialized",
            request.lifetime()?,
            "fp_runtime_sha256_workspace",
        ))
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

fn scripted_agent_session(program: &str) -> Metered<AgentSession> {
    let mut session = AgentSession::succeeded(AgentSessionId::new());
    let mut command = Command::new(program);
    command.args = vec!["exec".to_owned(), "--json".to_owned()];
    session.commands.push(CommandRecord {
        command,
        output: CommandOutput {
            status: ExitStatus { code: Some(0) },
            stdout: CapturedOutput::empty(),
            stderr: CapturedOutput::empty(),
            output_files: std::collections::BTreeMap::new(),
            duration: std::time::Duration::from_millis(10),
        },
    });
    Metered::new(session, Cost::llm_calls(1))
}

fn agent_run_workspace_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planagentcontract001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [
            {
                "kind": "call",
                "name": "workspace",
                "idempotency_key": "agent-contract-workspace-0001",
                "call": {
                    "kind": "workspace_materialize",
                    "candidate": "cand_planexec",
                    "surface": "program",
                    "mode": "copy_on_write",
                    "lifetime": "manual_release"
                }
            },
            {
                "kind": "call",
                "name": "completion",
                "deps": ["workspace"],
                "idempotency_key": "agent-contract-run-0001",
                "call": {
                    "kind": "agent_run",
                    "runtime": "codex",
                    "workspace": "ws_planexec_materialized",
                    "instructions": {
                        "system": "Stay within the workspace.",
                        "task": "Inspect the plan output."
                    },
                    "tool_policy": {
                        "allow_shell": false,
                        "allowed_tools": ["read_file"]
                    },
                    "output": {
                        "kind": "final_message",
                        "max_bytes": 1024
                    },
                    "limits": {
                        "timeout_s": 30,
                        "max_turns": 4,
                        "max_usd_micro": 1000
                    },
                    "input_classes": ["public"]
                }
            }
        ],
        "return": ["workspace", "completion"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn agent_run_json_schema_plan() -> Value {
    let mut plan = agent_run_workspace_plan();
    let schema = json!({
        "type": "object",
        "required": ["answer"],
        "properties": {
            "answer": {"type": "string"}
        },
        "additionalProperties": false
    });
    plan["ops"][1]["call"]["output"] = json!({
        "kind": "json_schema",
        "schema": schema,
        "schema_fingerprint": SchemaFingerprint::for_json_value(&schema).unwrap().as_str()
    });
    plan
}

fn blob_ref(id: &'static str) -> Value {
    json!({
        "kind": "blob_ref",
        "id": id,
        "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "bytes": 12,
        "data_classes": ["transcript.raw"]
    })
}

fn rebind_call_result_hash(result: &mut Value, receipt_index: usize, name: &str) {
    let value = result["values"][name].clone();
    result["receipts"][receipt_index]["result_hash"] = json!(format!(
        "fp_result_sha256_{}",
        jcs_canonicalize::sha256_jcs_hex(&json!({
            "schema_version": "leaven.plan_call_result.v1",
            "name": name,
            "value": value
        }))
        .unwrap()
    ));
}

fn plan_execution_context() -> PlanExecutionContext {
    PlanExecutionContext::new(
        "fp_cap_sha256_agentcontract",
        "fp_policy_sha256_agentcontract",
        "rev_agentcontract_base",
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
