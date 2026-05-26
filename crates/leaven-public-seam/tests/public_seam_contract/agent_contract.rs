use crate::support::{package, plan_call_result_hash, sha256_hex};
use leaven_agent::{AgentSession, CommandRecord};
use leaven_kernel::{AgentSessionId, Cost, Fingerprint, Metered};
use leaven_public_seam::{
    AgentCommandOutputRefs, CapabilityDocument, PlanAgentRunOutcome, PlanAgentRunRequest,
    PlanEmitRunEventOutcome, PlanEmitRunEventRequest, PlanExecutionContext, PlanExecutionHost,
    PlanLmCompleteOutcome, PlanLmCompleteRequest, PlanWorkspaceMaterializeOutcome,
    PlanWorkspaceMaterializeRequest, PublicSeamError, SchemaFingerprint,
};
use leaven_workspace::{CapturedOutput, Command, CommandOutput, ExitStatus, WorkspacePath};
use serde_json::{Value, json};

#[test]
fn agent_run_can_project_provider_neutral_agent_session_into_plan_result() {
    let package = package();
    let mut host = AgentSessionHost::new(scripted_agent_session("codex"));

    let report = package
        .execute_plan_document_with_capability(
            &agent_run_workspace_plan(),
            &plan_execution_context(),
            &agent_contract_capability(),
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
        session["data_classes"],
        json!(["public", "transcript.raw", "workspace.file"])
    );
    assert_eq!(
        session["commands"][0],
        json!({
            "argv": ["codex", "exec", "--json"],
            "status": "completed",
            "receipt": "agentrec_completion",
            "stdout_ref": blob_ref_for_bytes("blob_agent_command_stdout", b"agent stdout", &["transcript.raw"]),
            "stderr_ref": blob_ref_for_bytes("blob_agent_command_stderr", b"agent stderr", &["transcript.raw"]),
            "files": {
                "reports/agent.json": blob_ref_for_bytes("blob_agent_command_report", br#"{"ok":true}"#, &["workspace.file"])
            }
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
fn agent_run_denies_no_capability_execution_before_host_effects() {
    let package = package();
    let mut host = AgentSessionHost::new(scripted_agent_session("codex"));

    let error = package
        .execute_plan_document(
            &agent_run_workspace_plan(),
            &plan_execution_context(),
            &mut host,
        )
        .unwrap_err();

    assert!(host.calls.is_empty());
    assert!(
        error
            .to_string()
            .contains("agent_run execution requires capability-authorized Plan execution"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn agent_session_projection_rejects_invalid_command_argv_during_validation() {
    let package = package();
    let mut host = AgentSessionHost::new(scripted_agent_session(""));

    let error = package
        .execute_plan_document_with_capability(
            &agent_run_workspace_plan(),
            &plan_execution_context(),
            &agent_contract_capability(),
            &mut host,
        )
        .unwrap_err();

    assert!(
        error.to_string().contains("agent_run command argv"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn agent_session_command_output_refs_reject_unbound_captured_bytes_and_files() {
    let package = package();
    for (session, fixture, expected) in [
        (
            scripted_agent_session("codex"),
            CommandOutputRefsFixture::MissingRefSet,
            "agent session has 1 commands but 0 command output ref sets",
        ),
        (
            scripted_agent_session("codex"),
            CommandOutputRefsFixture::WrongStdoutHash,
            "agent command 0 stdout blob ref sha256 does not match captured output",
        ),
        (
            scripted_agent_session("codex"),
            CommandOutputRefsFixture::WrongStdoutBytes,
            "agent command 0 stdout blob ref bytes `99` do not match captured output bytes `12`",
        ),
        (
            scripted_agent_session("codex"),
            CommandOutputRefsFixture::WrongStderrHash,
            "agent command 0 stderr blob ref sha256 does not match captured output",
        ),
        (
            scripted_agent_session("codex"),
            CommandOutputRefsFixture::MissingOutputFile,
            "agent command 0 output file `reports/agent.json` is missing a blob ref",
        ),
        (
            scripted_agent_session("codex"),
            CommandOutputRefsFixture::ExtraOutputFile,
            "agent command 0 output file `reports/extra.json` blob ref does not match a captured command output file",
        ),
        (
            scripted_agent_session("codex"),
            CommandOutputRefsFixture::WrongFileHash,
            "agent command 0 output file `reports/agent.json` blob ref sha256 does not match captured output",
        ),
        (
            scripted_agent_session_with("codex", CommandCaptureFixture::TruncatedStdout),
            CommandOutputRefsFixture::Valid,
            "agent command 0 stdout capture is truncated and cannot be bound to a blob ref",
        ),
        (
            scripted_agent_session_with("codex", CommandCaptureFixture::TruncatedStderr),
            CommandOutputRefsFixture::Valid,
            "agent command 0 stderr capture is truncated and cannot be bound to a blob ref",
        ),
    ] {
        let mut host = AgentSessionHost::new(session).with_command_output_refs(fixture);

        let error = package
            .execute_plan_document_with_capability(
                &agent_run_workspace_plan(),
                &plan_execution_context(),
                &agent_contract_capability(),
                &mut host,
            )
            .unwrap_err();

        assert!(
            error.to_string().contains(expected),
            "unexpected error for {fixture:?}: {error:?}"
        );
    }
}

#[test]
fn agent_run_json_schema_executes_through_provider_neutral_agent_contract() {
    let package = package();
    let mut host =
        AgentSessionHost::new(scripted_agent_session("codex")).with_parsed(json!({"answer": "ok"}));
    let plan = agent_run_json_schema_plan();

    let report = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &agent_contract_json_schema_capability(),
            &mut host,
        )
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
    let package = package();
    let mut host =
        AgentSessionHost::new(scripted_agent_session("codex")).with_parsed(json!({"answer": 42}));

    let error = package
        .execute_plan_document_with_capability(
            &agent_run_json_schema_plan(),
            &plan_execution_context(),
            &agent_contract_json_schema_capability(),
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
    command_output_refs: CommandOutputRefsFixture,
    calls: Vec<&'static str>,
    agent_requests: Vec<leaven_agent::AgentRunRequest>,
}

impl AgentSessionHost {
    fn new(session: Metered<AgentSession>) -> Self {
        Self {
            session,
            parsed: None,
            command_output_refs: CommandOutputRefsFixture::Valid,
            calls: Vec::new(),
            agent_requests: Vec::new(),
        }
    }

    fn with_parsed(mut self, parsed: Value) -> Self {
        self.parsed = Some(parsed);
        self
    }

    fn with_command_output_refs(mut self, fixture: CommandOutputRefsFixture) -> Self {
        self.command_output_refs = fixture;
        self
    }
}

#[derive(Clone, Copy, Debug)]
enum CommandOutputRefsFixture {
    Valid,
    MissingRefSet,
    WrongStdoutHash,
    WrongStdoutBytes,
    WrongStderrHash,
    MissingOutputFile,
    ExtraOutputFile,
    WrongFileHash,
}

#[derive(Clone, Copy, Debug)]
enum CommandCaptureFixture {
    Valid,
    TruncatedStdout,
    TruncatedStderr,
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
        assert_eq!(
            request.runtime().map(leaven_kernel::AgentRuntimeId::as_str),
            Some("codex")
        );
        self.agent_requests
            .push(request.agent_run_request().clone());
        self.calls.push("agent");
        let mut outcome = PlanAgentRunOutcome::from_agent_session_with_command_output_refs(
            self.session.clone(),
            Fingerprint::from_bytes([77; 32]),
            blob_ref("blob_agent_transcript"),
            "agentrec_completion",
            command_output_refs(self.command_output_refs),
        )?;
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
    scripted_agent_session_with(program, CommandCaptureFixture::Valid)
}

fn scripted_agent_session_with(
    program: &str,
    fixture: CommandCaptureFixture,
) -> Metered<AgentSession> {
    let mut session = AgentSession::succeeded(AgentSessionId::new());
    let mut command = Command::new(program);
    command.args = vec!["exec".to_owned(), "--json".to_owned()];
    let output_files = std::collections::BTreeMap::from([(
        WorkspacePath::new("reports/agent.json").unwrap(),
        CapturedOutput::new(br#"{"ok":true}"#.to_vec(), None),
    )]);
    session.commands.push(CommandRecord {
        command,
        output: CommandOutput {
            status: ExitStatus { code: Some(0) },
            stdout: CapturedOutput::new(
                b"agent stdout".to_vec(),
                matches!(fixture, CommandCaptureFixture::TruncatedStdout).then_some(5),
            ),
            stderr: CapturedOutput::new(
                b"agent stderr".to_vec(),
                matches!(fixture, CommandCaptureFixture::TruncatedStderr).then_some(5),
            ),
            output_files,
            duration: std::time::Duration::from_millis(10),
        },
    });
    Metered::new(session, Cost::llm_calls(1))
}

fn command_output_refs(fixture: CommandOutputRefsFixture) -> Vec<AgentCommandOutputRefs> {
    if matches!(fixture, CommandOutputRefsFixture::MissingRefSet) {
        return Vec::new();
    }
    let stdout_ref = match fixture {
        CommandOutputRefsFixture::WrongStdoutHash => json!({
            "kind": "blob_ref",
            "id": "blob_agent_command_stdout",
            "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "bytes": 12,
            "data_classes": ["transcript.raw"]
        }),
        CommandOutputRefsFixture::WrongStdoutBytes => json!({
            "kind": "blob_ref",
            "id": "blob_agent_command_stdout",
            "sha256": sha256_hex(b"agent stdout"),
            "bytes": 99,
            "data_classes": ["transcript.raw"]
        }),
        _ => blob_ref_for_bytes(
            "blob_agent_command_stdout",
            b"agent stdout",
            &["transcript.raw"],
        ),
    };
    let stderr_ref = match fixture {
        CommandOutputRefsFixture::WrongStderrHash => json!({
            "kind": "blob_ref",
            "id": "blob_agent_command_stderr",
            "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "bytes": 12,
            "data_classes": ["transcript.raw"]
        }),
        _ => blob_ref_for_bytes(
            "blob_agent_command_stderr",
            b"agent stderr",
            &["transcript.raw"],
        ),
    };
    let mut refs = AgentCommandOutputRefs::new(stdout_ref, stderr_ref);
    if !matches!(fixture, CommandOutputRefsFixture::MissingOutputFile) {
        refs = refs.with_output_file(
            WorkspacePath::new(match fixture {
                CommandOutputRefsFixture::ExtraOutputFile => "reports/extra.json",
                _ => "reports/agent.json",
            })
            .unwrap(),
            blob_ref_for_bytes(
                "blob_agent_command_report",
                match fixture {
                    CommandOutputRefsFixture::WrongFileHash => b"{\"no\":true}" as &[u8],
                    _ => br#"{"ok":true}"#,
                },
                &["workspace.file"],
            ),
        );
    }
    vec![refs]
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
    let schema = agent_output_schema();
    plan["ops"][1]["call"]["output"] = json!({
        "kind": "json_schema",
        "schema": schema,
        "schema_fingerprint": SchemaFingerprint::for_json_value(&schema).unwrap().as_str()
    });
    plan
}

fn agent_output_schema() -> Value {
    json!({
        "type": "object",
        "required": ["answer"],
        "properties": {
            "answer": {"type": "string"}
        },
        "additionalProperties": false
    })
}

fn blob_ref(id: &'static str) -> Value {
    blob_ref_for_bytes(id, b"transcript", &["transcript.raw"])
}

fn blob_ref_for_bytes(id: &'static str, bytes: &[u8], data_classes: &[&str]) -> Value {
    json!({
        "kind": "blob_ref",
        "id": id,
        "sha256": sha256_hex(bytes),
        "bytes": bytes.len(),
        "data_classes": data_classes
    })
}

fn rebind_call_result_hash(result: &mut Value, receipt_index: usize, name: &str) {
    let value = result["values"][name].clone();
    result["receipts"][receipt_index]["result_hash"] = json!(plan_call_result_hash(name, &value));
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

fn agent_contract_capability() -> CapabilityDocument {
    CapabilityDocument::from_value(agent_contract_capability_value()).unwrap()
}

fn agent_contract_json_schema_capability() -> CapabilityDocument {
    let schema = agent_output_schema();
    let schema_fingerprint = SchemaFingerprint::for_json_value(&schema).unwrap();
    let mut value = agent_contract_capability_value();
    value["grants"][1]["constraints"]["schemas"] = json!([schema_fingerprint.as_str()]);
    CapabilityDocument::from_value(value).unwrap()
}

fn agent_contract_capability_value() -> Value {
    json!({
        "schema_version": "leaven.capability.v1",
        "jti": "jti_agent_contract",
        "capability_fingerprint": "fp_cap_sha256_agentcontract",
        "policy_fingerprint": "fp_policy_sha256_agentcontract",
        "subject_fingerprint": "fp_subject_sha256_agentcontract",
        "issuer": {
            "kind": "run_engine",
            "id": "engine_local"
        },
        "subject": {
            "kind": "stage_call",
            "run": "run_agent_contract",
            "stage_call_id": "sc_agent_contract",
            "role": "proposer"
        },
        "audience": ["leaven.acp.worker"],
        "issued_at": "2026-05-24T00:00:00Z",
        "expires_at": "2026-05-24T00:20:00Z",
        "expiry_behavior": "drain_inflight_no_new_ops",
        "token_binding": {
            "kind": "opaque_lookup",
            "token_id": "ltok_agent_contract"
        },
        "revocation": {
            "mode": "issuer_epoch",
            "revocation_epoch": 7,
            "check": "on_every_request"
        },
        "renewal": {
            "mode": "renew_before_expiry",
            "max_extensions": 2,
            "max_total_lifetime_s": 3600
        },
        "grants": [
            {
                "action": "workspace.materialize",
                "resource": {
                    "candidate_ids": ["cand_planexec"]
                },
                "constraints": {
                    "workspace_ops": ["materialize"]
                }
            },
            {
                "action": "agent.run",
                "resource": {
                    "workspace_ids": ["ws_planexec_materialized"]
                },
                "constraints": {
                    "allowed_input_classes": ["public"]
                },
                "limits": {
                    "timeout_s": 30,
                    "max_usd_micro": 1000
                }
            }
        ],
        "budgets": {},
        "execution_policy": {
            "profile": "managed_sandbox",
            "network": "leaven_endpoint_only",
            "subprocess": "deny_except_sandbox_exec",
            "filesystem": "workspace_handles_only",
            "byo_effects": "forbidden"
        },
        "delegation": {
            "may_delegate": false,
            "max_depth": 0,
            "must_attenuate": true,
            "allowed_actions": []
        }
    })
}
