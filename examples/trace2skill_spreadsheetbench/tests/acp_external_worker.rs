use std::{
    fs,
    path::{Path, PathBuf},
};

use leaven_acp::{AcpProcessCommand, AcpStdioProcessSession};
use leaven_public_seam::{AcpProfileDocument, PublicSeamPackage};
use serde_json::{Value, json};
use tempfile::TempDir;
use trace2skill_spreadsheetbench::{
    Trace2SkillOneCaseComparisonInput, Trace2SkillOneCaseInput, Trace2SkillOneCaseRunInput,
    Trace2SkillOneCaseRunScoringInput, compare_trace2skill_one_case_answer,
    prepare_trace2skill_one_case_run, score_trace2skill_one_case_run,
};

#[test]
fn acp_external_python_worker_solves_real_spreadsheetbench_case_and_scores_run() {
    let fixture = ExactCaseFixture::new();
    let temp = TempDir::new().unwrap();
    let run_dir = temp.path().join("run");
    let prepared = prepare_trace2skill_one_case_run(Trace2SkillOneCaseRunInput {
        case: fixture.case_input(),
        run_dir: &run_dir,
        output_workbook: None,
    })
    .unwrap();

    let before = compare_trace2skill_one_case_answer(Trace2SkillOneCaseComparisonInput {
        case_file: &fixture.case_file,
        candidate_workbook: &prepared.init_workbook.path,
        golden_workbook: &prepared.golden_workbook.path,
    })
    .unwrap();
    assert!(
        before.score < 1.0,
        "the initial workbook must be a real unsolved benchmark input"
    );

    let package = package();
    let profile = profile(&package);
    let script = write_worker_script();
    let mut session = spawn_worker(&package, &profile, &script, &run_dir, "solve");

    let response = session
        .call_extension("leaven/agent.run", &agent_run_plan_params())
        .unwrap();
    assert_eq!(response.method(), "leaven/agent.run");
    assert_eq!(response.primary_kind(), "agent_session");
    assert!(prepared.output_workbook.exists());
    let transcript_file = run_dir.join("agent_transcript.md");
    assert!(transcript_file.exists());

    drop(session);
    let scored = score_trace2skill_one_case_run(Trace2SkillOneCaseRunScoringInput {
        run_dir: &run_dir,
        model_id: "local-openpyxl-trace2skill-agent",
        transcript_file: &transcript_file,
    })
    .unwrap();

    assert!(scored.score_report.passed);
    assert_eq!(
        scored.score_report.matched_cells,
        scored.score_report.total_cells
    );
    assert_eq!(scored.score_report.score, 1.0);
    assert!(scored.score_report.score > before.score);

    let transcript = fs::read_to_string(transcript_file).unwrap();
    assert!(transcript.contains("ACTION: read RANGES"));
    assert!(transcript.contains("ACTION: grouped by DATE and REF"));
    assert!(transcript.contains("ACTION: wrote LISTS DATA and OPERATION sections"));
}

#[test]
fn acp_external_python_worker_success_without_workbook_does_not_clear_benchmark_run() {
    let fixture = ExactCaseFixture::new();
    let temp = TempDir::new().unwrap();
    let run_dir = temp.path().join("run");
    let prepared = prepare_trace2skill_one_case_run(Trace2SkillOneCaseRunInput {
        case: fixture.case_input(),
        run_dir: &run_dir,
        output_workbook: None,
    })
    .unwrap();

    let package = package();
    let profile = profile(&package);
    let script = write_worker_script();
    let mut session = spawn_worker(&package, &profile, &script, &run_dir, "fake_success");

    let response = session
        .call_extension("leaven/agent.run", &agent_run_plan_params())
        .unwrap();
    assert_eq!(response.primary_kind(), "agent_session");
    assert!(
        !prepared.output_workbook.exists(),
        "a valid ACP success envelope is not enough to fake benchmark completion"
    );
    let transcript_file = run_dir.join("agent_transcript.md");
    assert!(transcript_file.exists());
    assert!(
        score_trace2skill_one_case_run(Trace2SkillOneCaseRunScoringInput {
            run_dir: &run_dir,
            model_id: "fake-acp-worker",
            transcript_file: &transcript_file,
        })
        .is_err()
    );
}

struct ExactCaseFixture {
    case_file: PathBuf,
    spreadsheet_dir: PathBuf,
    system_prompt: PathBuf,
    released_skill: PathBuf,
}

impl ExactCaseFixture {
    fn new() -> Self {
        let repo = workspace_root();
        Self {
            case_file: repo.join(
                "tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/dataset_first_case.json",
            ),
            spreadsheet_dir: repo.join(
                "tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1",
            ),
            system_prompt: repo.join(
                "tmp/repros/trace2skill-upstream/spreadsheet_agent/system_prompt/cli_skill_preloaded_full_system_v1.txt",
            ),
            released_skill: repo.join(
                "tmp/repros/trace2skill-upstream/released_skills/trace2skill-xlsx-35B-combined/SKILL.md",
            ),
        }
    }

    fn case_input(&self) -> Trace2SkillOneCaseInput<'_> {
        Trace2SkillOneCaseInput {
            case_file: &self.case_file,
            spreadsheet_dir: &self.spreadsheet_dir,
            system_prompt_file: &self.system_prompt,
            released_skill_file: &self.released_skill,
        }
    }
}

fn spawn_worker(
    package: &PublicSeamPackage,
    profile: &AcpProfileDocument,
    script: &Path,
    run_dir: &Path,
    mode: &str,
) -> AcpStdioProcessSession {
    AcpStdioProcessSession::spawn(
        package.clone(),
        profile.clone(),
        AcpProcessCommand::new("uv")
            .arg("run")
            .arg("--with")
            .arg("openpyxl")
            .arg("python")
            .arg(script.to_str().unwrap())
            .env("LEAVEN_TRACE2SKILL_RUN_DIR", run_dir.to_str().unwrap())
            .env("LEAVEN_TRACE2SKILL_WORKER_MODE", mode),
        "secret-token",
        "stdio://trace2skill-spreadsheetbench/worker",
        "fp_cap_sha256_acp",
    )
    .unwrap()
}

fn write_worker_script() -> PathBuf {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("worker.py");
    fs::write(&path, PYTHON_WORKER).unwrap();
    std::mem::forget(dir);
    path
}

fn package() -> PublicSeamPackage {
    PublicSeamPackage::active_from_repo(workspace_root()).unwrap()
}

fn profile(package: &PublicSeamPackage) -> AcpProfileDocument {
    package
        .validate_acp_profile_document(&acp_profile())
        .unwrap()
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

fn locked_profile_methods() -> Vec<Value> {
    vec![
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

fn agent_run_plan_params() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_trace2skill_spreadsheetbench_acp",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "dry_run"},
        "ops": [{
            "kind": "let",
            "name": "task",
            "expr": {
                "kind": "literal",
                "value": "trace2skill_spreadsheetbench_verified_13_1",
                "data_classes": ["public"]
            }
        }],
        "return": ["task"],
        "commit": {"kind": "no_graph_writes"}
    })
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
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
    ordered = []
    for key, amount in sorted(groups.items()):
        ordered.append((first_dates[key], key[1], amount))
    return ordered


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


def primary(transcript, status):
    return {
        "kind": "agent_session",
        "status": status,
        "transcript_ref": {
            "kind": "blob_ref",
            "id": "blob_trace2skill_transcript",
            "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "bytes": transcript.stat().st_size,
            "data_classes": ["transcript.raw"],
        },
        "commands": [{
            "argv": ["python", "trace2skill_spreadsheetbench_solver"],
            "status": status,
            "receipt": "agentrec_trace2skill",
            "stdout_ref": {
                "kind": "blob_ref",
                "id": "blob_trace2skill_stdout",
                "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "bytes": 0,
                "data_classes": ["transcript.raw"],
            },
            "stderr_ref": {
                "kind": "blob_ref",
                "id": "blob_trace2skill_stderr",
                "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "bytes": 0,
                "data_classes": ["transcript.raw"],
            },
        }],
        "cost": {"usd_micro": 0, "agent_calls": 1},
        "graph_revision": "rev_trace2skill_acp",
        "data_classes": ["public", "transcript.raw"],
        "replayability": "fully_managed",
        "receipt": "agentrec_trace2skill",
    }


def extension_result(method, transcript, status):
    value = primary(transcript, status)
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
        "data_classes": ["public", "transcript.raw"],
    }


run_dir = Path(os.environ["LEAVEN_TRACE2SKILL_RUN_DIR"])
mode = os.environ["LEAVEN_TRACE2SKILL_WORKER_MODE"]

for line in sys.stdin:
    request = json.loads(line)
    assert request["jsonrpc"] == "2.0"
    assert request["method"] == "leaven/agent.run"
    assert request["params"]["schema_version"] == "leaven.plan.v1"
    assert request["params"]["ops"][0]["expr"]["value"] == "trace2skill_spreadsheetbench_verified_13_1"
    assert os.environ["LEAVEN_CAPABILITY_TOKEN"] == "secret-token"
    assert os.environ["LEAVEN_ENDPOINT"] == "stdio://trace2skill-spreadsheetbench/worker"

    if mode == "solve":
        solved = solve_spreadsheet(run_dir)
        transcript = write_transcript(run_dir, solved)
    else:
        transcript = run_dir / "agent_transcript.md"
        transcript.write_text("ACTION: claimed success without writing workbook\n", encoding="utf-8")

    print(json.dumps({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {"message": f"trace2skill worker mode {mode} completed", "priority": "critical"},
    }), flush=True)
    print(json.dumps({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": extension_result(request["method"], transcript, "completed"),
    }, sort_keys=True), flush=True)
"#;
