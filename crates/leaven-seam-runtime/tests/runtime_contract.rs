use std::sync::{Arc, Mutex};

use leaven_public_seam::PublicSeamPackage;
use leaven_seam_runtime::{
    JsonRpcErrorCode, RejectingSeamService, SeamOptimizeRunRequest, SeamPlanRequest, SeamRuntime,
    SeamService, SeamServiceError, SeamStageRunRequest,
};
use serde_json::{Value, json};

#[test]
fn runtime_exposes_every_locked_profile_method_and_validates_before_dispatch() {
    let service = RecordingService::default();
    let runtime = runtime(service.clone());
    let methods = runtime.methods().map(str::to_owned).collect::<Vec<_>>();
    assert!(!methods.is_empty());

    for (index, method) in methods.iter().enumerate() {
        let response = runtime.handle_value(&request_for(index, method));
        assert_eq!(
            error_code(response.value()),
            JsonRpcErrorCode::MethodUnavailable
        );
    }

    assert_eq!(service.called_methods(), methods);
}

#[test]
fn runtime_rejects_unknown_methods_without_reaching_service() {
    let service = RecordingService::default();
    let runtime = runtime(service.clone());

    let response = runtime.handle_value(&json!({
        "jsonrpc": "2.0",
        "id": "req_unknown",
        "method": "leaven/not.real",
        "params": plan_params()
    }));

    assert_eq!(
        error_code(response.value()),
        JsonRpcErrorCode::MethodNotFound
    );
    assert!(service.called_methods().is_empty());
}

#[test]
fn runtime_error_objects_omit_untyped_data() {
    let service = RecordingService::default();
    let runtime = runtime(service);

    let response = runtime.handle_value(&json!({
        "jsonrpc": "2.0",
        "id": "req_unknown",
        "method": "leaven/not.real",
        "params": plan_params()
    }));

    assert_eq!(
        error_code(response.value()),
        JsonRpcErrorCode::MethodNotFound
    );
    assert!(response.value()["error"].get("data").is_none());
}

#[test]
fn runtime_delivers_typed_plan_document_to_service() {
    let service = PlanDocumentRecordingService::default();
    let runtime = runtime(service.clone());

    let response = runtime.handle_value(&request_for(0, "leaven/lm.complete"));

    assert_eq!(
        error_code(response.value()),
        JsonRpcErrorCode::MethodUnavailable
    );
    assert_eq!(
        service.plan_facts(),
        vec![("leaven/lm.complete".to_owned(), "dry_run".to_owned(), 1)]
    );
}

#[test]
fn runtime_validates_stage_run_success_before_returning_jsonrpc_result() {
    let runtime = runtime(StageRunService::Valid);

    let response = runtime.handle_value(&json!({
        "jsonrpc": "2.0",
        "id": "req_stage",
        "method": "leaven/stage.run",
        "params": stage_run_request()
    }));

    assert!(!response.is_error(), "{:#}", response.value());
    assert_eq!(response.value()["result"]["message"], "stage_run_result");
    assert_eq!(
        response.value()["result"]["stage_call_id"],
        "sc_runner_stagerun"
    );
}

#[test]
fn runtime_preserves_numeric_jsonrpc_ids_with_typed_id_carrier() {
    let runtime = runtime(StageRunService::Valid);

    let response = runtime.handle_value(&json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "leaven/stage.run",
        "params": stage_run_request()
    }));

    assert!(!response.is_error(), "{:#}", response.value());
    assert_eq!(response.value()["id"], 42);
}

#[test]
fn runtime_rejects_malformed_stage_run_service_results() {
    let runtime = runtime(StageRunService::Invalid);

    let response = runtime.handle_value(&json!({
        "jsonrpc": "2.0",
        "id": "req_stage",
        "method": "leaven/stage.run",
        "params": stage_run_request()
    }));

    assert_eq!(
        error_code(response.value()),
        JsonRpcErrorCode::InvalidResult
    );
}

#[test]
fn runtime_validates_optimize_run_request_and_result_around_service() {
    let runtime = runtime(OptimizeRunService::Valid);

    let response = runtime.handle_value(&json!({
        "jsonrpc": "2.0",
        "id": "req_optimize",
        "method": "leaven/optimize.run",
        "params": optimize_run_request()
    }));

    assert!(!response.is_error(), "{:#}", response.value());
    assert_eq!(response.value()["id"], "req_optimize");
    assert_eq!(
        response.value()["result"]["best"]["candidate"],
        "cand_optimize_child"
    );
}

#[test]
fn runtime_rejects_invalid_optimize_run_request_before_dispatch() {
    let service = RecordingService::default();
    let runtime = runtime(service.clone());

    // Drop the required cases array: the request must fail validation before the
    // service is ever consulted.
    let mut params = optimize_run_request();
    params.as_object_mut().unwrap().remove("cases");

    let response = runtime.handle_value(&json!({
        "jsonrpc": "2.0",
        "id": "req_optimize_bad",
        "method": "leaven/optimize.run",
        "params": params
    }));

    assert_eq!(
        error_code(response.value()),
        JsonRpcErrorCode::InvalidRequest
    );
    assert!(service.called_methods().is_empty());
}

#[test]
fn runtime_rejects_malformed_optimize_run_service_results() {
    let runtime = runtime(OptimizeRunService::Invalid);

    let response = runtime.handle_value(&json!({
        "jsonrpc": "2.0",
        "id": "req_optimize",
        "method": "leaven/optimize.run",
        "params": optimize_run_request()
    }));

    assert_eq!(
        error_code(response.value()),
        JsonRpcErrorCode::InvalidResult
    );
}

#[test]
fn runtime_routes_optimize_run_to_the_injected_service() {
    let service = RecordingService::default();
    let runtime = runtime(service.clone());

    let response = runtime.handle_value(&json!({
        "jsonrpc": "2.0",
        "id": "req_optimize",
        "method": "leaven/optimize.run",
        "params": optimize_run_request()
    }));

    assert_eq!(
        error_code(response.value()),
        JsonRpcErrorCode::MethodUnavailable
    );
    assert_eq!(service.called_methods(), vec!["leaven/optimize.run"]);
}

#[test]
fn rejecting_service_exposes_the_whole_seam_as_unimplemented() {
    let runtime = runtime(RejectingSeamService);
    let method = runtime.methods().next().unwrap().to_owned();

    let response = runtime.handle_value(&request_for(0, &method));

    assert_eq!(
        error_code(response.value()),
        JsonRpcErrorCode::MethodUnavailable
    );
    assert!(
        response.value()["error"]["message"]
            .as_str()
            .unwrap()
            .contains(&method)
    );
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

#[derive(Clone, Default)]
struct RecordingService {
    called_methods: Arc<Mutex<Vec<String>>>,
}

impl RecordingService {
    fn called_methods(&self) -> Vec<String> {
        self.called_methods.lock().unwrap().clone()
    }
}

impl SeamService for RecordingService {
    fn handle_plan(&self, request: SeamPlanRequest<'_>) -> Result<Value, SeamServiceError> {
        self.called_methods
            .lock()
            .unwrap()
            .push(request.method().as_str().to_owned());
        Err(SeamServiceError::unavailable(request.method().as_str()))
    }

    fn handle_stage_run(
        &self,
        _request: SeamStageRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        self.called_methods
            .lock()
            .unwrap()
            .push("leaven/stage.run".to_owned());
        Err(SeamServiceError::unavailable("leaven/stage.run"))
    }

    fn handle_optimize_run(
        &self,
        _request: SeamOptimizeRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        self.called_methods
            .lock()
            .unwrap()
            .push("leaven/optimize.run".to_owned());
        Err(SeamServiceError::unavailable("leaven/optimize.run"))
    }
}

#[derive(Clone, Default)]
struct PlanDocumentRecordingService {
    plan_facts: Arc<Mutex<Vec<(String, String, usize)>>>,
}

impl PlanDocumentRecordingService {
    fn plan_facts(&self) -> Vec<(String, String, usize)> {
        self.plan_facts.lock().unwrap().clone()
    }
}

impl SeamService for PlanDocumentRecordingService {
    fn handle_plan(&self, request: SeamPlanRequest<'_>) -> Result<Value, SeamServiceError> {
        self.plan_facts.lock().unwrap().push((
            request.method().as_str().to_owned(),
            request.plan_document().mode_kind().to_owned(),
            request.plan_document().return_names().len(),
        ));
        Err(SeamServiceError::unavailable(request.method().as_str()))
    }

    fn handle_stage_run(
        &self,
        _request: SeamStageRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        Err(SeamServiceError::unavailable("leaven/stage.run"))
    }

    fn handle_optimize_run(
        &self,
        _request: SeamOptimizeRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        Err(SeamServiceError::unavailable("leaven/optimize.run"))
    }
}

enum StageRunService {
    Valid,
    Invalid,
}

impl SeamService for StageRunService {
    fn handle_plan(&self, request: SeamPlanRequest<'_>) -> Result<Value, SeamServiceError> {
        Err(SeamServiceError::unavailable(request.method().as_str()))
    }

    fn handle_stage_run(
        &self,
        _request: SeamStageRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        Ok(match self {
            Self::Valid => stage_run_result(),
            Self::Invalid => json!({
                "schema_version": "leaven.stage_run.v1",
                "message": "stage_run_result",
                "stage": "runner"
            }),
        })
    }

    fn handle_optimize_run(
        &self,
        _request: SeamOptimizeRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        Err(SeamServiceError::unavailable("leaven/optimize.run"))
    }
}

enum OptimizeRunService {
    Valid,
    Invalid,
}

impl SeamService for OptimizeRunService {
    fn handle_plan(&self, request: SeamPlanRequest<'_>) -> Result<Value, SeamServiceError> {
        Err(SeamServiceError::unavailable(request.method().as_str()))
    }

    fn handle_stage_run(
        &self,
        _request: SeamStageRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        Err(SeamServiceError::unavailable("leaven/stage.run"))
    }

    fn handle_optimize_run(
        &self,
        request: SeamOptimizeRunRequest<'_>,
    ) -> Result<Value, SeamServiceError> {
        // The request must validate before reaching the service.
        assert_eq!(request.document().run_id(), "run_optimize");
        Ok(match self {
            Self::Valid => optimize_run_result(),
            // Best candidate not present in the frontier: the runtime must reject
            // this as an invalid result rather than serialize it.
            Self::Invalid => {
                let mut result = optimize_run_result();
                result["best"]["candidate"] = json!("cand_optimize_phantom");
                result
            }
        })
    }
}

fn request_for(index: usize, method: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": format!("req_{index}"),
        "method": method,
        "params": if method == "leaven/stage.run" {
            stage_run_request()
        } else {
            plan_params()
        }
    })
}

fn plan_params() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_runtime_contract",
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

fn optimize_run_request() -> Value {
    json!({
        "schema_version": "leaven.optimize_run.v1",
        "message": "optimize_run_request",
        "run_id": "run_optimize",
        "seed": optimize_artifact(),
        "cases": [{
            "case": "case_optimize_1",
            "input": {"question": "2 + 2"},
            "target": {"answer": "4"},
            "split": "train"
        }],
        "optimizer": {
            "max_metric_calls": 30,
            "population_size": 4,
            "objective": "instance"
        },
        "reflection": {"kind": "lm", "model": "gpt-5.4-mini"},
        "capability_fingerprint": "fp_cap_sha256_optimize"
    })
}

fn optimize_run_result() -> Value {
    json!({
        "schema_version": "leaven.optimize_run.v1",
        "message": "optimize_run_result",
        "best": {
            "candidate": "cand_optimize_child",
            "parent": "cand_optimize_seed",
            "score": 0.75,
            "artifact": optimize_artifact()
        },
        "frontier": [{
            "candidate": "cand_optimize_child",
            "parent": "cand_optimize_seed",
            "score": 0.75,
            "artifact": optimize_artifact()
        }],
        "iterations": 1,
        "metric_calls_used": 12,
        "cost": {"usd_micro": 1500, "lm_calls": 6},
        "run": {"run": "run_optimize", "revision": "rev_optimize_final"},
        "applied_proposals": ["wrec_optimize_batch_1"]
    })
}

fn optimize_artifact() -> Value {
    json!({
        "artifact_type": "prompt",
        "artifact_schema": "fp_schema_sha256_prompt",
        "artifact": {"template": "Answer the question: {{question}}"}
    })
}

fn error_code(value: &Value) -> JsonRpcErrorCode {
    match value["error"]["code"].as_i64().unwrap() {
        -32700 => JsonRpcErrorCode::ParseError,
        -32600 => JsonRpcErrorCode::InvalidRequest,
        -32601 => JsonRpcErrorCode::MethodNotFound,
        -32004 => JsonRpcErrorCode::MethodUnavailable,
        -32005 => JsonRpcErrorCode::InvalidResult,
        other => panic!("unexpected JSON-RPC error code {other}: {value:#}"),
    }
}
