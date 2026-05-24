use leaven_kernel::{Cost, Fingerprint, Metered};
use leaven_public_seam::{
    PlanEmitRunEventOutcome, PlanEmitRunEventRequest, PlanExecutionContext, PlanExecutionHost,
    PlanLmCompleteOutcome, PlanLmCompleteRequest, PlanSandboxExecOutcome, PlanSandboxExecRequest,
    PlanWorkspaceMaterializeOutcome, PlanWorkspaceMaterializeRequest, PublicSeamError,
    PublicSeamPackage,
};
use leaven_workspace::{CapturedOutput, CommandOutput, ExitStatus};
use serde_json::{Value, json};

#[test]
fn sandbox_exec_can_project_provider_neutral_command_output_into_plan_result() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = SandboxHost::new(ExitStatus { code: Some(7) });

    let report = package
        .execute_plan_document(
            &sandbox_workspace_plan(),
            &plan_execution_context(),
            &mut host,
        )
        .unwrap();

    assert_eq!(host.calls, vec!["workspace_materialize", "sandbox"]);
    let command = host.commands.first().expect("sandbox command recorded");
    assert_eq!(command.program, "python");
    assert_eq!(command.args, vec!["-c", "print('ok')"]);
    assert_eq!(
        command
            .cwd
            .as_ref()
            .map(leaven_workspace::WorkspacePath::as_str),
        Some("work")
    );
    assert_eq!(command.env["LEAVEN_CASE"], "case_1");
    assert_eq!(command.limits.timeout.unwrap().as_secs(), 1);

    let completion = &report.value()["values"]["completion"];
    assert_eq!(completion["kind"], "sandbox_exec");
    assert_eq!(completion["status"], "completed");
    assert_eq!(completion["exit_code"], 7);
    assert_eq!(completion["stdout_ref"]["id"], "blob_sandbox_stdout");
    assert_eq!(completion["stderr_ref"]["id"], "blob_sandbox_stderr");
    assert_eq!(
        completion["data_classes"],
        json!(["public", "transcript.raw", "workspace.file"])
    );
    assert_eq!(completion["cost"], json!({"sandbox_calls": 1}));
    assert_eq!(report.value()["receipts"][1]["cost"], completion["cost"]);
}

#[test]
fn sandbox_exec_command_output_projection_preserves_missing_exit_for_validation() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = SandboxHost::new(ExitStatus { code: None });

    let error = package
        .execute_plan_document(
            &sandbox_workspace_plan(),
            &plan_execution_context(),
            &mut host,
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("completed sandbox_exec result value must carry exit_code"),
        "unexpected error: {error:?}"
    );
    assert_eq!(host.commands.len(), 1);
}

#[test]
fn sandbox_exec_command_output_projection_rejects_unbound_stream_blob_refs() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = SandboxHost::new(ExitStatus { code: Some(0) }).with_corrupt_stdout_ref();

    let error = package
        .execute_plan_document(
            &sandbox_workspace_plan(),
            &plan_execution_context(),
            &mut host,
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("sandbox stdout blob ref bytes `12` do not match captured output bytes `3`"),
        "unexpected error: {error:?}"
    );
    assert_eq!(host.commands.len(), 1);
}

struct SandboxHost {
    status: ExitStatus,
    corrupt_stdout_ref: bool,
    calls: Vec<&'static str>,
    commands: Vec<leaven_workspace::Command>,
}

impl SandboxHost {
    fn new(status: ExitStatus) -> Self {
        Self {
            status,
            corrupt_stdout_ref: false,
            calls: Vec::new(),
            commands: Vec::new(),
        }
    }

    fn with_corrupt_stdout_ref(mut self) -> Self {
        self.corrupt_stdout_ref = true;
        self
    }
}

impl PlanExecutionHost for SandboxHost {
    fn lm_complete(
        &mut self,
        request: PlanLmCompleteRequest<'_>,
    ) -> Result<PlanLmCompleteOutcome, PublicSeamError> {
        Err(unexpected_operation("lm_complete", request.name()))
    }

    fn sandbox_exec(
        &mut self,
        request: PlanSandboxExecRequest<'_>,
    ) -> Result<PlanSandboxExecOutcome, PublicSeamError> {
        assert_eq!(request.live_workspace()?, "ws_sandbox_contract");
        assert_eq!(request.stream_policy(), "blob_refs_only");
        let command = request.to_workspace_command()?;
        self.commands.push(command);
        self.calls.push("sandbox");
        let stdout_ref = if self.corrupt_stdout_ref {
            blob_ref(
                "blob_sandbox_stdout",
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                12,
                &["transcript.raw"],
            )
        } else {
            blob_ref(
                "blob_sandbox_stdout",
                "dc51b8c96c2d745df3bd5590d990230a482fd247123599548e0632fdbf97fc22",
                3,
                &["transcript.raw"],
            )
        };
        PlanSandboxExecOutcome::from_command_output(
            Metered::new(
                CommandOutput {
                    status: self.status,
                    stdout: CapturedOutput::new(b"ok\n".to_vec(), None),
                    stderr: CapturedOutput::empty(),
                    duration: std::time::Duration::from_millis(10),
                },
                Cost::custom("sandbox_calls", 1.0).unwrap(),
            ),
            Fingerprint::from_bytes([88; 32]),
            stdout_ref,
            blob_ref(
                "blob_sandbox_stderr",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                0,
                &["workspace.file"],
            ),
        )
    }

    fn workspace_materialize(
        &mut self,
        request: PlanWorkspaceMaterializeRequest<'_>,
    ) -> Result<PlanWorkspaceMaterializeOutcome, PublicSeamError> {
        self.calls.push("workspace_materialize");
        Ok(PlanWorkspaceMaterializeOutcome::new(
            "ws_sandbox_contract",
            request.lifetime()?,
            "fp_runtime_sha256_workspace",
        ))
    }

    fn emit_run_event(
        &mut self,
        request: PlanEmitRunEventRequest<'_>,
    ) -> Result<PlanEmitRunEventOutcome, PublicSeamError> {
        Err(unexpected_operation("emit_run_event", request.name()))
    }
}

fn unexpected_operation(kind: &str, name: &str) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: format!("unexpected {kind} operation `{name}` in sandbox contract host"),
    }
}

fn sandbox_workspace_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plansandboxcontract001",
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
                "idempotency_key": "sandbox-contract-workspace-0001",
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
                "idempotency_key": "sandbox-contract-exec-0001",
                "call": {
                    "kind": "sandbox_exec",
                    "workspace": "ws_sandbox_contract",
                    "argv": ["python", "-c", "print('ok')"],
                    "cwd": "work",
                    "env": {"LEAVEN_CASE": "case_1"},
                    "timeout_s": 1,
                    "output": {
                        "kind": "files",
                        "paths": ["out.txt"]
                    },
                    "stream_policy": "blob_refs_only",
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

fn blob_ref(id: &'static str, sha256: &'static str, bytes: u64, data_classes: &[&str]) -> Value {
    json!({
        "kind": "blob_ref",
        "id": id,
        "sha256": sha256,
        "bytes": bytes,
        "data_classes": data_classes
    })
}

fn plan_execution_context() -> PlanExecutionContext {
    PlanExecutionContext::new(
        "fp_cap_sha256_sandboxcontract",
        "fp_policy_sha256_sandboxcontract",
        "rev_sandboxcontract_base",
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
