use std::io::Cursor;

use leaven_public_seam::PublicSeamPackage;
use leaven_seam_runtime::{
    SeamPlanRequest, SeamRuntime, SeamService, SeamServiceError, SeamStageRunRequest,
};
use leaven_seam_stdio::serve_reader_writer;
use serde_json::{Value, json};

#[test]
fn stdio_serves_one_response_per_non_empty_request_line() {
    let runtime = runtime(RejectingService);
    let input = format!(
        "\n{}\nnot json\n{}\n",
        request("leaven/lm.complete", &plan_params()),
        request("leaven/stage.run", &stage_run_request())
    );
    let mut output = Vec::new();

    let report = serve_reader_writer(&runtime, Cursor::new(input), &mut output).unwrap();

    assert_eq!(report.requests, 3);
    let lines = response_lines(output);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["error"]["code"], -32004);
    assert_eq!(lines[1]["error"]["code"], -32700);
    assert_eq!(lines[2]["error"]["code"], -32004);
}

#[test]
fn stdio_returns_valid_stage_run_results_from_the_runtime() {
    let runtime = runtime(StageRunService);
    let input = format!("{}\n", request("leaven/stage.run", &stage_run_request()));
    let mut output = Vec::new();

    let report = serve_reader_writer(&runtime, Cursor::new(input), &mut output).unwrap();

    assert_eq!(report.requests, 1);
    let lines = response_lines(output);
    assert_eq!(lines[0]["result"]["message"], "stage_run_result");
    assert_eq!(lines[0]["result"]["stage_call_id"], "sc_runner_stagerun");
}

fn runtime<S>(service: S) -> SeamRuntime<S> {
    SeamRuntime::from_package(
        PublicSeamPackage::active_from_repo(workspace_root()).unwrap(),
        service,
    )
    .unwrap()
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crate lives under workspace/crates")
        .to_path_buf()
}

struct RejectingService;

impl SeamService for RejectingService {
    fn handle_plan(&self, request: SeamPlanRequest<'_>) -> Result<Value, SeamServiceError> {
        Err(SeamServiceError::unavailable(request.method().as_str()))
    }

    fn handle_stage_run(
        &self,
        _request: SeamStageRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        Err(SeamServiceError::unavailable("leaven/stage.run"))
    }
}

struct StageRunService;

impl SeamService for StageRunService {
    fn handle_plan(&self, request: SeamPlanRequest<'_>) -> Result<Value, SeamServiceError> {
        Err(SeamServiceError::unavailable(request.method().as_str()))
    }

    fn handle_stage_run(
        &self,
        _request: SeamStageRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        Ok(stage_run_result())
    }
}

fn request(method: &str, params: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "req_stdio",
        "method": method,
        "params": params
    })
}

fn plan_params() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_stdio_contract",
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

fn stage_run_request() -> Value {
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_request",
        "stage": "runner",
        "payload": {
            "schema_version": "leaven.stage_payloads.v1",
            "role": "runner",
            "run": "run_stagerun",
            "stage_call_id": "sc_runner_stagerun",
            "candidate": "cand_stagerun_parent",
            "case": "case_stagerun",
            "case_input": {"question": "5 + 7"},
            "target_forbidden": true,
            "capability_fingerprint": "fp_cap_sha256_stagerun"
        }
    })
}

fn stage_run_result() -> Value {
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": "sc_runner_stagerun",
        "output": {
            "kind": "text",
            "summary": "5 + 7 = 12",
            "value": "12",
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"]
        }
    })
}

fn response_lines(output: Vec<u8>) -> Vec<Value> {
    String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
