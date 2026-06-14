use std::{
    fs,
    path::{Path, PathBuf},
};

use leaven_acp::{AcpProcessCommand, AcpStdioProcessSession, RejectAllEffectHost};
use leaven_public_seam::{AcpProfileDocument, LockedMethod, MethodPrimaryKind, PublicSeamPackage};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    Trace2SkillFileArtifact, Trace2SkillManifestError, Trace2SkillOneCaseInput,
    Trace2SkillOneCaseRunInput, Trace2SkillOneCaseRunReport, Trace2SkillOneCaseRunScoreReport,
    Trace2SkillOneCaseRunScoringInput, file_artifact, prepare_trace2skill_one_case_run,
    score_trace2skill_one_case_run,
};

/// Inputs for the deterministic ACP external-worker one-case proof.
#[derive(Clone, Copy, Debug)]
pub struct Trace2SkillOneCaseAcpWorkerInput<'a> {
    /// Exact materialized case inputs.
    pub case: Trace2SkillOneCaseInput<'a>,
    /// Durable run directory where every artifact is written.
    pub run_dir: &'a Path,
    /// Solver identity recorded into the Leaven trajectory.
    pub model_id: &'a str,
}

/// Durable artifact report for the deterministic ACP external-worker one-case proof.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Trace2SkillOneCaseAcpWorkerReport {
    /// Prepared run-directory manifest and prompt artifacts.
    pub prepared: Trace2SkillOneCaseRunReport,
    /// Raw `leaven/agent.run` primary result returned by the worker.
    pub acp_result_file: Trace2SkillFileArtifact,
    /// Scored workbook and trajectory artifacts.
    pub scored: Trace2SkillOneCaseRunScoreReport,
}

/// Runs the deterministic local Python ACP worker, then scores its workbook.
pub fn run_trace2skill_one_case_acp_external_worker(
    input: Trace2SkillOneCaseAcpWorkerInput<'_>,
) -> Result<Trace2SkillOneCaseAcpWorkerReport, Trace2SkillManifestError> {
    let prepared = prepare_trace2skill_one_case_run(Trace2SkillOneCaseRunInput {
        case: input.case,
        run_dir: input.run_dir,
        output_workbook: None,
    })?;

    let package = PublicSeamPackage::active_from_repo(workspace_root())
        .map_err(|source| external_worker_error("load public seam package", source.to_string()))?;
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .map_err(|source| external_worker_error("validate ACP profile", source.to_string()))?;
    let worker_script = write_worker_script(input.run_dir)?;
    let mut session = spawn_worker(&package, &profile, &worker_script, input.run_dir)?;
    let response = session
        .call_extension(
            LockedMethod::AgentRun,
            &agent_run_plan_params(),
            &RejectAllEffectHost,
        )
        .map_err(|source| external_worker_error("call leaven/agent.run", source.to_string()))?;
    if response.method() != LockedMethod::AgentRun {
        return Err(external_worker_error(
            "validate ACP method",
            format!(
                "expected {:?}, got {:?}",
                LockedMethod::AgentRun,
                response.method()
            ),
        ));
    }
    if response.primary_kind() != MethodPrimaryKind::AgentSession {
        return Err(external_worker_error(
            "validate ACP primary kind",
            format!(
                "expected {:?}, got {:?}",
                MethodPrimaryKind::AgentSession,
                response.primary_kind()
            ),
        ));
    }
    assert_workbook_bound_to_acp_result(response.result(), &prepared.output_workbook)?;
    let acp_result_path = input.run_dir.join("acp_result.json");
    fs::write(
        &acp_result_path,
        serde_json::to_vec_pretty(response.result())?,
    )?;
    drop(session);

    let transcript_file = input.run_dir.join("agent_transcript.md");
    let scored = score_trace2skill_one_case_run(Trace2SkillOneCaseRunScoringInput {
        run_dir: input.run_dir,
        model_id: input.model_id,
        transcript_file: &transcript_file,
    })?;

    Ok(Trace2SkillOneCaseAcpWorkerReport {
        prepared,
        acp_result_file: file_artifact(&acp_result_path)?,
        scored,
    })
}

fn assert_workbook_bound_to_acp_result(
    result: &Value,
    output_workbook: &Path,
) -> Result<(), Trace2SkillManifestError> {
    let workbook_bytes = fs::read(output_workbook)?;
    let workbook_ref = result
        .pointer("/primary/commands/0/files/13-1_output.xlsx")
        .ok_or_else(|| {
            external_worker_error(
                "validate workbook binding",
                "ACP result omitted 13-1_output.xlsx".to_owned(),
            )
        })?;
    let expected_sha = sha256_hex(&workbook_bytes);
    if workbook_ref["kind"] != json!("blob_ref")
        || workbook_ref["bytes"] != json!(workbook_bytes.len())
        || workbook_ref["sha256"] != json!(expected_sha)
        || workbook_ref["data_classes"] != json!(["workspace.file"])
    {
        return Err(external_worker_error(
            "validate workbook binding",
            "ACP result did not match the produced workbook bytes".to_owned(),
        ));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn spawn_worker(
    package: &PublicSeamPackage,
    profile: &AcpProfileDocument,
    script: &Path,
    run_dir: &Path,
) -> Result<AcpStdioProcessSession, Trace2SkillManifestError> {
    AcpStdioProcessSession::spawn(
        package.clone(),
        profile.clone(),
        AcpProcessCommand::new("uv")
            .arg("run")
            .arg("--with")
            .arg("openpyxl")
            .arg("python")
            .arg(script.to_str().unwrap_or(""))
            .env("LEAVEN_TRACE2SKILL_RUN_DIR", run_dir.to_str().unwrap_or("")),
        "secret-token",
        "stdio://trace2skill-spreadsheetbench/worker",
        "fp_cap_sha256_acp",
    )
    .map_err(|source| external_worker_error("spawn ACP worker", source.to_string()))
}

fn write_worker_script(run_dir: &Path) -> Result<PathBuf, Trace2SkillManifestError> {
    fs::create_dir_all(run_dir)?;
    let path = run_dir.join("trace2skill_acp_worker.py");
    fs::write(&path, PYTHON_WORKER)?;
    Ok(path)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn external_worker_error(context: &'static str, message: String) -> Trace2SkillManifestError {
    Trace2SkillManifestError::ExternalWorker { context, message }
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
        extension_method("leaven/proposal.submit_batch", "proposal.submit_batch"),
        extension_method("leaven/proposal.apply", "proposal.apply_batch"),
        extension_method("leaven/assessment.submit", "assessment.submit"),
        extension_method("leaven/evaluation.request", "evaluation.request"),
        extension_method("leaven/event.emit", "event.emit"),
    ]
}

fn agent_run_plan_params() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_trace2skill_spreadsheetbench_acp",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [
            {
                "kind": "call",
                "name": "workspace",
                "idempotency_key": "trace2skill-acp-workspace-0001",
                "call": {
                    "kind": "workspace_materialize",
                    "candidate": "cand_trace2skill_spreadsheetbench_13_1",
                    "surface": "program",
                    "mode": "copy_on_write",
                    "lifetime": "manual_release"
                }
            },
            {
                "kind": "call",
                "name": "solve_case",
                "deps": ["workspace"],
                "idempotency_key": "trace2skill-acp-agent-run-0001",
                "call": {
                    "kind": "agent_run",
                    "runtime": "python-stdio-trace2skill-worker",
                    "runtime_fingerprint": "fp_runtime_sha256_trace2skillpythonworker",
                    "workspace": "ws_trace2skill_spreadsheetbench_13_1",
                    "instructions": {
                        "system": "Solve exactly one Trace2Skill SpreadsheetBench-Verified case inside the materialized workspace.",
                        "task": "Read the initial 13-1 workbook, group RANGES rows by DATE and REF, write the LISTS DATA and OPERATION sections, and return the transcript plus the produced workbook artifact."
                    },
                    "tool_policy": {
                        "allow_shell": false,
                        "allowed_tools": ["read_file", "write_file"],
                        "allowed_commands": ["python"]
                    },
                    "output": {
                        "kind": "files",
                        "paths": ["13-1_output.xlsx"],
                        "max_bytes": 1048576
                    },
                    "limits": {
                        "timeout_s": 30,
                        "max_turns": 4,
                        "max_usd_micro": 0
                    },
                    "input_classes": ["public", "workspace.file"]
                }
            }
        ],
        "return": ["workspace", "solve_case"],
        "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"}
    })
}

const PYTHON_WORKER: &str = r#"
import datetime
import hashlib
import json
import os
import sys
from collections import defaultdict
from pathlib import Path


def canon(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def result_hash(primary):
    bound = {
        "schema_version": "leaven.plan_call_result.v1",
        "name": "worker_call",
        "value": primary,
    }
    return "fp_result_sha256_" + hashlib.sha256(canon(bound).encode("utf-8")).hexdigest()


def parse_date(value):
    if isinstance(value, datetime.datetime):
        return value
    if isinstance(value, datetime.date):
        return datetime.datetime(value.year, value.month, value.day)
    return datetime.datetime.strptime(str(value), "%m/%d/%Y")


def grouped_rows(ranges, section):
    current = None
    groups = defaultdict(int)
    first_dates = {}
    for row in ranges.iter_rows(min_row=1, values_only=True):
        marker = row[2]
        if marker in ("STAGE", "DATA", "OPERATION"):
            current = marker
            continue
        if current != section or row[0] in (None, "S.N"):
            continue
        date = parse_date(row[1])
        ref = row[3]
        amount = row[4]
        key = (date.date().isoformat(), ref)
        groups[key] += int(amount)
        first_dates[key] = date
    return [(first_dates[key], key[1], amount) for key, amount in sorted(groups.items())]


def section_starts(lists):
    starts = {}
    for row in range(1, lists.max_row + 1):
        marker = lists.cell(row=row, column=3).value
        if marker in ("STAGE", "DATA", "OPERATION"):
            starts[marker] = row + 2
    return starts


def write_section(lists, starts, section, rows):
    start = starts[section]
    row = start
    for index, (date, ref, amount) in enumerate(rows, start=1):
        lists.cell(row=row, column=1, value=index)
        lists.cell(row=row, column=2, value=date)
        lists.cell(row=row, column=3, value=ref)
        lists.cell(row=row, column=4, value=amount)
        row += 1
    lists.cell(row=row, column=1, value="TOTAL")
    lists.cell(row=row, column=2, value=None)
    lists.cell(row=row, column=3, value=None)
    lists.cell(row=row, column=4, value=sum(amount for _, _, amount in rows))


def solve_spreadsheet(run_dir):
    from openpyxl import load_workbook

    source = run_dir / "1_13-1_init.xlsx"
    output = run_dir / "13-1_output.xlsx"
    workbook = load_workbook(source)
    ranges = workbook["RANGES"]
    lists = workbook["LISTS"]
    starts = section_starts(lists)
    for section in ("STAGE", "DATA", "OPERATION"):
        write_section(lists, starts, section, grouped_rows(ranges, section))
    workbook.save(output)
    return output


def write_transcript(run_dir, solved):
    transcript = run_dir / "agent_transcript.md"
    transcript.write_text(
        "\n".join([
            "ACTION: read RANGES",
            "OBSERVATION: found STAGE, DATA, and OPERATION source sections",
            "ACTION: grouped by DATE and REF",
            "ACTION: wrote LISTS DATA and OPERATION sections",
            f"OUTPUT: {solved}",
            "",
        ]),
        encoding="utf-8",
    )
    return transcript


def blob_ref(path, blob_id, data_classes):
    data = path.read_bytes()
    return {
        "kind": "blob_ref",
        "id": blob_id,
        "sha256": hashlib.sha256(data).hexdigest(),
        "bytes": len(data),
        "data_classes": data_classes,
    }


def empty_blob_ref(blob_id):
    return {
        "kind": "blob_ref",
        "id": blob_id,
        "sha256": hashlib.sha256(b"").hexdigest(),
        "bytes": 0,
        "data_classes": ["transcript.raw"],
    }


def primary(transcript, status, solved):
    files = {
        "13-1_output.xlsx": blob_ref(
            solved,
            "blob_trace2skill_output_workbook",
            ["workspace.file"],
        )
    }
    return {
        "kind": "agent_session",
        "status": status,
        "transcript_ref": blob_ref(transcript, "blob_trace2skill_transcript", ["transcript.raw"]),
        "commands": [{
            "argv": ["python", "trace2skill_spreadsheetbench_solver"],
            "status": status,
            "receipt": "agentrec_trace2skill",
            "stdout_ref": empty_blob_ref("blob_trace2skill_stdout"),
            "stderr_ref": empty_blob_ref("blob_trace2skill_stderr"),
            "files": files,
        }],
        "cost": {"usd_micro": 0, "agent_calls": 1},
        "graph_revision": "rev_trace2skill_acp",
        "data_classes": ["public", "transcript.raw", "workspace.file"],
        "replayability": "fully_managed",
        "receipt": "agentrec_trace2skill",
    }


def extension_result(method, transcript, status, solved):
    value = primary(transcript, status, solved)
    return {
        "method": method,
        "primary": value,
        "receipts": [{
            "kind": "call",
            "receipt": "agentrec_trace2skill",
            "op_var": "worker_call",
            "started_at": "2026-05-24T00:00:00Z",
            "completed_at": "2026-05-24T00:00:01Z",
            "call_kind": "agent_run",
            "request_hash": "fp_request_sha256_trace2skill_acp",
            "result_hash": result_hash(value),
            "runtime_fingerprint": "fp_runtime_sha256_trace2skill_acp",
            "cost": {"usd_micro": 0, "agent_calls": 1},
            "status": "succeeded",
        }],
        "redactions": [],
        "capability_fingerprint": os.environ["LEAVEN_CAPABILITY_FINGERPRINT"],
        "data_classes": value["data_classes"],
    }


run_dir = Path(os.environ["LEAVEN_TRACE2SKILL_RUN_DIR"])

for line in sys.stdin:
    request = json.loads(line)
    assert request["jsonrpc"] == "2.0"
    assert request["method"] == "leaven/agent.run"
    params = request["params"]
    assert params["schema_version"] == "leaven.plan.v1"
    assert params["mode"]["kind"] == "execute"
    assert params["commit"]["kind"] == "graph_writes_atomic"
    workspace_call = params["ops"][0]["call"]
    agent_call = params["ops"][1]["call"]
    assert workspace_call["kind"] == "workspace_materialize"
    assert workspace_call["candidate"] == "cand_trace2skill_spreadsheetbench_13_1"
    assert agent_call["kind"] == "agent_run"
    assert agent_call["workspace"] == "ws_trace2skill_spreadsheetbench_13_1"
    assert agent_call["tool_policy"]["allow_shell"] is False
    assert agent_call["output"]["kind"] == "files"
    assert agent_call["output"]["paths"] == ["13-1_output.xlsx"]
    assert "workspace.file" in agent_call["input_classes"]
    assert os.environ["LEAVEN_CAPABILITY_TOKEN"] == "secret-token"
    assert os.environ["LEAVEN_ENDPOINT"] == "stdio://trace2skill-spreadsheetbench/worker"

    solved = solve_spreadsheet(run_dir)
    transcript = write_transcript(run_dir, solved)

    print(json.dumps({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {"message": "trace2skill worker completed", "priority": "critical"},
    }), flush=True)
    print(json.dumps({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": extension_result(request["method"], transcript, "completed", solved),
    }, sort_keys=True), flush=True)
"#;
