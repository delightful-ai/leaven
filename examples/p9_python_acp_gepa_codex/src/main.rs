use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use leaven_acp::{AcpProcessCommand, AcpStdioProcessSession, RejectAllEffectHost};
use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, OutputContract,
};
use leaven_agent_codex_cli::{CodexCliApproval, CodexCliConfig, CodexCliRuntime};
use leaven_kernel::{AgentSessionId, BudgetSnapshot};
use leaven_public_seam::{AcpProfileDocument, PublicSeamPackage};
use leaven_workspace::{
    FactoryError, WorkspaceConfig, WorkspaceError, WorkspaceFactory, WorkspacePath,
};
use leaven_workspace_local::LocalWorkspaceFactory;
use serde::Serialize;
use serde_json::{Map, Value, json};

type Result<T> = std::result::Result<T, P9Error>;

const ACP_METHODS: [(&str, &str); 5] = [
    ("leaven/event.emit", "extension"),
    ("leaven/lm.complete", "lm_response"),
    ("leaven/agent.run", "agent_session"),
    ("leaven/proposal.submit_batch", "proposal_batch_receipt"),
    ("leaven/assessment.submit", "assessment_batch_receipt"),
];

#[derive(Debug, thiserror::Error)]
enum P9Error {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    Acp(#[from] leaven_acp::AcpTransportError),
    #[error(transparent)]
    Agent(#[from] leaven_agent::AgentRuntimeError),
    #[error(transparent)]
    PublicSeam(#[from] leaven_public_seam::PublicSeamError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Factory(#[from] FactoryError),
    #[error(transparent)]
    WorkspacePath(#[from] leaven_workspace::WorkspacePathError),
}

#[derive(Clone, Debug)]
struct CliArgs {
    live: bool,
    run_dir: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct CodexProof {
    child_prompt: String,
    child_prompt_path: PathBuf,
    session_path: PathBuf,
}

#[derive(Serialize)]
struct P9Summary {
    schema_version: &'static str,
    live: bool,
    run_dir: String,
    iterations: u64,
    seed_prompt: String,
    child_prompt_path: String,
    codex_session_path: String,
    seed_score: f64,
    child_score: f64,
    accepted_candidate: &'static str,
    acp_methods: Vec<&'static str>,
    acp_request_count: usize,
    acp_observed_requests_path: String,
    proof_limits: Vec<&'static str>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse()?;
    require_live(&args)?;

    let run_dir = args.run_dir.unwrap_or_else(default_run_dir);
    std::fs::create_dir_all(&run_dir)?;

    let seed_prompt = "Always answer 0.".to_owned();
    let codex = run_live_codex(&run_dir, &seed_prompt).await?;
    let seed_score = score_prompt(&seed_prompt);
    let child_score = score_prompt(&codex.child_prompt);
    if child_score <= seed_score {
        return Err(P9Error::Message(format!(
            "live Codex child prompt did not improve the tiny scorer: seed={seed_score} child={child_score}"
        )));
    }

    let acp = run_python_acp_worker(&run_dir, &codex, seed_score, child_score)?;
    let summary = P9Summary {
        schema_version: "p9.python_acp_gepa_codex.summary.v1",
        live: true,
        run_dir: run_dir.display().to_string(),
        iterations: 2,
        seed_prompt,
        child_prompt_path: codex.child_prompt_path.display().to_string(),
        codex_session_path: codex.session_path.display().to_string(),
        seed_score,
        child_score,
        accepted_candidate: "codex_child",
        acp_methods: ACP_METHODS.iter().map(|(method, _)| *method).collect(),
        acp_request_count: acp.request_count,
        acp_observed_requests_path: acp.observed_requests_path.display().to_string(),
        proof_limits: vec![
            "not a Python SDK implementation",
            "not durable Codex agent-kit installation",
            "not Codex hooks",
            "not full GEPA optimizer policy",
        ],
    };
    let summary_path = run_dir.join("result_summary.json");
    std::fs::write(&summary_path, serde_json::to_vec_pretty(&summary)?)?;

    println!("p9_python_acp_gepa_codex_ok=true");
    println!("result_summary={}", summary_path.display());
    println!("seed_score={seed_score:.3}");
    println!("child_score={child_score:.3}");
    println!("acp_request_count={}", acp.request_count);
    Ok(())
}

impl CliArgs {
    fn parse() -> Result<Self> {
        let mut live = false;
        let mut run_dir = None;
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--live" => live = true,
                "--run-dir" => {
                    let Some(path) = args.next() else {
                        return Err(P9Error::Message("--run-dir requires a value".to_owned()));
                    };
                    run_dir = Some(PathBuf::from(path));
                }
                "--help" | "-h" => {
                    println!(
                        "usage: p9_python_acp_gepa_codex --live [--run-dir <path>]\n\
                         requires LEAVEN_P9_LIVE=1 and LEAVEN_CODEX_LIVE=1"
                    );
                    std::process::exit(0);
                }
                other => return Err(P9Error::Message(format!("unknown argument `{other}`"))),
            }
        }
        Ok(Self { live, run_dir })
    }
}

fn require_live(args: &CliArgs) -> Result<()> {
    if !args.live {
        return Err(P9Error::Message(
            "pass --live to acknowledge that P9 runs live Codex and an ACP subprocess".to_owned(),
        ));
    }
    if std::env::var("LEAVEN_P9_LIVE").as_deref() != Ok("1") {
        return Err(P9Error::Message(
            "set LEAVEN_P9_LIVE=1 to run the P9 live proof".to_owned(),
        ));
    }
    if std::env::var("LEAVEN_CODEX_LIVE").as_deref() != Ok("1") {
        return Err(P9Error::Message(
            "set LEAVEN_CODEX_LIVE=1 because Codex execution is part of the P9 gate".to_owned(),
        ));
    }
    Ok(())
}

async fn run_live_codex(run_dir: &Path, seed_prompt: &str) -> Result<CodexProof> {
    let workspace_parent = run_dir.join("codex-workspaces");
    std::fs::create_dir_all(&workspace_parent)?;
    let factory = LocalWorkspaceFactory::new(workspace_parent);
    let mut workspace = factory.allocate(WorkspaceConfig::default()).await?;
    let child_path = WorkspacePath::new("agent/child_prompt.txt")?;
    let seed_path = WorkspacePath::new("agent/seed_prompt.txt")?;
    let session_path = run_dir.join("codex_session.json");
    let durable_child_path = run_dir.join("codex_child_prompt.txt");

    let (child_prompt, session) = {
        let mut view = workspace.view();
        view.write_file(&seed_path, seed_prompt.as_bytes())?;

        let mut instructions = AgentInstructions::task(
            "A Leaven P9 proof harness is testing a tiny prompt optimizer.\n\
             The current seed prompt in agent/seed_prompt.txt always answers 0.\n\
             Create agent/child_prompt.txt with a replacement prompt.\n\
             Requirements:\n\
             - instruct the downstream assistant to add the two integers in inputs like `2 + 3`\n\
             - plain text only\n\
             - no Markdown fences\n\
             - do not edit other files",
        );
        instructions.system = Some(
            "You are contributing to a Leaven live-proof harness. Obey the file contract exactly."
                .to_owned(),
        );

        let request = AgentRunRequest::new(
            instructions,
            OutputContract::Files {
                paths: vec![child_path.clone()],
            },
        );
        let budget = BudgetSnapshot::default();
        let ctx = AgentRunContext::new(AgentSessionId::new(), &budget);
        let metered = codex_runtime().run_session(&mut view, request, ctx).await?;
        let child_prompt = String::from_utf8(view.read_file(&child_path)?)?;
        (child_prompt, metered.value)
    };

    workspace.cleanup().await?;
    std::fs::write(&durable_child_path, &child_prompt)?;
    std::fs::write(&session_path, serde_json::to_vec_pretty(&session)?)?;

    Ok(CodexProof {
        child_prompt,
        child_prompt_path: durable_child_path,
        session_path,
    })
}

fn codex_runtime() -> CodexCliRuntime {
    let mut config = CodexCliConfig::new(codex_bin().to_string_lossy().into_owned());
    "gpt-5.4-mini".clone_into(&mut config.model);
    config.approval = CodexCliApproval::BypassSandboxAndApprovals;
    config.timeout = Some(Duration::from_secs(codex_timeout_secs()));
    CodexCliRuntime::new(config)
}

fn codex_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("LEAVEN_CODEX_BIN") {
        return path.into();
    }
    let home = std::env::var_os("HOME").expect("HOME must be set for Codex binary discovery");
    PathBuf::from(home).join(".bun/bin/codex")
}

fn codex_timeout_secs() -> u64 {
    std::env::var("LEAVEN_P9_CODEX_TIMEOUT_SECS")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(300)
}

struct AcpProof {
    observed_requests_path: PathBuf,
    request_count: usize,
}

fn run_python_acp_worker(
    run_dir: &Path,
    codex: &CodexProof,
    seed_score: f64,
    child_score: f64,
) -> Result<AcpProof> {
    let package = PublicSeamPackage::active_from_repo(workspace_root())?;
    let profile = acp_profile(&package)?;
    let worker = Path::new(env!("CARGO_MANIFEST_DIR")).join("worker/p9_worker.py");
    let observed_requests_path = run_dir.join("acp_observed_requests.jsonl");
    let response_map = response_map(codex, seed_score, child_score);
    let mut session = AcpStdioProcessSession::spawn(
        package,
        profile,
        AcpProcessCommand::new("python3")
            .arg(worker.to_string_lossy())
            .env("LEAVEN_P9_RUN_DIR", run_dir.to_string_lossy())
            .env("LEAVEN_P9_RESPONSE_MAP", response_map.to_string()),
        "secret-token",
        "stdio://worker/session",
        "fp_cap_sha256_p9",
    )?;

    for (method, primary_kind) in ACP_METHODS {
        let response = session.call_extension(
            method,
            &acp_plan_params_for_method(method),
            &RejectAllEffectHost,
        )?;
        if response.method() != method {
            return Err(P9Error::Message(format!(
                "ACP response method mismatch: expected {method}, got {}",
                response.method()
            )));
        }
        if response.primary_kind() != primary_kind {
            return Err(P9Error::Message(format!(
                "ACP response primary mismatch for {method}: expected {primary_kind}, got {}",
                response.primary_kind()
            )));
        }
    }

    let request_count = std::fs::read_to_string(&observed_requests_path)?
        .lines()
        .count();
    Ok(AcpProof {
        observed_requests_path,
        request_count,
    })
}

fn response_map(codex: &CodexProof, seed_score: f64, child_score: f64) -> Value {
    let mut responses = Map::new();
    for (index, (method, _)) in ACP_METHODS.iter().enumerate() {
        let result = match *method {
            "leaven/event.emit" => extension_result(
                method,
                extension_primary("event.emit"),
                write_receipt("emit_run_event", "wrec_event_emit"),
                &["public"],
            ),
            "leaven/lm.complete" => extension_result(
                method,
                lm_response_primary(seed_score, child_score),
                call_receipt("lm_complete", "lmrec_p9"),
                &["completion.raw"],
            ),
            "leaven/agent.run" => extension_result(
                method,
                agent_session_primary(codex),
                call_receipt("agent_run", "agentrec_p9"),
                &["public", "transcript.raw"],
            ),
            "leaven/proposal.submit_batch" => extension_result(
                method,
                proposal_batch_primary(),
                write_receipt("submit_proposal_batch", "wrec_proposal_submit"),
                &["public"],
            ),
            "leaven/assessment.submit" => extension_result(
                method,
                assessment_batch_primary(),
                write_receipt("submit_assessments", "wrec_assessment_submit"),
                &["public"],
            ),
            _ => unreachable!("P9 ACP method table is static"),
        };
        responses.insert(
            format!("leaven-acp-{index}"),
            response_for(method, &format!("leaven-acp-{index}"), result),
        );
    }
    Value::Object(responses)
}

fn response_for(method: &str, id: &str, mut result: Value) -> Value {
    result["method"] = json!(method);
    result["capability_fingerprint"] = json!("__CAPABILITY_FINGERPRINT__");
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn score_prompt(prompt: &str) -> f64 {
    let lower = prompt.to_lowercase();
    if lower.contains("always answer 0") || lower.contains("always answer zero") {
        return 0.0;
    }
    if ["add", "sum", "plus", "integer"]
        .iter()
        .any(|token| lower.contains(token))
    {
        1.0
    } else {
        0.0
    }
}

fn acp_profile(package: &PublicSeamPackage) -> Result<AcpProfileDocument> {
    let value = json!({
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
    });
    Ok(package.validate_acp_profile_document(&value)?)
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

fn acp_plan_params_for_method(method: &str) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": format!("plan_p9_{}", method.replace(['/', '.'], "_")),
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "dry_run"},
        "ops": [{
            "kind": "let",
            "name": "input",
            "expr": {
                "kind": "literal",
                "value": method,
                "data_classes": ["public"]
            }
        }],
        "return": ["input"],
        "commit": {"kind": "no_graph_writes"}
    })
}

fn extension_result(method: &str, primary: Value, receipt: Value, data_classes: &[&str]) -> Value {
    let mut result = json!({
        "method": method,
        "redactions": [],
        "capability_fingerprint": "fp_cap_sha256_p9",
        "data_classes": data_classes
    });
    result["primary"] = primary;
    result["receipts"] = Value::Array(vec![receipt]);
    let schema_version = match result["receipts"][0]["kind"].as_str().unwrap() {
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

fn extension_primary(op: &str) -> Value {
    json!({
        "kind": "extension",
        "namespace": "leaven",
        "op": op,
        "schema_fingerprint": "fp_schema_sha256_p9",
        "payload": {"status": "p9-started"}
    })
}

fn lm_response_primary(seed_score: f64, child_score: f64) -> Value {
    json!({
        "kind": "lm_response",
        "message": {
            "role": "assistant",
            "content": [{
                "kind": "text",
                "text": format!("seed_score={seed_score:.3}; child_score={child_score:.3}; accepted=codex_child")
            }]
        },
        "graph_revision": "rev_p9",
        "cost": {"usd_micro": 42, "lm_calls": 1},
        "data_classes": ["completion.raw"],
        "replayability": "fully_managed",
        "receipt": "lmrec_p9"
    })
}

fn agent_session_primary(_codex: &CodexProof) -> Value {
    json!({
        "kind": "agent_session",
        "status": "completed",
        "transcript_ref": acp_blob_ref("blob_p9_codex_transcript", &["transcript.raw"]),
        "commands": [{
            "argv": ["codex", "exec", "--model", "gpt-5.4-mini"],
            "status": "completed",
            "receipt": "agentrec_p9",
            "stdout_ref": acp_blob_ref("blob_p9_codex_stdout", &["transcript.raw"]),
            "stderr_ref": acp_blob_ref("blob_p9_codex_stderr", &["transcript.raw"])
        }],
        "cost": {"usd_micro": 1000, "agent_calls": 1},
        "graph_revision": "rev_p9",
        "data_classes": ["public", "transcript.raw"],
        "replayability": "fully_managed",
        "receipt": "agentrec_p9"
    })
}

fn proposal_batch_primary() -> Value {
    json!({
        "kind": "proposal_batch_receipt",
        "batch_id": "pb_p9",
        "proposal_ids": ["prop_p9_codex_child"],
        "status": "committed",
        "graph_revision": "rev_p9",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "wrec_proposal_submit"
    })
}

fn assessment_batch_primary() -> Value {
    json!({
        "kind": "assessment_batch_receipt",
        "evaluation_request_id": "evalreq_p9",
        "assessment_ids": ["assess_p9_codex_child"],
        "per_assessment": [
            {
                "assessment": "assess_p9_codex_child",
                "replayability": "fully_managed"
            }
        ],
        "status": "committed",
        "graph_revision": "rev_p9",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "wrec_assessment_submit"
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
        "request_hash": "fp_request_sha256_p9",
        "result_hash": "fp_result_sha256_p9",
        "runtime_fingerprint": "fp_runtime_sha256_p9",
        "status": "succeeded"
    });
    match call_kind {
        "lm_complete" => value["cost"] = json!({"usd_micro": 42, "lm_calls": 1}),
        "agent_run" => value["cost"] = json!({"usd_micro": 1000, "agent_calls": 1}),
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
        "request_hash": "fp_request_sha256_p9",
        "result_hash": "fp_result_sha256_p9",
        "base_revision": "rev_p9",
        "committed_revision": "rev_p9",
        "status": "succeeded"
    });
    match write_kind {
        "submit_proposal_batch" => {
            value["proposal_batch_id"] = json!("pb_p9");
            value["proposal_ids"] = json!(["prop_p9_codex_child"]);
        }
        "submit_assessments" => {
            value["evaluation_request_id"] = json!("evalreq_p9");
            value["assessment_ids"] = json!(["assess_p9_codex_child"]);
            value["request_hash"] = json!(prefixed_jcs_hash(
                "fp_request_sha256_",
                &json!({
                    "schema_version": "leaven.submit_assessments_request.v1",
                    "evaluation_request_id": "evalreq_p9",
                    "assessment_ids": ["assess_p9_codex_child"]
                }),
            ));
        }
        "emit_run_event" => {
            value["event_id"] = json!("event_p9");
        }
        other => panic!("unexpected write kind {other}"),
    }
    value
}

fn prefixed_jcs_hash(prefix: &str, value: &Value) -> String {
    format!(
        "{prefix}{}",
        jcs_canonicalize::sha256_jcs_hex(value).unwrap()
    )
}

fn default_run_dir() -> PathBuf {
    PathBuf::from("tmp/p9_python_acp_gepa_codex")
        .join(Utc::now().format("%Y%m%dT%H%M%SZ").to_string())
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiny_prompt_scorer_requires_arithmetic_instruction() {
        assert!((score_prompt("Always answer 0.") - 0.0).abs() < f64::EPSILON);
        assert!(
            (score_prompt("Add the two integers and answer with the sum.") - 1.0).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn response_map_uses_sequential_acp_request_ids() {
        let proof = CodexProof {
            child_prompt: "Add the two integers.".to_owned(),
            child_prompt_path: PathBuf::from("tmp/child.txt"),
            session_path: PathBuf::from("tmp/session.json"),
        };
        let responses = response_map(&proof, 0.0, 1.0);
        for index in 0..ACP_METHODS.len() {
            assert!(responses.get(format!("leaven-acp-{index}")).is_some());
        }
    }
}
