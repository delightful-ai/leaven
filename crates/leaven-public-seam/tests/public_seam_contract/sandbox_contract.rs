use leaven_kernel::{Cost, Fingerprint, Metered};
use leaven_public_seam::{
    CapabilityDocument, PlanEmitRunEventOutcome, PlanEmitRunEventRequest, PlanExecutionContext,
    PlanExecutionHost, PlanLmCompleteOutcome, PlanLmCompleteRequest, PlanSandboxExecOutcome,
    PlanSandboxExecRequest, PlanWorkspaceMaterializeOutcome, PlanWorkspaceMaterializeRequest,
    PublicSeamError, PublicSeamPackage,
};
use leaven_workspace::{CapturedOutput, CommandOutput, ExitStatus, WorkspacePath};
use serde_json::{Value, json};

#[test]
fn sandbox_exec_can_project_provider_neutral_command_output_into_plan_result() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = SandboxHost::new(ExitStatus { code: Some(7) });

    let report = package
        .execute_plan_document_with_capability(
            &sandbox_workspace_plan(),
            &plan_execution_context(),
            &sandbox_contract_capability(),
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
    assert_eq!(command.output_files, vec![workspace_file_path()]);
    assert_eq!(command.limits.max_output_file_bytes, Some(4096));

    let completion = &report.value()["values"]["completion"];
    assert_eq!(completion["kind"], "sandbox_exec");
    assert_eq!(completion["status"], "completed");
    assert_eq!(completion["exit_code"], 7);
    assert_eq!(completion["stdout_ref"]["id"], "blob_sandbox_stdout");
    assert_eq!(completion["stderr_ref"]["id"], "blob_sandbox_stderr");
    assert_eq!(
        completion["files"]["reports/out.txt"]["id"],
        "blob_sandbox_file"
    );
    assert_eq!(
        completion["data_classes"],
        json!(["public", "transcript.raw", "workspace.file"])
    );
    assert_eq!(completion["cost"], json!({"sandbox_calls": 1}));
    assert_eq!(report.value()["receipts"][1]["cost"], completion["cost"]);
}

#[test]
fn sandbox_exec_denies_no_capability_execution_before_host_effects() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = SandboxHost::new(ExitStatus { code: Some(0) });

    let error = package
        .execute_plan_document(
            &sandbox_workspace_plan(),
            &plan_execution_context(),
            &mut host,
        )
        .unwrap_err();

    assert!(host.calls.is_empty());
    assert!(
        error
            .to_string()
            .contains("sandbox_exec execution requires capability-authorized Plan execution"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn sandbox_exec_command_output_projection_rejects_missing_exit_during_validation() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = SandboxHost::new(ExitStatus { code: None });

    let error = package
        .execute_plan_document_with_capability(
            &sandbox_workspace_plan(),
            &plan_execution_context(),
            &sandbox_contract_capability(),
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
        .execute_plan_document_with_capability(
            &sandbox_workspace_plan(),
            &plan_execution_context(),
            &sandbox_contract_capability(),
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

#[test]
fn sandbox_exec_rejects_captured_file_refs_outside_output_contract() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = SandboxHost::new(ExitStatus { code: Some(0) }).with_wrong_output_path();

    let error = package
        .execute_plan_document_with_capability(
            &sandbox_workspace_plan(),
            &plan_execution_context(),
            &sandbox_contract_capability(),
            &mut host,
        )
        .unwrap_err();

    assert!(
        error.to_string().contains(
            "sandbox_exec output file refs must match output contract paths; missing=[\"reports/out.txt\"] extra=[\"reports/other.txt\"]"
        ),
        "unexpected error: {error:?}"
    );
    assert_eq!(host.commands.len(), 1);
}

#[test]
fn sandbox_exec_output_file_refs_reject_unbound_captured_bytes() {
    let bad_bytes = PlanSandboxExecOutcome::from_command_output_with_file_refs(
        Metered::new(
            command_output_with_file(b"actual file\n", false),
            Cost::custom("sandbox_calls", 1.0).unwrap(),
        ),
        Fingerprint::from_bytes([88; 32]),
        stdout_ref(),
        stderr_ref(),
        [(
            workspace_file_path(),
            blob_ref(
                "blob_sandbox_file",
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                11,
                &["workspace.file"],
            ),
        )],
    );
    let error = bad_bytes.expect_err("file refs must bind to captured bytes");
    assert!(
        error
            .to_string()
            .contains("sandbox output file `reports/out.txt` blob ref bytes `11` do not match captured output bytes `12`"),
        "unexpected error: {error:?}"
    );

    let bad_sha = PlanSandboxExecOutcome::from_command_output_with_file_refs(
        Metered::new(
            command_output_with_file(b"actual file\n", false),
            Cost::custom("sandbox_calls", 1.0).unwrap(),
        ),
        Fingerprint::from_bytes([88; 32]),
        stdout_ref(),
        stderr_ref(),
        [(
            workspace_file_path(),
            blob_ref(
                "blob_sandbox_file",
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                12,
                &["workspace.file"],
            ),
        )],
    );
    let error = bad_sha.expect_err("file refs must bind sha256 to captured bytes");
    assert!(
        error.to_string().contains(
            "sandbox output file `reports/out.txt` blob ref sha256 does not match captured output"
        ),
        "unexpected error: {error:?}"
    );

    let duplicate_ref = PlanSandboxExecOutcome::from_command_output_with_file_refs(
        Metered::new(
            command_output_with_file(b"artifact\n", false),
            Cost::custom("sandbox_calls", 1.0).unwrap(),
        ),
        Fingerprint::from_bytes([88; 32]),
        stdout_ref(),
        stderr_ref(),
        [
            (workspace_file_path(), sandbox_file_ref()),
            (workspace_file_path(), sandbox_file_ref()),
        ],
    );
    let error = duplicate_ref.expect_err("duplicate file refs must be rejected");
    assert!(
        error
            .to_string()
            .contains("sandbox output file `reports/out.txt` has duplicate blob refs"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn sandbox_exec_rejects_unsafe_output_contract_paths_before_host_execution() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    for path in ["/tmp/secret.txt", "../secret.txt", "", "out//secret.txt"] {
        let mut plan = sandbox_workspace_plan();
        plan["ops"][1]["call"]["output"]["paths"] = json!([path]);
        let mut host = SandboxHost::new(ExitStatus { code: Some(0) });

        let error = package
            .execute_plan_document_with_capability(
                &plan,
                &plan_execution_context(),
                &sandbox_contract_capability(),
                &mut host,
            )
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("files output path: workspace path"),
            "unexpected error for {path}: {error:?}"
        );
        assert_eq!(host.calls, vec!["workspace_materialize"]);
    }
}

#[test]
fn sandbox_exec_command_output_projection_rejects_missing_captured_output_file_refs() {
    let missing_ref = PlanSandboxExecOutcome::from_command_output(
        Metered::new(
            command_output_with_file(b"artifact\n", false),
            Cost::custom("sandbox_calls", 1.0).unwrap(),
        ),
        Fingerprint::from_bytes([88; 32]),
        stdout_ref(),
        stderr_ref(),
    );
    let error = missing_ref.expect_err("captured output files need blob refs");
    assert!(
        error
            .to_string()
            .contains("sandbox output file `reports/out.txt` is missing a blob ref"),
        "unexpected error: {error:?}"
    );

    let extra_ref = PlanSandboxExecOutcome::from_command_output_with_file_refs(
        Metered::new(
            CommandOutput::new(
                ExitStatus { code: Some(0) },
                CapturedOutput::new(b"ok\n".to_vec(), None),
                CapturedOutput::empty(),
                std::time::Duration::from_millis(10),
            ),
            Cost::custom("sandbox_calls", 1.0).unwrap(),
        ),
        Fingerprint::from_bytes([88; 32]),
        stdout_ref(),
        stderr_ref(),
        [(workspace_file_path(), sandbox_file_ref())],
    );
    let error = extra_ref.expect_err("uncaptured output file refs are not allowed");
    assert!(
        error.to_string().contains(
            "sandbox output file `reports/out.txt` blob ref does not match a captured command output file"
        ),
        "unexpected error: {error:?}"
    );

    let truncated = PlanSandboxExecOutcome::from_command_output_with_file_refs(
        Metered::new(
            command_output_with_file(b"artifact\n", true),
            Cost::custom("sandbox_calls", 1.0).unwrap(),
        ),
        Fingerprint::from_bytes([88; 32]),
        stdout_ref(),
        stderr_ref(),
        [(workspace_file_path(), sandbox_file_ref())],
    );
    let error = truncated.expect_err("truncated output files cannot be bound as complete blobs");
    assert!(
        error.to_string().contains(
            "sandbox output file `reports/out.txt` capture is truncated and cannot be bound to a blob ref"
        ),
        "unexpected error: {error:?}"
    );
}

struct SandboxHost {
    status: ExitStatus,
    corrupt_stdout_ref: bool,
    wrong_output_path: bool,
    calls: Vec<&'static str>,
    commands: Vec<leaven_workspace::Command>,
}

impl SandboxHost {
    fn new(status: ExitStatus) -> Self {
        Self {
            status,
            corrupt_stdout_ref: false,
            wrong_output_path: false,
            calls: Vec::new(),
            commands: Vec::new(),
        }
    }

    fn with_corrupt_stdout_ref(mut self) -> Self {
        self.corrupt_stdout_ref = true;
        self
    }

    fn with_wrong_output_path(mut self) -> Self {
        self.wrong_output_path = true;
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
        let command = request.workspace_command().clone();
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
        let (output_path, file_ref) = if self.wrong_output_path {
            (
                WorkspacePath::new("reports/other.txt").unwrap(),
                blob_ref(
                    "blob_sandbox_file",
                    "5b3513f580c8397212ff2c8f459c199efc0c90e4354a5f3533adf0a3fff3a530",
                    9,
                    &["workspace.file"],
                ),
            )
        } else {
            (workspace_file_path(), sandbox_file_ref())
        };
        PlanSandboxExecOutcome::from_command_output_with_file_refs(
            Metered::new(
                command_output_with_status_and_file(
                    self.status,
                    output_path.clone(),
                    b"artifact\n",
                    false,
                ),
                Cost::custom("sandbox_calls", 1.0).unwrap(),
            ),
            Fingerprint::from_bytes([88; 32]),
            stdout_ref,
            stderr_ref(),
            [(output_path, file_ref)],
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
                        "paths": ["reports/out.txt"],
                        "max_bytes": 4096
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

fn stdout_ref() -> Value {
    blob_ref(
        "blob_sandbox_stdout",
        "dc51b8c96c2d745df3bd5590d990230a482fd247123599548e0632fdbf97fc22",
        3,
        &["transcript.raw"],
    )
}

fn stderr_ref() -> Value {
    blob_ref(
        "blob_sandbox_stderr",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        0,
        &["workspace.file"],
    )
}

fn sandbox_file_ref() -> Value {
    blob_ref(
        "blob_sandbox_file",
        "5b3513f580c8397212ff2c8f459c199efc0c90e4354a5f3533adf0a3fff3a530",
        9,
        &["workspace.file"],
    )
}

fn workspace_file_path() -> WorkspacePath {
    WorkspacePath::new("reports/out.txt").unwrap()
}

fn command_output_with_file(bytes: &[u8], truncated: bool) -> CommandOutput {
    command_output_with_status_and_file(
        ExitStatus { code: Some(0) },
        workspace_file_path(),
        bytes,
        truncated,
    )
}

fn command_output_with_status_and_file(
    status: ExitStatus,
    path: WorkspacePath,
    bytes: &[u8],
    truncated: bool,
) -> CommandOutput {
    CommandOutput::new(
        status,
        CapturedOutput::new(b"ok\n".to_vec(), None),
        CapturedOutput::empty(),
        std::time::Duration::from_millis(10),
    )
    .with_output_file(
        path,
        CapturedOutput {
            bytes: bytes.to_vec(),
            truncated,
        },
    )
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

fn sandbox_contract_capability() -> CapabilityDocument {
    CapabilityDocument::from_value(json!({
        "schema_version": "leaven.capability.v1",
        "jti": "jti_sandbox_contract",
        "capability_fingerprint": "fp_cap_sha256_sandboxcontract",
        "policy_fingerprint": "fp_policy_sha256_sandboxcontract",
        "subject_fingerprint": "fp_subject_sha256_sandboxcontract",
        "issuer": {
            "kind": "run_engine",
            "id": "engine_local"
        },
        "subject": {
            "kind": "stage_call",
            "run": "run_sandbox_contract",
            "stage_call_id": "sc_sandbox_contract",
            "role": "scorer"
        },
        "audience": ["leaven.acp.worker"],
        "issued_at": "2026-05-24T00:00:00Z",
        "expires_at": "2026-05-24T00:20:00Z",
        "expiry_behavior": "drain_inflight_no_new_ops",
        "token_binding": {
            "kind": "opaque_lookup",
            "token_id": "ltok_sandbox_contract"
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
                "action": "sandbox.exec",
                "resource": {
                    "workspace_ids": ["ws_sandbox_contract"]
                },
                "constraints": {
                    "allowed_input_classes": ["public"],
                    "workspace_ops": ["exec"],
                    "allowed_commands": ["python"]
                },
                "limits": {
                    "timeout_s": 1
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
    }))
    .unwrap()
}

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
