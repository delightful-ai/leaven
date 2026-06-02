use std::{fs, os::unix::fs::PermissionsExt, path::Path, thread, time::Duration};

use leaven_acp::{
    AcpEffectHost, AcpProcessCommand, AcpStdioProcessSession, AcpTransportError,
    RejectAllEffectHost,
};
use leaven_public_seam::{AcpProfileDocument, AcpProgressDisposition, PublicSeamPackage};
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn stdio_session_starts_worker_process_with_profile_roles_and_env() {
    let package = package();
    let profile = profile(&package, 32, "pause_worker");
    let mut session = spawn_worker(
        &package,
        &profile,
        worker_script(
            r#"
read request
printf '%s\n' "$LEAVEN_TEST_RESPONSE" | sed "s/__CAPABILITY_FINGERPRINT__/$LEAVEN_CAPABILITY_FINGERPRINT/g"
"#,
        ),
        response_for(
            "leaven/lm.complete",
            "leaven-acp-0",
            extension_result(
                "leaven/lm.complete",
                lm_response_primary(),
                call_receipt("lm_complete", "lmrec_acp"),
                &["completion.raw"],
            ),
        ),
    );

    let worker_session = session.worker_session_snapshot();
    assert_eq!(worker_session.pinned_acp_version(), "0.4.0");
    assert_eq!(worker_session.transport(), "stdio_jsonrpc");
    assert_eq!(worker_session.engine_role(), "engine_client");
    assert_eq!(worker_session.worker_role(), "worker_agent");

    let response = session
        .call_extension(
            "leaven/lm.complete",
            &acp_plan_params(),
            &RejectAllEffectHost,
        )
        .unwrap();
    assert_eq!(response.method(), "leaven/lm.complete");
    assert_eq!(response.primary_kind(), "lm_response");
}

#[test]
fn stdio_session_runs_python_external_worker_program_end_to_end() {
    let package = package();
    let profile = profile(&package, 32, "pause_worker");
    let temp = TempDir::new().unwrap();
    let observed_request = temp.path().join("observed-request.json");
    let response = response_for(
        "leaven/lm.complete",
        "leaven-acp-0",
        extension_result(
            "leaven/lm.complete",
            lm_response_primary(),
            call_receipt("lm_complete", "lmrec_acp"),
            &["completion.raw"],
        ),
    );
    let script = python_worker_script(
        r#"
import json
import os
import sys

request = json.loads(sys.stdin.readline())
assert request["jsonrpc"] == "2.0"
assert request["id"] == "leaven-acp-0"
assert request["method"] == "leaven/lm.complete"
assert request["params"]["schema_version"] == "leaven.plan.v1"
assert request["params"]["ops"][0]["kind"] == "let"
assert os.environ["LEAVEN_CAPABILITY_TOKEN"] == "secret-token"
assert os.environ["LEAVEN_CAPABILITY_FINGERPRINT"] == "fp_cap_sha256_acp"
assert os.environ["LEAVEN_ENDPOINT"] == "stdio://worker/session"

with open(os.environ["LEAVEN_TEST_OBSERVED_REQUEST"], "w", encoding="utf-8") as handle:
    json.dump(request, handle, sort_keys=True)

print(json.dumps({
    "jsonrpc": "2.0",
    "method": "session/update",
    "params": {"message": "python worker accepted plan", "priority": "critical"},
}), flush=True)

response = json.loads(os.environ["LEAVEN_TEST_RESPONSE"])
response["result"]["capability_fingerprint"] = os.environ["LEAVEN_CAPABILITY_FINGERPRINT"]
print(json.dumps(response, sort_keys=True), flush=True)
"#,
    );
    let script_path = script.path().join("worker.py");
    let mut session = AcpStdioProcessSession::spawn(
        package,
        profile,
        AcpProcessCommand::new("python3")
            .arg(script_path.to_str().unwrap())
            .env("LEAVEN_TEST_RESPONSE", response)
            .env(
                "LEAVEN_TEST_OBSERVED_REQUEST",
                observed_request.to_str().unwrap(),
            ),
        "secret-token",
        "stdio://worker/session",
        "fp_cap_sha256_acp",
    )
    .unwrap();

    let result = session
        .call_extension(
            "leaven/lm.complete",
            &acp_plan_params(),
            &RejectAllEffectHost,
        )
        .unwrap();
    assert_eq!(result.method(), "leaven/lm.complete");
    assert_eq!(result.primary_kind(), "lm_response");
    assert_eq!(
        session
            .worker_session_snapshot()
            .lifecycle()
            .inflight_updates(),
        1
    );

    let observed: Value = serde_json::from_str(&fs::read_to_string(observed_request).unwrap())
        .expect("python worker wrote observed request");
    assert_eq!(observed["method"], json!("leaven/lm.complete"));
    assert_eq!(observed["params"]["return"], json!(["input"]));
    std::mem::forget(script);
    std::mem::forget(temp);
}

/// Host effect handler that records the worker-initiated `leaven/lm.complete`
/// params and answers with a valid `lm_response` extension result. The
/// capability fingerprint is intentionally omitted so the transport stamps the
/// launched session fingerprint on the reply.
struct RecordingLmCompleteHost {
    observed_params: std::sync::Mutex<Option<Value>>,
}

impl RecordingLmCompleteHost {
    fn new() -> Self {
        Self {
            observed_params: std::sync::Mutex::new(None),
        }
    }
}

impl AcpEffectHost for RecordingLmCompleteHost {
    fn lm_complete(&self, params: &Value) -> Result<Value, AcpTransportError> {
        *self.observed_params.lock().unwrap() = Some(params.clone());
        let mut result = extension_result(
            "leaven/lm.complete",
            lm_response_primary(),
            call_receipt("lm_complete", "lmrec_acp"),
            &["completion.raw"],
        );
        result
            .as_object_mut()
            .unwrap()
            .remove("capability_fingerprint");
        Ok(result)
    }
}

#[test]
fn stdio_session_services_python_worker_initiated_lm_complete_request() {
    // The inverse of `stdio_session_runs_python_external_worker_program_end_to_end`:
    // the worker is the ACP agent and *initiates* `leaven/lm.complete` back into
    // the engine, and the host services the inbound request and responds.
    let package = package();
    let profile = profile(&package, 32, "pause_worker");
    let temp = TempDir::new().unwrap();
    let observed_response = temp.path().join("observed-response.json");
    let script = python_worker_script(
        r#"
import json
import os
import sys

# Worker-side lifecycle progress precedes the worker-initiated request.
print(json.dumps({
    "jsonrpc": "2.0",
    "method": "session/update",
    "params": {"message": "python worker starting rollout", "priority": "critical"},
}), flush=True)

# The worker is the ACP agent: it initiates leaven/lm.complete back into the host.
request = {
    "jsonrpc": "2.0",
    "id": "worker-req-7",
    "method": "leaven/lm.complete",
    "params": {
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_worker_lm_complete",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "dry_run"},
        "ops": [{
            "kind": "let",
            "name": "prompt",
            "expr": {
                "kind": "literal",
                "value": "what is 2 + 2?",
                "data_classes": ["public"],
            },
        }],
        "return": ["prompt"],
        "commit": {"kind": "no_graph_writes"},
    },
}
print(json.dumps(request, sort_keys=True), flush=True)

# The host services the inbound request and replies under the worker's id.
response = json.loads(sys.stdin.readline())
assert response["jsonrpc"] == "2.0", response
assert response["id"] == "worker-req-7", response
assert "result" in response, response
result = response["result"]
assert result["method"] == "leaven/lm.complete", result
assert result["primary"]["kind"] == "lm_response", result
# The transport stamped the launched session fingerprint onto the reply.
assert result["capability_fingerprint"] == os.environ["LEAVEN_CAPABILITY_FINGERPRINT"], result

with open(os.environ["LEAVEN_TEST_OBSERVED_RESPONSE"], "w", encoding="utf-8") as handle:
    json.dump(response, handle, sort_keys=True)
"#,
    );
    let script_path = script.path().join("worker.py");
    let mut session = AcpStdioProcessSession::spawn(
        package,
        profile,
        AcpProcessCommand::new("python3")
            .arg(script_path.to_str().unwrap())
            .env(
                "LEAVEN_TEST_OBSERVED_RESPONSE",
                observed_response.to_str().unwrap(),
            ),
        "secret-token",
        "stdio://worker/session",
        "fp_cap_sha256_acp",
    )
    .unwrap();

    let host = RecordingLmCompleteHost::new();
    let request = session.serve_next_inbound_request(&host).unwrap();
    assert_eq!(request.id(), "worker-req-7");
    assert_eq!(request.method(), "leaven/lm.complete");
    // The worker's session/update preceding the request was applied as lifecycle
    // control, not confused with the inbound request.
    assert_eq!(
        session
            .worker_session_snapshot()
            .lifecycle()
            .inflight_updates(),
        1
    );

    let observed_params = host
        .observed_params
        .lock()
        .unwrap()
        .clone()
        .expect("host received the worker's Plan IR params");
    assert_eq!(observed_params["plan_id"], json!("plan_worker_lm_complete"));
    assert_eq!(observed_params["return"], json!(["prompt"]));

    assert!(session.wait_for_exit().unwrap().success());
    let observed: Value = serde_json::from_str(&fs::read_to_string(observed_response).unwrap())
        .expect("python worker wrote observed response");
    assert_eq!(observed["id"], json!("worker-req-7"));
    assert_eq!(observed["result"]["method"], json!("leaven/lm.complete"));
    assert_eq!(observed["result"]["primary"]["kind"], json!("lm_response"));
    assert_eq!(
        observed["result"]["capability_fingerprint"],
        json!("fp_cap_sha256_acp")
    );
    std::mem::forget(script);
    std::mem::forget(temp);
}

#[test]
#[allow(clippy::too_many_lines)]
fn stdio_session_runs_python_external_worker_program_across_v1_method_families() {
    let package = package();
    let profile = profile(&package, 32, "pause_worker");
    let temp = TempDir::new().unwrap();
    let observed_requests = temp.path().join("observed-requests.json");
    let program_cases = extension_result_cases();
    let expected: Vec<(&str, &str)> = program_cases
        .iter()
        .map(|case| (case.method, case.primary_kind))
        .collect();
    let script = python_worker_script(
        r#"
import hashlib
import json
import os
import sys

EXPECTED_METHODS = [
    "leaven/graph.query",
    "leaven/case.load",
    "leaven/case.input",
    "leaven/case.target",
    "leaven/case.metadata",
    "leaven/human.review",
    "leaven/event.emit",
    "leaven/workspace.materialize",
    "leaven/workspace.release",
    "leaven/workspace.snapshot",
    "leaven/workspace.list",
    "leaven/workspace.read_file",
    "leaven/workspace.stat",
    "leaven/workspace.digest",
    "leaven/workspace.git_log",
    "leaven/workspace.git_diff",
    "leaven/workspace.git_status",
    "leaven/workspace.capture_artifacts",
    "leaven/lm.complete",
    "leaven/agent.run",
    "leaven/sandbox.exec",
    "leaven/proposal.submit_batch",
    "leaven/proposal.apply",
    "leaven/assessment.submit",
    "leaven/evaluation.request",
]

GENERIC_EXTENSION_OPS = {
    "leaven/graph.query": ("graph.query", "qrec_graph", "query", ["public"]),
    "leaven/case.load": ("case.load", "qrec_case_load", "query", ["public"]),
    "leaven/case.input": ("case.input", "qrec_case_input", "query", ["public"]),
    "leaven/case.target": ("case.target", "qrec_case_target", "query", ["public"]),
    "leaven/case.metadata": ("case.metadata", "qrec_case_metadata", "query", ["public"]),
    "leaven/human.review": ("human.review", "humanrec_acp", "call", ["public"]),
    "leaven/event.emit": ("event.emit", "wrec_event_emit", "write", ["public"]),
}

WORKSPACE_METHODS = {
    "leaven/workspace.materialize": ("workspace_handle", "wrec_materialize", "call", ["workspace.file"]),
    "leaven/workspace.release": ("workspace_handle", "wrec_release", "call", ["workspace.file"]),
    "leaven/workspace.snapshot": ("workspace_snapshot", "qrec_workspace_snapshot", "query", ["workspace.file"]),
    "leaven/workspace.list": ("workspace_listing", "qrec_workspace_list", "query", ["workspace.file"]),
    "leaven/workspace.read_file": ("workspace_file", "qrec_workspace_file", "query", ["workspace.file"]),
    "leaven/workspace.stat": ("workspace_listing", "qrec_workspace_stat", "query", ["workspace.file"]),
    "leaven/workspace.digest": ("workspace_snapshot", "qrec_workspace_digest", "query", ["workspace.file"]),
    "leaven/workspace.git_log": ("workspace_diff", "qrec_workspace_git_log", "query", ["workspace.file"]),
    "leaven/workspace.git_diff": ("workspace_diff", "qrec_workspace_git_diff", "query", ["workspace.file"]),
    "leaven/workspace.git_status": ("workspace_diff", "qrec_workspace_git_status", "query", ["workspace.file"]),
    "leaven/workspace.capture_artifacts": ("workspace_listing", "qrec_workspace_capture", "query", ["workspace.file"]),
}

EFFECT_AND_WRITE_METHODS = {
    "leaven/lm.complete": ("lm_response", "lmrec_acp", "call", ["completion.raw"]),
    "leaven/agent.run": ("agent_session", "agentrec_acp", "call", ["public", "transcript.raw"]),
    "leaven/sandbox.exec": ("sandbox_exec", "execrec_acp", "call", ["public"]),
    "leaven/proposal.submit_batch": ("proposal_batch_receipt", "wrec_proposal_submit", "write", ["public"]),
    "leaven/proposal.apply": ("apply_receipt", "wrec_proposal_apply", "write", ["public"]),
    "leaven/assessment.submit": ("assessment_batch_receipt", "wrec_assessment_submit", "write", ["public"]),
    "leaven/evaluation.request": ("evaluation_request_receipt", "wrec_evaluation_request", "write", ["public"]),
}

observed = []

def canonical_hash(prefix, value):
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return prefix + hashlib.sha256(encoded).hexdigest()

def blob_ref(blob_id, data_classes):
    return {
        "kind": "blob_ref",
        "id": blob_id,
        "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "bytes": 32,
        "data_classes": data_classes,
    }

def query_receipt(receipt):
    return {
        "kind": "query",
        "receipt": receipt,
        "op_var": "workspace_read",
        "started_at": "2026-05-23T00:00:00Z",
        "completed_at": "2026-05-23T00:00:01Z",
        "op_hash": "fp_query_sha256_acp",
        "result_hash": "fp_result_sha256_acp",
        "graph_revision": "rev_acp",
        "status": "succeeded",
        "read_scope_fingerprint": "fp_scope_sha256_acp",
        "projection_fingerprint": "fp_projection_sha256_acp",
    }

def call_receipt(call_kind, receipt):
    value = {
        "kind": "call",
        "receipt": receipt,
        "op_var": "worker_call",
        "started_at": "2026-05-23T00:00:00Z",
        "completed_at": "2026-05-23T00:00:01Z",
        "call_kind": call_kind,
        "request_hash": "fp_request_sha256_acp",
        "result_hash": "fp_result_sha256_acp",
        "runtime_fingerprint": "fp_runtime_sha256_acp",
        "status": "succeeded",
    }
    if call_kind == "lm_complete":
        value["cost"] = {"usd_micro": 42, "lm_calls": 1}
    if call_kind == "agent_run":
        value["cost"] = {"usd_micro": 1000, "agent_calls": 1}
    if call_kind == "sandbox_exec":
        value["cost"] = {"usd_micro": 10, "sandbox_calls": 1}
    return value

def write_receipt(write_kind, receipt):
    value = {
        "kind": "write",
        "receipt": receipt,
        "op_var": "primary",
        "started_at": "2026-05-23T00:00:00Z",
        "completed_at": "2026-05-23T00:00:01Z",
        "write_kind": write_kind,
        "request_hash": "fp_request_sha256_acp",
        "result_hash": "fp_result_sha256_acp",
        "base_revision": "rev_acp",
        "committed_revision": "rev_acp",
        "status": "succeeded",
    }
    if write_kind == "submit_proposal_batch":
        value["proposal_batch_id"] = "pb_acp"
        value["proposal_ids"] = ["prop_acp"]
    if write_kind == "apply_proposal_batch":
        value["created_candidates"] = ["cand_acp_created"]
    if write_kind == "submit_assessments":
        value["evaluation_request_id"] = "evalreq_acp"
        value["assessment_ids"] = ["assess_acp"]
        value["request_hash"] = canonical_hash("fp_request_sha256_", {
            "schema_version": "leaven.submit_assessments_request.v1",
            "evaluation_request_id": "evalreq_acp",
            "assessment_ids": ["assess_acp"],
        })
    if write_kind == "request_evaluation":
        value["evaluation_request_id"] = "evalreq_acp"
    if write_kind == "emit_run_event":
        value["event_id"] = "event_acp"
    return value

def extension_primary(op):
    return {
        "kind": "extension",
        "namespace": "leaven",
        "op": op,
        "schema_fingerprint": "fp_schema_sha256_acpextension",
        "payload": {"status": "ok"},
    }

def workspace_primary(kind, receipt):
    if kind == "workspace_handle":
        return {
            "kind": "workspace_handle",
            "workspace": "ws_acp",
            "lifetime": "stage_call",
            "released": receipt == "wrec_release",
            "graph_revision": "rev_acp",
            "data_classes": ["workspace.file"],
            "replayability": "fully_managed",
            "receipt": receipt,
        }
    if kind == "workspace_snapshot":
        return {
            "kind": "workspace_snapshot",
            "workspace": "ws_acp",
            "digest": "sha256:workspace",
            "graph_revision": "rev_acp",
            "data_classes": ["workspace.file"],
            "replayability": "pure_read",
        }
    if kind == "workspace_listing":
        return {
            "kind": "workspace_listing",
            "entries": [{"path": "src/lib.rs", "kind": "file", "data_classes": ["workspace.file"]}],
            "graph_revision": "rev_acp",
            "data_classes": ["workspace.file"],
            "replayability": "pure_read",
        }
    if kind == "workspace_file":
        return {
            "kind": "workspace_file",
            "path": "src/lib.rs",
            "content": "pub fn demo() {}",
            "graph_revision": "rev_acp",
            "data_classes": ["workspace.file"],
            "replayability": "pure_read",
            "receipt": "qrec_workspace_file",
        }
    if kind == "workspace_diff":
        return {
            "kind": "workspace_diff",
            "text": " M src/lib.rs",
            "graph_revision": "rev_acp",
            "data_classes": ["workspace.file"],
            "replayability": "pure_read",
        }
    raise AssertionError(f"unknown workspace primary {kind}")

def effect_or_write_primary(kind):
    if kind == "lm_response":
        return {
            "kind": "lm_response",
            "message": {"role": "assistant", "content": [{"kind": "text", "text": "ok"}]},
            "graph_revision": "rev_acp",
            "cost": {"usd_micro": 42, "lm_calls": 1},
            "data_classes": ["completion.raw"],
            "replayability": "fully_managed",
            "receipt": "lmrec_acp",
        }
    if kind == "agent_session":
        return {
            "kind": "agent_session",
            "status": "completed",
            "transcript_ref": blob_ref("blob_agent_transcript", ["transcript.raw"]),
            "commands": [{
                "argv": ["codex"],
                "status": "completed",
                "receipt": "agentrec_acp",
                "stdout_ref": blob_ref("blob_agent_stdout", ["transcript.raw"]),
                "stderr_ref": blob_ref("blob_agent_stderr", ["transcript.raw"]),
            }],
            "cost": {"usd_micro": 1000, "agent_calls": 1},
            "graph_revision": "rev_acp",
            "data_classes": ["public", "transcript.raw"],
            "replayability": "fully_managed",
            "receipt": "agentrec_acp",
        }
    if kind == "sandbox_exec":
        return {
            "kind": "sandbox_exec",
            "status": "completed",
            "exit_code": 0,
            "cost": {"usd_micro": 10, "sandbox_calls": 1},
            "stdout_ref": blob_ref("blob_sandbox_stdout", ["public"]),
            "stderr_ref": blob_ref("blob_sandbox_stderr", ["public"]),
            "graph_revision": "rev_acp",
            "data_classes": ["public"],
            "replayability": "fully_managed",
            "receipt": "execrec_acp",
        }
    if kind == "proposal_batch_receipt":
        return {
            "kind": "proposal_batch_receipt",
            "batch_id": "pb_acp",
            "proposal_ids": ["prop_acp"],
            "status": "committed",
            "graph_revision": "rev_acp",
            "data_classes": ["public"],
            "replayability": "fully_managed",
            "receipt": "wrec_proposal_submit",
        }
    if kind == "apply_receipt":
        return {
            "kind": "apply_receipt",
            "created_candidates": ["cand_acp_created"],
            "status": "committed",
            "graph_revision": "rev_acp",
            "data_classes": ["public"],
            "replayability": "fully_managed",
            "receipt": "wrec_proposal_apply",
        }
    if kind == "assessment_batch_receipt":
        return {
            "kind": "assessment_batch_receipt",
            "evaluation_request_id": "evalreq_acp",
            "assessment_ids": ["assess_acp"],
            "per_assessment": [{"assessment": "assess_acp", "replayability": "fully_managed"}],
            "status": "committed",
            "graph_revision": "rev_acp",
            "data_classes": ["public"],
            "replayability": "fully_managed",
            "receipt": "wrec_assessment_submit",
        }
    if kind == "evaluation_request_receipt":
        return {
            "kind": "evaluation_request_receipt",
            "evaluation_request_id": "evalreq_acp",
            "status": "recorded",
            "graph_revision": "rev_acp",
            "data_classes": ["public"],
            "replayability": "fully_managed",
            "receipt": "wrec_evaluation_request",
        }
    raise AssertionError(f"unknown effect/write primary {kind}")

def make_result(method):
    if method in GENERIC_EXTENSION_OPS:
        op, receipt_id, receipt_kind, data_classes = GENERIC_EXTENSION_OPS[method]
        primary = extension_primary(op)
        if receipt_kind == "query":
            receipt = query_receipt(receipt_id)
        elif receipt_kind == "call":
            receipt = call_receipt("human_review", receipt_id)
        else:
            receipt = write_receipt("emit_run_event", receipt_id)
    elif method in WORKSPACE_METHODS:
        primary_kind, receipt_id, receipt_kind, data_classes = WORKSPACE_METHODS[method]
        primary = workspace_primary(primary_kind, receipt_id)
        if receipt_kind == "query":
            receipt = query_receipt(receipt_id)
        else:
            call_kind = "workspace_materialize" if method.endswith("materialize") else "workspace_release"
            receipt = call_receipt(call_kind, receipt_id)
    else:
        primary_kind, receipt_id, receipt_kind, data_classes = EFFECT_AND_WRITE_METHODS[method]
        primary = effect_or_write_primary(primary_kind)
        if receipt_kind == "call":
            call_kind = {
                "leaven/lm.complete": "lm_complete",
                "leaven/agent.run": "agent_run",
                "leaven/sandbox.exec": "sandbox_exec",
            }[method]
            receipt = call_receipt(call_kind, receipt_id)
        else:
            write_kind = {
                "leaven/proposal.submit_batch": "submit_proposal_batch",
                "leaven/proposal.apply": "apply_proposal_batch",
                "leaven/assessment.submit": "submit_assessments",
                "leaven/evaluation.request": "request_evaluation",
            }[method]
            receipt = write_receipt(write_kind, receipt_id)
    schema_version = {
        "query": "leaven.plan_query_result.v1",
        "call": "leaven.plan_call_result.v1",
        "write": "leaven.plan_write_result.v1",
    }[receipt["kind"]]
    op_name = receipt.get("op_var", "primary")
    receipt["result_hash"] = canonical_hash("fp_result_sha256_", {
        "schema_version": schema_version,
        "name": op_name,
        "value": primary,
    })
    return {
        "method": method,
        "redactions": [],
        "capability_fingerprint": os.environ["LEAVEN_CAPABILITY_FINGERPRINT"],
        "data_classes": data_classes,
        "primary": primary,
        "receipts": [receipt],
    }

for index, expected_method in enumerate(EXPECTED_METHODS):
    request = json.loads(sys.stdin.readline())
    assert request["jsonrpc"] == "2.0"
    assert request["id"] == f"leaven-acp-{index}"
    assert request["method"] == expected_method
    assert request["params"]["schema_version"] == "leaven.plan.v1"
    assert request["params"]["commit"]["kind"] == "no_graph_writes"
    assert request["params"]["ops"][0]["expr"]["value"] == expected_method
    assert "mcp" not in request["method"]
    assert os.environ["LEAVEN_CAPABILITY_TOKEN"] == "secret-token"
    assert os.environ["LEAVEN_CAPABILITY_FINGERPRINT"] == "fp_cap_sha256_acp"
    observed.append({
        "id": request["id"],
        "method": request["method"],
        "return": request["params"]["return"],
    })
    print(json.dumps({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "message": f"python worker completed {request['method']}",
            "priority": "critical",
        },
    }), flush=True)
    result = make_result(expected_method)
    assert result["method"] == request["method"]
    print(json.dumps({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": result,
    }, sort_keys=True), flush=True)

with open(os.environ["LEAVEN_TEST_OBSERVED_REQUESTS"], "w", encoding="utf-8") as handle:
    json.dump(observed, handle, sort_keys=True)
"#,
    );
    let script_path = script.path().join("worker.py");
    let mut session = AcpStdioProcessSession::spawn(
        package,
        profile,
        AcpProcessCommand::new("python3")
            .arg(script_path.to_str().unwrap())
            .env(
                "LEAVEN_TEST_OBSERVED_REQUESTS",
                observed_requests.to_str().unwrap(),
            ),
        "secret-token",
        "stdio://worker/session",
        "fp_cap_sha256_acp",
    )
    .unwrap();

    for (index, (method, primary_kind)) in expected.iter().enumerate() {
        let response = session
            .call_extension(
                method,
                &acp_plan_params_for_method(method),
                &RejectAllEffectHost,
            )
            .unwrap_or_else(|error| panic!("program call {index} {method} failed: {error:?}"));
        assert_eq!(response.id(), format!("leaven-acp-{index}"));
        assert_eq!(response.method(), *method);
        assert_eq!(response.primary_kind(), *primary_kind);
    }
    assert_eq!(
        session
            .worker_session_snapshot()
            .lifecycle()
            .inflight_updates(),
        expected.len()
    );

    let observed: Vec<Value> =
        serde_json::from_str(&fs::read_to_string(observed_requests).unwrap())
            .expect("python worker wrote observed request sequence");
    assert_eq!(observed.len(), expected.len());
    for (index, (method, _)) in expected.iter().enumerate() {
        assert_eq!(observed[index]["id"], json!(format!("leaven-acp-{index}")));
        assert_eq!(observed[index]["method"], json!(*method));
        assert_eq!(observed[index]["return"], json!(["input"]));
    }
    std::mem::forget(script);
    std::mem::forget(temp);
}

/// Host effect handler that answers `leaven/lm.complete` while asserting a
/// capability fingerprint from a *different* session. The transport must refuse
/// it instead of letting a host answer on behalf of another session.
struct ForeignFingerprintLmCompleteHost;

impl AcpEffectHost for ForeignFingerprintLmCompleteHost {
    fn lm_complete(&self, _params: &Value) -> Result<Value, AcpTransportError> {
        let mut result = extension_result(
            "leaven/lm.complete",
            lm_response_primary(),
            call_receipt("lm_complete", "lmrec_acp"),
            &["completion.raw"],
        );
        result["capability_fingerprint"] = json!("fp_cap_sha256_other_session");
        Ok(result)
    }
}

#[test]
fn stdio_session_rejects_inbound_host_result_with_foreign_capability_fingerprint() {
    let package = package();
    let profile = profile(&package, 32, "pause_worker");
    let script = python_worker_script(
        r#"
import json
import sys

print(json.dumps({
    "jsonrpc": "2.0",
    "id": "worker-req-fp",
    "method": "leaven/lm.complete",
    "params": {
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_worker_lm_complete",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "dry_run"},
        "ops": [{
            "kind": "let",
            "name": "prompt",
            "expr": {"kind": "literal", "value": "x", "data_classes": ["public"]},
        }],
        "return": ["prompt"],
        "commit": {"kind": "no_graph_writes"},
    },
}), flush=True)
"#,
    );
    let script_path = script.path().join("worker.py");
    let mut session = AcpStdioProcessSession::spawn(
        package,
        profile,
        AcpProcessCommand::new("python3").arg(script_path.to_str().unwrap()),
        "secret-token",
        "stdio://worker/session",
        "fp_cap_sha256_acp",
    )
    .unwrap();
    assert!(matches!(
        session.serve_next_inbound_request(&ForeignFingerprintLmCompleteHost),
        Err(AcpTransportError::EffectFingerprintMismatch { actual, .. })
            if actual == "fp_cap_sha256_other_session"
    ));
    std::mem::forget(script);
}

#[test]
fn stdio_session_rejects_private_and_mcp_inbound_worker_requests() {
    // The no-private, no-MCP guarantee holds in the inbound direction too: a
    // worker that initiates a non-Leaven or MCP method is rejected before any
    // host lowering runs, mirroring the host->worker rejection.
    let package = package();
    let profile = profile(&package, 32, "pause_worker");
    for method in ["private/run_lm", "leaven/mcp.bridge"] {
        let script = python_worker_script(&format!(
            r#"
import json
import sys

print(json.dumps({{
    "jsonrpc": "2.0",
    "id": "worker-req-private",
    "method": "{method}",
    "params": {{
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_private_inbound",
        "consistency": {{"kind": "latest_at_start"}},
        "mode": {{"kind": "dry_run"}},
        "ops": [{{
            "kind": "let",
            "name": "prompt",
            "expr": {{"kind": "literal", "value": "x", "data_classes": ["public"]}},
        }}],
        "return": ["prompt"],
        "commit": {{"kind": "no_graph_writes"}},
    }},
}}), flush=True)
"#,
        ));
        let script_path = script.path().join("worker.py");
        let mut session = AcpStdioProcessSession::spawn(
            package.clone(),
            profile.clone(),
            AcpProcessCommand::new("python3").arg(script_path.to_str().unwrap()),
            "secret-token",
            "stdio://worker/session",
            "fp_cap_sha256_acp",
        )
        .unwrap();
        assert!(
            matches!(
                session.serve_next_inbound_request(&RejectAllEffectHost),
                Err(AcpTransportError::PublicSeam(_))
            ),
            "inbound method `{method}` must be rejected by the locked profile"
        );
        std::mem::forget(script);
    }
}

#[test]
fn stdio_session_rejects_external_worker_program_bare_payload_mid_sequence() {
    let package = package();
    let profile = profile(&package, 32, "pause_worker");
    let first_response = response_for(
        "leaven/workspace.materialize",
        "leaven-acp-0",
        extension_result(
            "leaven/workspace.materialize",
            workspace_handle_primary(false, "wrec_materialize"),
            call_receipt("workspace_materialize", "wrec_materialize"),
            &["workspace.file"],
        ),
    );
    let script = python_worker_script(
        r#"
import json
import os
import sys

first = json.loads(sys.stdin.readline())
assert first["method"] == "leaven/workspace.materialize"
response = json.loads(os.environ["LEAVEN_TEST_FIRST_RESPONSE"])
response["result"]["capability_fingerprint"] = os.environ["LEAVEN_CAPABILITY_FINGERPRINT"]
print(json.dumps(response, sort_keys=True), flush=True)

second = json.loads(sys.stdin.readline())
assert second["method"] == "leaven/lm.complete"
print(json.dumps({
    "jsonrpc": "2.0",
    "id": second["id"],
    "result": {
        "message": {
            "role": "assistant",
            "content": [{"kind": "text", "text": "bare payload without ACP receipts"}],
        },
    },
}, sort_keys=True), flush=True)
"#,
    );
    let script_path = script.path().join("worker.py");
    let mut session = AcpStdioProcessSession::spawn(
        package,
        profile,
        AcpProcessCommand::new("python3")
            .arg(script_path.to_str().unwrap())
            .env("LEAVEN_TEST_FIRST_RESPONSE", first_response),
        "secret-token",
        "stdio://worker/session",
        "fp_cap_sha256_acp",
    )
    .unwrap();

    let response = session
        .call_extension(
            "leaven/workspace.materialize",
            &acp_plan_params(),
            &RejectAllEffectHost,
        )
        .unwrap();
    assert_eq!(response.primary_kind(), "workspace_handle");
    assert!(matches!(
        session.call_extension(
            "leaven/lm.complete",
            &acp_plan_params(),
            &RejectAllEffectHost
        ),
        Err(AcpTransportError::PublicSeam(_))
    ));
    std::mem::forget(script);
}

#[test]
fn stdio_session_launch_respects_worker_current_dir() {
    let package = package();
    let profile = profile(&package, 32, "pause_worker");
    let script = worker_script(
        r#"
read request
printf '%s\n' "$(pwd)" > "$LEAVEN_TEST_PWD_LOG"
printf '%s\n' "$LEAVEN_TEST_RESPONSE" | sed "s/__CAPABILITY_FINGERPRINT__/$LEAVEN_CAPABILITY_FINGERPRINT/g"
"#,
    );
    let pwd_log = script.path().join("pwd.log");
    let script_path = script.path().join("worker.sh");
    let current_dir = script.path().join("worker-cwd");
    fs::create_dir(&current_dir).unwrap();
    let command = AcpProcessCommand::new("/bin/sh")
        .arg(script_path.to_str().unwrap())
        .current_dir(&current_dir)
        .env("LEAVEN_TEST_PWD_LOG", pwd_log.to_str().unwrap())
        .env(
            "LEAVEN_TEST_RESPONSE",
            response_for(
                "leaven/lm.complete",
                "leaven-acp-0",
                extension_result(
                    "leaven/lm.complete",
                    lm_response_primary(),
                    call_receipt("lm_complete", "lmrec_acp"),
                    &["completion.raw"],
                ),
            ),
        );
    let mut session = AcpStdioProcessSession::spawn(
        package,
        profile,
        command,
        "secret-token",
        "stdio://worker/session",
        "fp_cap_sha256_acp",
    )
    .unwrap();

    session
        .call_extension(
            "leaven/lm.complete",
            &acp_plan_params(),
            &RejectAllEffectHost,
        )
        .unwrap();
    assert_eq!(
        fs::canonicalize(fs::read_to_string(&pwd_log).unwrap().trim()).unwrap(),
        fs::canonicalize(&current_dir).unwrap()
    );
    std::mem::forget(script);
}

#[test]
fn stdio_session_rejects_private_mcp_or_bare_process_protocols() {
    let package = package();
    let profile = profile(&package, 32, "pause_worker");
    let mut private_session = spawn_worker(
        &package,
        &profile,
        worker_script("read request\n"),
        response_for(
            "leaven/lm.complete",
            "leaven-acp-0",
            extension_result(
                "leaven/lm.complete",
                lm_response_primary(),
                call_receipt("lm_complete", "lmrec_acp"),
                &["completion.raw"],
            ),
        ),
    );
    assert!(matches!(
        private_session.call_extension("private/run_lm", &acp_plan_params(), &RejectAllEffectHost),
        Err(AcpTransportError::PublicSeam(_))
    ));
    assert!(matches!(
        private_session.call_extension(
            "leaven/mcp.bridge",
            &acp_plan_params(),
            &RejectAllEffectHost
        ),
        Err(AcpTransportError::PublicSeam(_))
    ));

    let mut bare_payload_session = spawn_worker(
        &package,
        &profile,
        worker_script(
            r#"
read request
printf '%s\n' '{"jsonrpc":"2.0","id":"leaven-acp-0","result":{"message":{"role":"assistant","content":[{"kind":"text","text":"ok"}]}}}'
"#,
        ),
        "{}".to_owned(),
    );
    assert!(matches!(
        bare_payload_session.call_extension(
            "leaven/lm.complete",
            &acp_plan_params(),
            &RejectAllEffectHost
        ),
        Err(AcpTransportError::PublicSeam(_))
    ));
}

#[test]
fn stdio_session_carries_extension_result_envelopes_for_all_v1_method_families() {
    let package = package();
    let profile = profile(&package, 32, "pause_worker");

    for (index, case) in extension_result_cases().into_iter().enumerate() {
        let request_id = "leaven-acp-0";
        let mut session = spawn_worker(
            &package,
            &profile,
            worker_script(
                r#"
read request
printf '%s\n' "$LEAVEN_TEST_RESPONSE" | sed "s/__CAPABILITY_FINGERPRINT__/$LEAVEN_CAPABILITY_FINGERPRINT/g"
"#,
            ),
            response_for(case.method, request_id, case.result),
        );
        let response = session
            .call_extension(case.method, &acp_plan_params(), &RejectAllEffectHost)
            .unwrap_or_else(|error| panic!("case {index} {} failed: {error:?}", case.method));
        assert_eq!(response.method(), case.method);
        assert_eq!(response.primary_kind(), case.primary_kind);
    }
}

#[test]
fn stdio_session_rejects_malformed_session_update_notifications() {
    let package = package();
    let profile = profile(&package, 32, "pause_worker");
    let mut non_object = spawn_worker(
        &package,
        &profile,
        worker_script("printf '%s\n' '[]'\n"),
        "{}".to_owned(),
    );
    assert!(matches!(
        non_object.read_next_session_update(),
        Err(AcpTransportError::Protocol { .. })
    ));

    let mut response_shaped_update = spawn_worker(
        &package,
        &profile,
        worker_script(
            r#"
printf '%s\n' '{"jsonrpc":"2.0","id":"progress-1","method":"session/update","params":{"message":"bad","priority":"critical"}}'
"#,
        ),
        "{}".to_owned(),
    );
    assert!(matches!(
        response_shaped_update.read_next_session_update(),
        Err(AcpTransportError::Protocol { .. })
    ));

    let mut unknown_priority = spawn_worker(
        &package,
        &profile,
        worker_script(
            r#"
printf '%s\n' '{"jsonrpc":"2.0","method":"session/update","params":{"message":"bad","priority":"urgent"}}'
"#,
        ),
        "{}".to_owned(),
    );
    assert!(matches!(
        unknown_priority.read_next_session_update(),
        Err(AcpTransportError::Protocol { .. })
    ));
}

#[test]
fn stdio_session_rejects_cross_method_and_semantic_result_fakes_from_worker_process() {
    let package = package();
    let profile = profile(&package, 32, "pause_worker");

    let mut cross_method = spawn_worker(
        &package,
        &profile,
        worker_script(
            r#"
read request
printf '%s\n' "$LEAVEN_TEST_RESPONSE" | sed "s/__CAPABILITY_FINGERPRINT__/$LEAVEN_CAPABILITY_FINGERPRINT/g"
"#,
        ),
        response_for(
            "leaven-acp-ignored",
            "leaven-acp-0",
            extension_result(
                "leaven/lm.complete",
                lm_response_primary(),
                call_receipt("lm_complete", "lmrec_acp"),
                &["completion.raw"],
            ),
        ),
    );
    assert!(matches!(
        cross_method.call_extension("leaven/agent.run", &acp_plan_params(), &RejectAllEffectHost),
        Err(AcpTransportError::PublicSeam(_))
    ));

    let mut missing_receipts = extension_result(
        "leaven/lm.complete",
        lm_response_primary(),
        call_receipt("lm_complete", "lmrec_acp"),
        &["completion.raw"],
    );
    missing_receipts["receipts"] = json!([]);
    let mut semantic_fake = spawn_worker(
        &package,
        &profile,
        worker_script(
            r#"
read request
printf '%s\n' "$LEAVEN_TEST_RESPONSE" | sed "s/__CAPABILITY_FINGERPRINT__/$LEAVEN_CAPABILITY_FINGERPRINT/g"
"#,
        ),
        response_for("leaven/lm.complete", "leaven-acp-0", missing_receipts),
    );
    assert!(matches!(
        semantic_fake.call_extension(
            "leaven/lm.complete",
            &acp_plan_params(),
            &RejectAllEffectHost
        ),
        Err(AcpTransportError::PublicSeam(_))
    ));
}

#[test]
fn stdio_session_rejects_progress_overflow_for_default_pause_worker_queue() {
    let package = package();
    let profile = profile(&package, 1, "pause_worker");
    let mut session = spawn_worker(
        &package,
        &profile,
        worker_script(
            r#"
printf '%s\n' '{"jsonrpc":"2.0","method":"session/update","params":{"message":"first","priority":"critical"}}'
printf '%s\n' '{"jsonrpc":"2.0","method":"session/update","params":{"message":"second","priority":"critical"}}'
read request
printf '%s\n' "$LEAVEN_TEST_RESPONSE" | sed "s/__CAPABILITY_FINGERPRINT__/$LEAVEN_CAPABILITY_FINGERPRINT/g"
"#,
        ),
        response_for(
            "leaven/lm.complete",
            "leaven-acp-0",
            extension_result(
                "leaven/lm.complete",
                lm_response_primary(),
                call_receipt("lm_complete", "lmrec_acp"),
                &["completion.raw"],
            ),
        ),
    );

    let error = session
        .call_extension(
            "leaven/lm.complete",
            &acp_plan_params(),
            &RejectAllEffectHost,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("worker must pause"),
        "unexpected error: {error:?}"
    );
    assert_eq!(
        session
            .worker_session_snapshot()
            .lifecycle()
            .inflight_updates(),
        1
    );
}

#[test]
fn stdio_session_cancellation_reaches_live_worker_and_stops_later_calls() {
    let package = package();
    let profile = profile(&package, 32, "pause_worker");
    let temp = TempDir::new().unwrap();
    let cancel_log = temp.path().join("cancel.json");
    let mut session = spawn_worker_with_env(
        &package,
        &profile,
        worker_script(
            r#"
read cancel
printf '%s\n' "$cancel" > "$LEAVEN_TEST_CANCEL_LOG"
"#,
        ),
        response_for(
            "leaven/lm.complete",
            "leaven-acp-0",
            extension_result(
                "leaven/lm.complete",
                lm_response_primary(),
                call_receipt("lm_complete", "lmrec_acp"),
                &["completion.raw"],
            ),
        ),
        &[(
            "LEAVEN_TEST_CANCEL_LOG",
            cancel_log.to_str().unwrap().to_owned(),
        )],
    );

    session
        .cancel_with_error(
            "operator cancelled",
            "valrec_cancel",
            json!({
                "code": "cancelled",
                "message": "operator cancelled",
                "receipt": "valrec_cancel"
            }),
        )
        .unwrap();
    assert!(session.wait_for_exit().unwrap().success());

    let cancellation: Value =
        serde_json::from_str(&fs::read_to_string(cancel_log).unwrap()).unwrap();
    assert_eq!(cancellation["method"], json!("session/cancel"));
    assert_eq!(cancellation["params"]["receipt"], json!("valrec_cancel"));
    assert_eq!(cancellation["params"]["error"]["code"], json!("cancelled"));

    assert!(matches!(
        session.call_extension(
            "leaven/lm.complete",
            &acp_plan_params(),
            &RejectAllEffectHost
        ),
        Err(AcpTransportError::Protocol { .. })
    ));
}

#[test]
fn stdio_session_cancellation_interrupts_in_flight_extension_call_and_worker_observes_cancel() {
    let package = package();
    let profile = profile(&package, 32, "pause_worker");
    let temp = TempDir::new().unwrap();
    let request_log = temp.path().join("request.json");
    let cancel_log = temp.path().join("cancel.json");
    let mut session = spawn_worker_with_env(
        &package,
        &profile,
        worker_script(
            r#"
read request
printf '%s\n' "$request" > "$LEAVEN_TEST_REQUEST_LOG"
read cancel
printf '%s\n' "$cancel" > "$LEAVEN_TEST_CANCEL_LOG"
"#,
        ),
        response_for(
            "leaven/lm.complete",
            "leaven-acp-0",
            extension_result(
                "leaven/lm.complete",
                lm_response_primary(),
                call_receipt("lm_complete", "lmrec_acp"),
                &["completion.raw"],
            ),
        ),
        &[
            (
                "LEAVEN_TEST_REQUEST_LOG",
                request_log.to_str().unwrap().to_owned(),
            ),
            (
                "LEAVEN_TEST_CANCEL_LOG",
                cancel_log.to_str().unwrap().to_owned(),
            ),
        ],
    );
    let cancellation = session.cancellation_handle();
    let call = thread::spawn(move || {
        let result = session.call_extension(
            "leaven/lm.complete",
            &acp_plan_params(),
            &RejectAllEffectHost,
        );
        (session, result)
    });

    wait_for_file(&request_log);
    cancellation
        .cancel_with_error(
            "operator cancelled in-flight call",
            "valrec_inflight_cancel",
            json!({
                "code": "cancelled",
                "message": "operator cancelled in-flight call",
                "receipt": "valrec_inflight_cancel"
            }),
        )
        .unwrap();

    let (mut session, result) = call.join().unwrap();
    assert!(matches!(
        result,
        Err(AcpTransportError::Cancelled { receipt, .. })
            if receipt == "valrec_inflight_cancel"
    ));
    assert!(session.wait_for_exit().unwrap().success());
    let cancellation_value: Value =
        serde_json::from_str(&fs::read_to_string(cancel_log).unwrap()).unwrap();
    assert_eq!(cancellation_value["method"], json!("session/cancel"));
    assert_eq!(
        cancellation_value["params"]["receipt"],
        json!("valrec_inflight_cancel")
    );
    assert!(matches!(
        session.call_extension(
            "leaven/lm.complete",
            &acp_plan_params(),
            &RejectAllEffectHost
        ),
        Err(AcpTransportError::Protocol { .. })
    ));
}

#[test]
fn stdio_session_rejects_extension_response_after_recorded_cancellation() {
    let package = package();
    let profile = profile(&package, 32, "pause_worker");
    let temp = TempDir::new().unwrap();
    let request_log = temp.path().join("request.json");
    let mut session = spawn_worker_with_env(
        &package,
        &profile,
        worker_script(
            r#"
read request
printf '%s\n' "$request" > "$LEAVEN_TEST_REQUEST_LOG"
read cancel
printf '%s\n' "$LEAVEN_TEST_RESPONSE" | sed "s/__CAPABILITY_FINGERPRINT__/$LEAVEN_CAPABILITY_FINGERPRINT/g"
"#,
        ),
        response_for(
            "leaven/lm.complete",
            "leaven-acp-0",
            extension_result(
                "leaven/lm.complete",
                lm_response_primary(),
                call_receipt("lm_complete", "lmrec_acp"),
                &["completion.raw"],
            ),
        ),
        &[(
            "LEAVEN_TEST_REQUEST_LOG",
            request_log.to_str().unwrap().to_owned(),
        )],
    );
    let cancellation = session.cancellation_handle();
    let call = thread::spawn(move || {
        let result = session.call_extension(
            "leaven/lm.complete",
            &acp_plan_params(),
            &RejectAllEffectHost,
        );
        (session, result)
    });

    wait_for_file(&request_log);
    cancellation
        .cancel_with_error(
            "operator cancelled before late success",
            "valrec_late_success_cancel",
            json!({
                "code": "cancelled",
                "message": "operator cancelled before late success",
                "receipt": "valrec_late_success_cancel"
            }),
        )
        .unwrap();

    let (mut session, result) = call.join().unwrap();
    assert!(matches!(
        result,
        Err(AcpTransportError::Cancelled { receipt, .. })
            if receipt == "valrec_late_success_cancel"
    ));
    assert!(session.wait_for_exit().unwrap().success());
}

#[test]
fn stdio_session_progress_updates_are_lifecycle_control_without_response_side_effects() {
    let package = package();
    let profile = profile(&package, 2, "pause_worker");
    let mut session = spawn_worker(
        &package,
        &profile,
        worker_script(
            r#"
printf '%s\n' '{"jsonrpc":"2.0","method":"session/update","params":{"message":"standalone","priority":"critical"}}'
read request
"#,
        ),
        "{}".to_owned(),
    );

    let disposition = session.read_next_session_update().unwrap();
    assert!(matches!(
        disposition,
        AcpProgressDisposition::Enqueued(update) if update.message() == "standalone"
    ));
    assert_eq!(
        session
            .worker_session_snapshot()
            .lifecycle()
            .inflight_updates(),
        1
    );
}

#[test]
fn stdio_session_backpressure_disconnects_live_overproducer() {
    let package = package();
    let profile = profile(&package, 1, "disconnect");
    let temp = TempDir::new().unwrap();
    let cancel_log = temp.path().join("cancel.json");
    let mut session = spawn_worker_with_env(
        &package,
        &profile,
        worker_script(
            r#"
printf '%s\n' '{"jsonrpc":"2.0","method":"session/update","params":{"message":"first","priority":"critical"}}'
printf '%s\n' '{"jsonrpc":"2.0","method":"session/update","params":{"message":"overflow","priority":"critical"}}'
read cancel
printf '%s\n' "$cancel" > "$LEAVEN_TEST_CANCEL_LOG"
"#,
        ),
        "{}".to_owned(),
        &[(
            "LEAVEN_TEST_CANCEL_LOG",
            cancel_log.to_str().unwrap().to_owned(),
        )],
    );

    assert!(matches!(
        session.read_next_session_update().unwrap(),
        AcpProgressDisposition::Enqueued(_)
    ));
    assert!(matches!(
        session.read_next_session_update().unwrap(),
        AcpProgressDisposition::Disconnected(reason)
            if reason == "ACP session disconnected after update overflow"
    ));
    assert!(session.wait_for_exit().unwrap().success());
    let cancellation: Value =
        serde_json::from_str(&fs::read_to_string(cancel_log).unwrap()).unwrap();
    assert_eq!(cancellation["method"], json!("session/cancel"));
    assert_eq!(
        cancellation["params"]["receipt"],
        json!("valrec_acp_disconnect_1")
    );
}

struct ExtensionCase {
    method: &'static str,
    primary_kind: &'static str,
    result: Value,
}

fn package() -> PublicSeamPackage {
    PublicSeamPackage::active_from_repo(workspace_root()).unwrap()
}

fn profile(
    package: &PublicSeamPackage,
    max_inflight_updates: u64,
    backpressure: &str,
) -> AcpProfileDocument {
    let mut value = acp_profile();
    value["flow_control"]["default_max_inflight_updates"] = json!(max_inflight_updates);
    value["flow_control"]["backpressure"] = json!(backpressure);
    package.validate_acp_profile_document(&value).unwrap()
}

fn spawn_worker(
    package: &PublicSeamPackage,
    profile: &AcpProfileDocument,
    script: TempDir,
    response: String,
) -> AcpStdioProcessSession {
    spawn_worker_with_env(package, profile, script, response, &[])
}

fn spawn_worker_with_env(
    package: &PublicSeamPackage,
    profile: &AcpProfileDocument,
    script: TempDir,
    response: String,
    extra_env: &[(&str, String)],
) -> AcpStdioProcessSession {
    let script_path = script.path().join("worker.sh");
    let mut command = AcpProcessCommand::new("/bin/sh")
        .arg(script_path.to_str().unwrap())
        .env("LEAVEN_TEST_RESPONSE", response);
    for (key, value) in extra_env {
        command = command.env(*key, value.clone());
    }
    // Keep the temporary script alive for the child process lifetime by leaking
    // the test fixture directory. The process itself is killed on session drop.
    std::mem::forget(script);
    AcpStdioProcessSession::spawn(
        package.clone(),
        profile.clone(),
        command,
        "secret-token",
        "stdio://worker/session",
        "fp_cap_sha256_acp",
    )
    .unwrap()
}

fn worker_script(body: &str) -> TempDir {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("worker.sh");
    fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    temp
}

fn python_worker_script(body: &str) -> TempDir {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("worker.py");
    fs::write(&path, body).unwrap();
    temp
}

fn wait_for_file(path: &Path) {
    for _ in 0..100 {
        if path.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {}", path.display());
}

fn response_for(method: &str, id: &str, mut result: Value) -> String {
    result["method"] = json!(method);
    result["capability_fingerprint"] = json!("__CAPABILITY_FINGERPRINT__");
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
    .to_string()
}

fn extension_result_cases() -> Vec<ExtensionCase> {
    let mut cases = extension_result_query_cases();
    cases.extend(extension_result_workspace_cases());
    cases.extend(extension_result_effect_and_write_cases());
    cases
}

fn extension_result_query_cases() -> Vec<ExtensionCase> {
    let mut cases = Vec::new();
    for (method, op, receipt) in [
        ("leaven/graph.query", "graph.query", "qrec_graph"),
        ("leaven/case.load", "case.load", "qrec_case_load"),
        ("leaven/case.input", "case.input", "qrec_case_input"),
        ("leaven/case.target", "case.target", "qrec_case_target"),
        (
            "leaven/case.metadata",
            "case.metadata",
            "qrec_case_metadata",
        ),
        ("leaven/human.review", "human.review", "humanrec_acp"),
        ("leaven/event.emit", "event.emit", "wrec_event_emit"),
    ] {
        let receipt = if method == "leaven/human.review" {
            call_receipt("human_review", receipt)
        } else if method == "leaven/event.emit" {
            write_receipt("emit_run_event", receipt)
        } else {
            query_receipt(receipt)
        };
        cases.push(case(
            method,
            "extension",
            extension_result(method, extension_primary(op), receipt, &["public"]),
        ));
    }
    cases
}

fn extension_result_workspace_cases() -> Vec<ExtensionCase> {
    let mut cases = vec![
        case(
            "leaven/workspace.materialize",
            "workspace_handle",
            extension_result(
                "leaven/workspace.materialize",
                workspace_handle_primary(false, "wrec_materialize"),
                call_receipt("workspace_materialize", "wrec_materialize"),
                &["workspace.file"],
            ),
        ),
        case(
            "leaven/workspace.release",
            "workspace_handle",
            extension_result(
                "leaven/workspace.release",
                workspace_handle_primary(true, "wrec_release"),
                call_receipt("workspace_release", "wrec_release"),
                &["workspace.file"],
            ),
        ),
    ];
    for (method, primary_kind, primary, receipt) in [
        (
            "leaven/workspace.snapshot",
            "workspace_snapshot",
            workspace_snapshot_primary(),
            query_receipt("qrec_workspace_snapshot"),
        ),
        (
            "leaven/workspace.list",
            "workspace_listing",
            workspace_listing_primary(),
            query_receipt("qrec_workspace_list"),
        ),
        (
            "leaven/workspace.read_file",
            "workspace_file",
            workspace_file_primary(),
            query_receipt("qrec_workspace_file"),
        ),
        (
            "leaven/workspace.stat",
            "workspace_listing",
            workspace_listing_primary(),
            query_receipt("qrec_workspace_stat"),
        ),
        (
            "leaven/workspace.digest",
            "workspace_snapshot",
            workspace_snapshot_primary(),
            query_receipt("qrec_workspace_digest"),
        ),
        (
            "leaven/workspace.git_log",
            "workspace_diff",
            workspace_diff_primary(),
            query_receipt("qrec_workspace_git_log"),
        ),
        (
            "leaven/workspace.git_diff",
            "workspace_diff",
            workspace_diff_primary(),
            query_receipt("qrec_workspace_git_diff"),
        ),
        (
            "leaven/workspace.git_status",
            "workspace_diff",
            workspace_diff_primary(),
            query_receipt("qrec_workspace_git_status"),
        ),
        (
            "leaven/workspace.capture_artifacts",
            "workspace_listing",
            workspace_listing_primary(),
            query_receipt("qrec_workspace_capture"),
        ),
    ] {
        cases.push(case(
            method,
            primary_kind,
            extension_result(method, primary, receipt, &["workspace.file"]),
        ));
    }
    cases
}

fn extension_result_effect_and_write_cases() -> Vec<ExtensionCase> {
    vec![
        case(
            "leaven/lm.complete",
            "lm_response",
            extension_result(
                "leaven/lm.complete",
                lm_response_primary(),
                call_receipt("lm_complete", "lmrec_acp"),
                &["completion.raw"],
            ),
        ),
        case(
            "leaven/agent.run",
            "agent_session",
            extension_result(
                "leaven/agent.run",
                agent_session_primary(),
                call_receipt("agent_run", "agentrec_acp"),
                &["public", "transcript.raw"],
            ),
        ),
        case(
            "leaven/sandbox.exec",
            "sandbox_exec",
            extension_result(
                "leaven/sandbox.exec",
                sandbox_exec_primary(),
                call_receipt("sandbox_exec", "execrec_acp"),
                &["public"],
            ),
        ),
        case(
            "leaven/proposal.submit_batch",
            "proposal_batch_receipt",
            extension_result(
                "leaven/proposal.submit_batch",
                proposal_batch_primary(),
                write_receipt("submit_proposal_batch", "wrec_proposal_submit"),
                &["public"],
            ),
        ),
        case(
            "leaven/proposal.apply",
            "apply_receipt",
            extension_result(
                "leaven/proposal.apply",
                apply_receipt_primary(),
                write_receipt("apply_proposal_batch", "wrec_proposal_apply"),
                &["public"],
            ),
        ),
        case(
            "leaven/assessment.submit",
            "assessment_batch_receipt",
            extension_result(
                "leaven/assessment.submit",
                assessment_batch_primary(),
                write_receipt("submit_assessments", "wrec_assessment_submit"),
                &["public"],
            ),
        ),
        case(
            "leaven/evaluation.request",
            "evaluation_request_receipt",
            extension_result(
                "leaven/evaluation.request",
                evaluation_request_primary(),
                write_receipt("request_evaluation", "wrec_evaluation_request"),
                &["public"],
            ),
        ),
    ]
}

fn case(method: &'static str, primary_kind: &'static str, result: Value) -> ExtensionCase {
    ExtensionCase {
        method,
        primary_kind,
        result,
    }
}

fn extension_result(method: &str, primary: Value, receipt: Value, data_classes: &[&str]) -> Value {
    let mut result = json!({
        "method": method,
        "redactions": [],
        "capability_fingerprint": "fp_cap_sha256_acp",
        "data_classes": data_classes
    });
    result["primary"] = primary;
    result["receipts"] = Value::Array(vec![receipt]);
    let schema_version = match result["receipts"][0]["kind"].as_str().unwrap() {
        "query" => "leaven.plan_query_result.v1",
        "call" => "leaven.plan_call_result.v1",
        "write" => "leaven.plan_write_result.v1",
        other => panic!("unexpected receipt kind {other}"),
    };
    let op_name = result["receipts"][0]["op_var"]
        .as_str()
        .unwrap_or("primary");
    result["receipts"][0]["result_hash"] = json!(prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": schema_version,
            "name": op_name,
            "value": result["primary"]
        }),
    ));
    result
}

fn acp_profile() -> Value {
    json!({
        "schema_version": "leaven.acp_profile.v1",
        "base_protocol": "agent-client-protocol",
        "pinned_acp_version": "0.4.0",
        "transports": ["stdio_jsonrpc", "unix_socket_jsonrpc"],
        "auth": {
            "token_env": "LEAVEN_CAPABILITY_TOKEN",
            "endpoint_env": "LEAVEN_ENDPOINT",
            "fingerprint_env": "LEAVEN_CAPABILITY_FINGERPRINT",
            "http_header": "Authorization: Bearer <token>",
            "authenticate_maps_to": "leaven.capability.v1"
        },
        "permission_model": {
            "source": "ACP session/request_permission",
            "answer": "programmatic capability grant check",
            "denial": "PlanError + Redaction"
        },
        "extension_methods": locked_profile_methods(),
        "flow_control": {
            "bounded_channel_required": true,
            "default_max_inflight_updates": 32,
            "backpressure": "pause_worker",
            "heartbeat_ms": 1000
        }
    })
}

fn locked_profile_methods() -> Vec<Value> {
    vec![
        stage_run_method(),
        extension_method("leaven/graph.query", "graph.query"),
        extension_method("leaven/case.load", "case.read"),
        extension_method("leaven/case.input", "case.read"),
        extension_method("leaven/case.target", "case.read"),
        extension_method("leaven/case.metadata", "case.read"),
        extension_method("leaven/workspace.materialize", "workspace.materialize"),
        extension_method("leaven/workspace.snapshot", "workspace.read"),
        extension_method("leaven/workspace.list", "workspace.read"),
        extension_method("leaven/workspace.read_file", "workspace.read"),
        extension_method("leaven/workspace.stat", "workspace.read"),
        extension_method("leaven/workspace.digest", "workspace.read"),
        extension_method("leaven/workspace.git_log", "workspace.read"),
        extension_method("leaven/workspace.git_diff", "workspace.read"),
        extension_method("leaven/workspace.git_status", "workspace.read"),
        extension_method("leaven/workspace.capture_artifacts", "workspace.read"),
        extension_method("leaven/workspace.release", "workspace.release"),
        extension_method("leaven/lm.complete", "lm.complete"),
        extension_method("leaven/agent.run", "agent.run"),
        extension_method("leaven/sandbox.exec", "sandbox.exec"),
        extension_method("leaven/human.review", "human.review"),
        extension_method("leaven/proposal.submit_batch", "proposal.submit_batch"),
        extension_method("leaven/proposal.apply", "proposal.apply_batch"),
        extension_method("leaven/assessment.submit", "assessment.submit"),
        extension_method("leaven/evaluation.request", "evaluation.request"),
        extension_method("leaven/event.emit", "event.emit"),
    ]
}

fn extension_method(method: &str, action: &str) -> Value {
    json!({
        "method": method,
        "params_schema": "leaven.plan.v1.schema.json",
        "result_schema": "leaven.plan_result.v1.schema.json",
        "required_action": action,
        "produces_receipt": true
    })
}

fn stage_run_method() -> Value {
    json!({
        "method": "leaven/stage.run",
        "params_schema": "leaven.stage_run.v1.schema.json",
        "result_schema": "leaven.stage_run.v1.schema.json",
        "required_action": "stage.run",
        "produces_receipt": true
    })
}

fn acp_plan_params() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_acp_jsonrpc",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "dry_run"},
        "ops": [{
            "kind": "let",
            "name": "input",
            "expr": {
                "kind": "literal",
                "value": "hello",
                "data_classes": ["public"]
            }
        }],
        "return": ["input"],
        "commit": {"kind": "no_graph_writes"}
    })
}

fn acp_plan_params_for_method(method: &str) -> Value {
    let mut params = acp_plan_params();
    params["ops"][0]["expr"]["value"] = json!(method);
    params
}

fn extension_primary(op: &str) -> Value {
    json!({
        "kind": "extension",
        "namespace": "leaven",
        "op": op,
        "schema_fingerprint": "fp_schema_sha256_acpextension",
        "payload": {"status": "ok"}
    })
}

fn workspace_handle_primary(released: bool, receipt: &str) -> Value {
    json!({
        "kind": "workspace_handle",
        "workspace": "ws_acp",
        "lifetime": "stage_call",
        "released": released,
        "graph_revision": "rev_acp",
        "data_classes": ["workspace.file"],
        "replayability": "fully_managed",
        "receipt": receipt
    })
}

fn workspace_snapshot_primary() -> Value {
    json!({
        "kind": "workspace_snapshot",
        "workspace": "ws_acp",
        "digest": "sha256:workspace",
        "graph_revision": "rev_acp",
        "data_classes": ["workspace.file"],
        "replayability": "pure_read"
    })
}

fn workspace_listing_primary() -> Value {
    json!({
        "kind": "workspace_listing",
        "entries": [{"path": "src/lib.rs", "kind": "file", "data_classes": ["workspace.file"]}],
        "graph_revision": "rev_acp",
        "data_classes": ["workspace.file"],
        "replayability": "pure_read"
    })
}

fn workspace_file_primary() -> Value {
    json!({
        "kind": "workspace_file",
        "path": "src/lib.rs",
        "content": "pub fn demo() {}",
        "graph_revision": "rev_acp",
        "data_classes": ["workspace.file"],
        "replayability": "pure_read",
        "receipt": "qrec_workspace_file"
    })
}

fn workspace_diff_primary() -> Value {
    json!({
        "kind": "workspace_diff",
        "text": " M src/lib.rs",
        "graph_revision": "rev_acp",
        "data_classes": ["workspace.file"],
        "replayability": "pure_read"
    })
}

fn lm_response_primary() -> Value {
    json!({
        "kind": "lm_response",
        "message": {
            "role": "assistant",
            "content": [{"kind": "text", "text": "ok"}]
        },
        "graph_revision": "rev_acp",
        "cost": {"usd_micro": 42, "lm_calls": 1},
        "data_classes": ["completion.raw"],
        "replayability": "fully_managed",
        "receipt": "lmrec_acp"
    })
}

fn agent_session_primary() -> Value {
    json!({
        "kind": "agent_session",
        "status": "completed",
        "transcript_ref": acp_blob_ref("blob_agent_transcript", &["transcript.raw"]),
        "commands": [{
            "argv": ["codex"],
            "status": "completed",
            "receipt": "agentrec_acp",
            "stdout_ref": acp_blob_ref("blob_agent_stdout", &["transcript.raw"]),
            "stderr_ref": acp_blob_ref("blob_agent_stderr", &["transcript.raw"])
        }],
        "cost": {"usd_micro": 1000, "agent_calls": 1},
        "graph_revision": "rev_acp",
        "data_classes": ["public", "transcript.raw"],
        "replayability": "fully_managed",
        "receipt": "agentrec_acp"
    })
}

fn sandbox_exec_primary() -> Value {
    json!({
        "kind": "sandbox_exec",
        "status": "completed",
        "exit_code": 0,
        "cost": {"usd_micro": 10, "sandbox_calls": 1},
        "stdout_ref": acp_blob_ref("blob_sandbox_stdout", &["public"]),
        "stderr_ref": acp_blob_ref("blob_sandbox_stderr", &["public"]),
        "graph_revision": "rev_acp",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "execrec_acp"
    })
}

fn proposal_batch_primary() -> Value {
    json!({
        "kind": "proposal_batch_receipt",
        "batch_id": "pb_acp",
        "proposal_ids": ["prop_acp"],
        "status": "committed",
        "graph_revision": "rev_acp",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "wrec_proposal_submit"
    })
}

fn apply_receipt_primary() -> Value {
    json!({
        "kind": "apply_receipt",
        "created_candidates": ["cand_acp_created"],
        "status": "committed",
        "graph_revision": "rev_acp",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "wrec_proposal_apply"
    })
}

fn assessment_batch_primary() -> Value {
    json!({
        "kind": "assessment_batch_receipt",
        "evaluation_request_id": "evalreq_acp",
        "assessment_ids": ["assess_acp"],
        "per_assessment": [
            {
                "assessment": "assess_acp",
                "replayability": "fully_managed"
            }
        ],
        "status": "committed",
        "graph_revision": "rev_acp",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "wrec_assessment_submit"
    })
}

fn evaluation_request_primary() -> Value {
    json!({
        "kind": "evaluation_request_receipt",
        "evaluation_request_id": "evalreq_acp",
        "status": "recorded",
        "graph_revision": "rev_acp",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "wrec_evaluation_request"
    })
}

fn acp_blob_ref(id: &str, data_classes: &[&str]) -> Value {
    json!({
        "kind": "blob_ref",
        "id": id,
        "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "bytes": 32,
        "data_classes": data_classes
    })
}

fn call_receipt(call_kind: &str, receipt: &str) -> Value {
    let mut value = json!({
        "kind": "call",
        "receipt": receipt,
        "op_var": "worker_call",
        "started_at": "2026-05-23T00:00:00Z",
        "completed_at": "2026-05-23T00:00:01Z",
        "call_kind": call_kind,
        "request_hash": "fp_request_sha256_acp",
        "result_hash": "fp_result_sha256_acp",
        "runtime_fingerprint": "fp_runtime_sha256_acp",
        "status": "succeeded"
    });
    match call_kind {
        "lm_complete" => value["cost"] = json!({"usd_micro": 42, "lm_calls": 1}),
        "agent_run" => value["cost"] = json!({"usd_micro": 1000, "agent_calls": 1}),
        "sandbox_exec" => value["cost"] = json!({"usd_micro": 10, "sandbox_calls": 1}),
        _ => {}
    }
    value
}

fn write_receipt(write_kind: &str, receipt: &str) -> Value {
    let mut value = json!({
        "kind": "write",
        "receipt": receipt,
        "op_var": "primary",
        "started_at": "2026-05-23T00:00:00Z",
        "completed_at": "2026-05-23T00:00:01Z",
        "write_kind": write_kind,
        "request_hash": "fp_request_sha256_acp",
        "result_hash": "fp_result_sha256_acp",
        "base_revision": "rev_acp",
        "committed_revision": "rev_acp",
        "status": "succeeded"
    });
    match write_kind {
        "submit_proposal_batch" => {
            value["proposal_batch_id"] = json!("pb_acp");
            value["proposal_ids"] = json!(["prop_acp"]);
        }
        "apply_proposal_batch" => {
            value["created_candidates"] = json!(["cand_acp_created"]);
        }
        "submit_assessments" => {
            value["evaluation_request_id"] = json!("evalreq_acp");
            value["assessment_ids"] = json!(["assess_acp"]);
            value["request_hash"] = json!(prefixed_jcs_hash(
                "fp_request_sha256_",
                &json!({
                    "schema_version": "leaven.submit_assessments_request.v1",
                    "evaluation_request_id": "evalreq_acp",
                    "assessment_ids": ["assess_acp"]
                }),
            ));
        }
        "request_evaluation" => {
            value["evaluation_request_id"] = json!("evalreq_acp");
        }
        "emit_run_event" => {
            value["event_id"] = json!("event_acp");
        }
        other => panic!("unexpected write kind {other}"),
    }
    value
}

fn query_receipt(receipt: &str) -> Value {
    json!({
        "kind": "query",
        "receipt": receipt,
        "op_var": "workspace_read",
        "started_at": "2026-05-23T00:00:00Z",
        "completed_at": "2026-05-23T00:00:01Z",
        "op_hash": "fp_query_sha256_acp",
        "result_hash": "fp_result_sha256_acp",
        "graph_revision": "rev_acp",
        "status": "succeeded",
        "read_scope_fingerprint": "fp_scope_sha256_acp",
        "projection_fingerprint": "fp_projection_sha256_acp"
    })
}

fn prefixed_jcs_hash(prefix: &str, value: &Value) -> String {
    format!(
        "{prefix}{}",
        jcs_canonicalize::sha256_jcs_hex(value).unwrap()
    )
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
}
