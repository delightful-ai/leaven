use leaven_agent::{AgentSession, CommandRecord};
use leaven_kernel::{AgentSessionId, Cost, Fingerprint, Metered};
use leaven_public_seam::{
    PlanAgentRunOutcome, PlanAgentRunRequest, PlanEmitRunEventOutcome, PlanEmitRunEventRequest,
    PlanExecutionContext, PlanExecutionHost, PlanLmCompleteOutcome, PlanLmCompleteRequest,
    PlanWorkspaceMaterializeOutcome, PlanWorkspaceMaterializeRequest, PublicSeamError,
    PublicSeamPackage,
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

struct AgentSessionHost {
    session: Metered<AgentSession>,
    calls: Vec<&'static str>,
    agent_requests: Vec<leaven_agent::AgentRunRequest>,
}

impl AgentSessionHost {
    fn new(session: Metered<AgentSession>) -> Self {
        Self {
            session,
            calls: Vec::new(),
            agent_requests: Vec::new(),
        }
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
        Ok(PlanAgentRunOutcome::from_agent_session(
            self.session.clone(),
            Fingerprint::from_bytes([77; 32]),
            blob_ref("blob_agent_transcript"),
            "agentrec_completion",
        ))
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

fn blob_ref(id: &'static str) -> Value {
    json!({
        "kind": "blob_ref",
        "id": id,
        "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "bytes": 12,
        "data_classes": ["public"]
    })
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
