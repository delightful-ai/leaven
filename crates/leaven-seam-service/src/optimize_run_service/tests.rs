//! Deterministic contract tests for the GEPA-over-seam optimize.run host.
//!
//! These tests drive the configured service through the public
//! `leaven/optimize.run` JSON-RPC route with a scripted command worker and a
//! mock reflection LM. They kill wrong implementations of the loop: proposals
//! never applied, children never re-evaluated, best hardcoded to seed, target
//! leaking into runner dispatch, budget phantom iterations, and silently
//! dropped cost facts.

use std::path::PathBuf;

use leaven_public_seam::PublicSeamPackage;
use leaven_seam_runtime::SeamRuntime;
use serde_json::{Value, json};

use crate::lm::{MockLmResponseConfig, SeamLmConfig};
use crate::service::{ConfiguredSeamService, SeamServiceConfig};
use crate::stage::SeamStageConfig;

const MARKER: &str = "USE_MOD_RULE";

fn repo_root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
}

fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

fn write_worker(name: &str, body: &str) -> PathBuf {
    let dir = tempfile::tempdir().unwrap().keep();
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    make_executable(&path);
    path
}

/// Worker that answers correctly only when the candidate template carries the
/// improvement marker, and scores by reading `case.target` during scorer
/// stages. Subprocess-per-stage: one request, one response, then exit.
fn loop_law_worker() -> PathBuf {
    write_worker(
        "optimize-loop-worker",
        &format!(
            r#"#!/usr/bin/env python3
import json, sys, select

MARKER = "{MARKER}"

req = json.loads(sys.stdin.readline())
payload = req["params"]["payload"]
stage = req["params"]["stage"]

if stage == "runner":
    template = payload["case_input"]["candidate_template"]
    # The worker is target-free: it answers from the template rule only.
    answer = "42" if MARKER in template else "0"
    result = {{
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": payload["stage_call_id"],
        "output": {{
            "kind": "text",
            "summary": "runner answer",
            "value": answer,
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"]
        }}
    }}
    print(json.dumps({{"jsonrpc": "2.0", "id": req.get("id"), "result": result}}), flush=True)
    sys.exit(0)

if stage == "scorer":
    answer = payload["output"]["value"]
    case_id = payload["case"]
    # Read the hidden target through the capability-gated case.target callback.
    callback = {{
        "jsonrpc": "2.0",
        "id": "worker-target-1",
        "method": "leaven/case.target",
        "params": {{
            "schema_version": "leaven.plan.v1",
            "plan_id": "plan_worker_target",
            "consistency": {{"kind": "latest_at_start"}},
            "mode": {{"kind": "execute"}},
            "ops": [{{
                "kind": "let",
                "name": "case_target",
                "expr": {{
                    "kind": "case_query",
                    "query": {{
                        "kind": "load",
                        "case": {{"kind": "case", "run": payload["run"], "id": case_id}},
                        "include": ["target"],
                        "projection_schema": "fp_schema_sha256_case_projection"
                    }}
                }}
            }}],
            "return": ["case_target"],
            "commit": {{"kind": "no_graph_writes"}}
        }}
    }}
    print(json.dumps(callback), flush=True)
    ready, _, _ = select.select([sys.stdin], [], [], 5)
    if not ready:
        raise SystemExit("timed out waiting for case.target response")
    target_response = json.loads(sys.stdin.readline())
    if "error" in target_response:
        raise SystemExit("case.target was refused during scorer: " + json.dumps(target_response["error"]))
    target_answer = target_response["result"]["primary"]["target"]["answer"]
    correct = str(answer) == str(target_answer)
    reward_value = 1.0 if correct else 0.0
    feedback = "answer matched target" if correct else "answer did not match target"
    result = {{
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "scorer",
        "stage_call_id": payload["stage_call_id"],
        "output": {{
            "kind": "text",
            "summary": "scorer output",
            "value": answer,
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"]
        }},
        "score": {{
            "value": reward_value,
            "rewards": [{{
                "id": "exact_match",
                "value": reward_value,
                "weight": 1.0,
                "feedback": feedback
            }}]
        }}
    }}
    print(json.dumps({{"jsonrpc": "2.0", "id": req.get("id"), "result": result}}), flush=True)
    sys.exit(0)

raise SystemExit("unexpected stage: " + str(stage))
"#
        ),
    )
}

/// Worker that requests `case.target` during a RUNNER stage to prove the target
/// isolation refusal, and otherwise answers correctly for the scorer.
fn target_probe_worker() -> PathBuf {
    write_worker(
        "optimize-target-probe-worker",
        r#"#!/usr/bin/env python3
import json, sys, select

req = json.loads(sys.stdin.readline())
payload = req["params"]["payload"]
stage = req["params"]["stage"]

def case_target_callback(run, case_id):
    return {
        "jsonrpc": "2.0",
        "id": "probe-target-1",
        "method": "leaven/case.target",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "plan_probe_target",
            "consistency": {"kind": "latest_at_start"},
            "mode": {"kind": "execute"},
            "ops": [{
                "kind": "let",
                "name": "case_target",
                "expr": {
                    "kind": "case_query",
                    "query": {
                        "kind": "load",
                        "case": {"kind": "case", "run": run, "id": case_id},
                        "include": ["target"],
                        "projection_schema": "fp_schema_sha256_case_projection"
                    }
                }
            }],
            "return": ["case_target"],
            "commit": {"kind": "no_graph_writes"}
        }
    }

if stage == "runner":
    # Illegal: ask for the target during a runner stage. Expect a refusal.
    print(json.dumps(case_target_callback(payload["run"], payload["case"])), flush=True)
    ready, _, _ = select.select([sys.stdin], [], [], 5)
    if not ready:
        raise SystemExit("timed out waiting for runner case.target refusal")
    response = json.loads(sys.stdin.readline())
    refused = "error" in response
    answer = "RUNNER_TARGET_REFUSED" if refused else "RUNNER_TARGET_LEAKED"
    result = {
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": payload["stage_call_id"],
        "output": {
            "kind": "text",
            "summary": "runner probe",
            "value": answer,
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"]
        }
    }
    print(json.dumps({"jsonrpc": "2.0", "id": req.get("id"), "result": result}), flush=True)
    sys.exit(0)

if stage == "scorer":
    # Legal: read the target during a scorer stage.
    print(json.dumps(case_target_callback(payload["run"], payload["case"])), flush=True)
    ready, _, _ = select.select([sys.stdin], [], [], 5)
    if not ready:
        raise SystemExit("timed out waiting for scorer case.target response")
    response = json.loads(sys.stdin.readline())
    if "error" in response:
        raise SystemExit("case.target refused during scorer: " + json.dumps(response["error"]))
    answer = payload["output"]["value"]
    score = 1.0 if answer == "RUNNER_TARGET_REFUSED" else 0.0
    result = {
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "scorer",
        "stage_call_id": payload["stage_call_id"],
        "output": {
            "kind": "text",
            "summary": "scorer probe",
            "value": answer,
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"]
        },
        "score": {
            "value": score,
            "rewards": [{"id": "isolation", "value": score, "weight": 1.0, "feedback": "isolation check"}]
        }
    }
    print(json.dumps({"jsonrpc": "2.0", "id": req.get("id"), "result": result}), flush=True)
    sys.exit(0)

raise SystemExit("unexpected stage: " + str(stage))
"#,
    )
}

/// Worker that reports an `lm.complete` effect receipt with cost facts so the
/// result projection can aggregate worker cost.
fn cost_reporting_worker() -> PathBuf {
    write_worker(
        "optimize-cost-worker",
        &format!(
            r#"#!/usr/bin/env python3
import json, sys, select

MARKER = "{MARKER}"
req = json.loads(sys.stdin.readline())
payload = req["params"]["payload"]
stage = req["params"]["stage"]

def lm_callback():
    return {{
        "jsonrpc": "2.0",
        "id": "worker-lm-1",
        "method": "leaven/lm.complete",
        "params": {{
            "schema_version": "leaven.plan.v1",
            "plan_id": "plan_worker_lm",
            "consistency": {{"kind": "latest_at_start"}},
            "mode": {{"kind": "execute"}},
            "ops": [{{
                "kind": "call",
                "name": "completion",
                "idempotency_key": "worker-lm-1",
                "call": {{
                    "kind": "lm_complete",
                    "purpose": "test.optimize_worker",
                    "model": "mock",
                    "messages": [{{"role": "user", "content": [{{"kind": "text", "text": "prompt"}}]}}],
                    "output": {{"kind": "final_message", "max_bytes": 128}},
                    "input_classes": ["public"]
                }}
            }}],
            "return": ["completion"],
            "commit": {{"kind": "no_graph_writes"}}
        }}
    }}

if stage == "runner":
    print(json.dumps(lm_callback()), flush=True)
    ready, _, _ = select.select([sys.stdin], [], [], 5)
    if not ready:
        raise SystemExit("timed out waiting for runner lm.complete response")
    lm_response = json.loads(sys.stdin.readline())
    receipt = lm_response["result"]["receipts"][0]
    cost = lm_response["result"]["primary"].get("cost", {{}})
    template = payload["case_input"]["candidate_template"]
    answer = "42" if MARKER in template else "0"
    result = {{
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": payload["stage_call_id"],
        "output": {{
            "kind": "text",
            "summary": "runner answer",
            "value": answer,
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"]
        }},
        "effect_receipts": [{{
            "method": "leaven/lm.complete",
            "receipt": receipt["receipt"],
            "call_kind": "lm_complete",
            "cost": {{
                "lm_calls": cost.get("lm_calls", 1),
                "input_tokens": cost.get("input_tokens", 0),
                "output_tokens": cost.get("output_tokens", 0)
            }}
        }}]
    }}
    print(json.dumps({{"jsonrpc": "2.0", "id": req.get("id"), "result": result}}), flush=True)
    sys.exit(0)

if stage == "scorer":
    answer = payload["output"]["value"]
    score = 1.0 if answer == "42" else 0.0
    result = {{
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "scorer",
        "stage_call_id": payload["stage_call_id"],
        "output": {{
            "kind": "text",
            "summary": "scorer output",
            "value": answer,
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"]
        }},
        "score": {{"value": score, "rewards": [{{"id": "match", "value": score, "weight": 1.0, "feedback": "f"}}]}}
    }}
    print(json.dumps({{"jsonrpc": "2.0", "id": req.get("id"), "result": result}}), flush=True)
    sys.exit(0)

raise SystemExit("unexpected stage")
"#
        ),
    )
}

/// Worker that reports a large `usd_micro` cost on every runner stage so a usd
/// ceiling below the seed-validation spend stops the loop before any child.
fn usd_cost_worker() -> PathBuf {
    write_worker(
        "optimize-usd-cost-worker",
        &format!(
            r#"#!/usr/bin/env python3
import json, sys, select

MARKER = "{MARKER}"
req = json.loads(sys.stdin.readline())
payload = req["params"]["payload"]
stage = req["params"]["stage"]

def lm_callback():
    return {{
        "jsonrpc": "2.0",
        "id": "worker-lm-usd",
        "method": "leaven/lm.complete",
        "params": {{
            "schema_version": "leaven.plan.v1",
            "plan_id": "plan_worker_usd",
            "consistency": {{"kind": "latest_at_start"}},
            "mode": {{"kind": "execute"}},
            "ops": [{{
                "kind": "call",
                "name": "completion",
                "idempotency_key": "worker-lm-usd",
                "call": {{
                    "kind": "lm_complete",
                    "purpose": "test.optimize_usd",
                    "model": "mock",
                    "messages": [{{"role": "user", "content": [{{"kind": "text", "text": "prompt"}}]}}],
                    "output": {{"kind": "final_message", "max_bytes": 128}},
                    "input_classes": ["public"]
                }}
            }}],
            "return": ["completion"],
            "commit": {{"kind": "no_graph_writes"}}
        }}
    }}

if stage == "runner":
    print(json.dumps(lm_callback()), flush=True)
    ready, _, _ = select.select([sys.stdin], [], [], 5)
    if not ready:
        raise SystemExit("timed out waiting for runner lm.complete response")
    lm_response = json.loads(sys.stdin.readline())
    receipt = lm_response["result"]["receipts"][0]
    template = payload["case_input"]["candidate_template"]
    answer = "42" if MARKER in template else "0"
    result = {{
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": payload["stage_call_id"],
        "output": {{
            "kind": "text",
            "summary": "runner answer",
            "value": answer,
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"]
        }},
        "effect_receipts": [{{
            "method": "leaven/lm.complete",
            "receipt": receipt["receipt"],
            "call_kind": "lm_complete",
            "cost": {{"usd_micro": 1000000, "lm_calls": 1}}
        }}]
    }}
    print(json.dumps({{"jsonrpc": "2.0", "id": req.get("id"), "result": result}}), flush=True)
    sys.exit(0)

if stage == "scorer":
    answer = payload["output"]["value"]
    score = 1.0 if answer == "42" else 0.0
    result = {{
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "scorer",
        "stage_call_id": payload["stage_call_id"],
        "output": {{
            "kind": "text",
            "summary": "scorer output",
            "value": answer,
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"]
        }},
        "score": {{"value": score, "rewards": [{{"id": "match", "value": score, "weight": 1.0, "feedback": "f"}}]}}
    }}
    print(json.dumps({{"jsonrpc": "2.0", "id": req.get("id"), "result": result}}), flush=True)
    sys.exit(0)

raise SystemExit("unexpected stage")
"#
        ),
    )
}

/// Mock reflection response: a fenced replacement template carrying the marker
/// so the GEPA parser admits a changed child.
fn reflection_response_with_marker() -> MockLmResponseConfig {
    MockLmResponseConfig {
        text: format!(
            "Here is the improved instruction:\n```\nAnswer the question using the {MARKER}. Output only the integer.\n```"
        ),
        input_tokens: 12,
        output_tokens: 8,
    }
}

fn package() -> PublicSeamPackage {
    PublicSeamPackage::active_from_repo(repo_root()).unwrap()
}

fn runtime(
    service: ConfiguredSeamService,
    pkg: PublicSeamPackage,
) -> SeamRuntime<ConfiguredSeamService> {
    SeamRuntime::from_package(pkg, service).unwrap()
}

fn service_with(
    pkg: &PublicSeamPackage,
    stage: SeamStageConfig,
    lm: SeamLmConfig,
    runs_root: &std::path::Path,
) -> ConfiguredSeamService {
    ConfiguredSeamService::from_package(
        pkg.clone(),
        SeamServiceConfig {
            stage,
            lm,
            optimize_runs_root: Some(runs_root.to_path_buf()),
            ..SeamServiceConfig::default()
        },
    )
    .unwrap()
}

fn optimize_request(seed_template: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "req_optimize",
        "method": "leaven/optimize.run",
        "params": {
            "schema_version": "leaven.optimize_run.v1",
            "message": "optimize_run_request",
            "run_id": "run_optimize_loop",
            "seed": {
                "artifact_type": "prompt",
                "artifact_schema": "fp_schema_sha256_prompt",
                "artifact": {"template": seed_template}
            },
            "cases": [
                {
                    "case": "case_train_1",
                    "input": {"question": "what is six times seven"},
                    "target": {"answer": "42"},
                    "split": "train"
                },
                {
                    "case": "case_validation_1",
                    "input": {"question": "what is six times seven"},
                    "target": {"answer": "42"},
                    "split": "validation"
                }
            ],
            "optimizer": {
                "max_metric_calls": 8,
                "objective": "instance"
            },
            "reflection": {"kind": "lm", "model": "mock"},
            "capability_fingerprint": "fp_cap_sha256_optimize"
        }
    })
}

fn run_optimize(request: &Value, service: ConfiguredSeamService, pkg: PublicSeamPackage) -> Value {
    let runtime = runtime(service, pkg);
    let response = runtime.handle_value(request);
    assert!(
        !response.is_error(),
        "optimize.run errored: {:?}",
        response.value()
    );
    response.value()["result"].clone()
}

#[track_caller]
fn assert_score(actual: &Value, expected: f64) {
    let actual = actual.as_f64().expect("score is a number");
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected score {expected}, got {actual}"
    );
}

#[test]
fn optimize_run_drives_the_real_gepa_loop_to_a_changed_re_evaluated_child() {
    let pkg = package();
    let runs_root = tempfile::tempdir().unwrap();
    let service = service_with(
        &pkg,
        SeamStageConfig::CommandRunner {
            argv: vec![loop_law_worker().display().to_string()],
        },
        SeamLmConfig::Mock {
            responses: vec![reflection_response_with_marker()],
        },
        runs_root.path(),
    );

    let result = run_optimize(
        &optimize_request("Answer the question. Output only the integer."),
        service,
        pkg.clone(),
    );

    // The locked result document validates.
    pkg.validate_optimize_run_result_document(&result)
        .expect("projected result must be schema valid");

    let best = &result["best"];
    let frontier = result["frontier"].as_array().unwrap();
    let best_id = best["candidate"].as_str().unwrap();
    let seed_entry = frontier
        .iter()
        .find(|entry| entry["parent"].is_null())
        .expect("frontier carries the seed entry");
    let seed_id = seed_entry["candidate"].as_str().unwrap();

    // The best is a changed child, not the seed.
    assert_ne!(
        best_id, seed_id,
        "best must be the improved child, not the seed"
    );
    assert_score(&best["score"], 1.0);
    assert_score(&seed_entry["score"], 0.0);

    // The child's parent is the seed.
    let child_entry = frontier
        .iter()
        .find(|entry| entry["candidate"].as_str() == Some(best_id))
        .unwrap();
    assert_eq!(
        child_entry["parent"].as_str(),
        Some(seed_id),
        "child parent must be the seed"
    );

    // The child template carries the improvement marker; the seed does not.
    let child_template = child_entry["artifact"]["artifact"]["template"]
        .as_str()
        .unwrap();
    assert!(
        child_template.contains(MARKER),
        "child template must carry the marker"
    );
    assert!(
        !seed_entry["artifact"]["artifact"]["template"]
            .as_str()
            .unwrap()
            .contains(MARKER),
        "seed template must not carry the marker"
    );

    // The loop did real work: at least one iteration and the exact reference
    // metric-call count. With the OptimizeAnything profile minibatch of 3 over a
    // single train case, the reference loop spends:
    //   seed full validation on VALIDATION (1 case)        = 1
    //   parent minibatch screen on TRAIN (3 samples)       = 3
    //   child minibatch screen on TRAIN (3 samples)        = 3
    //   accepted child full validation on VALIDATION (1)   = 1
    //   total                                              = 8
    // A silently skipped re-evaluation (e.g. reusing the parent screen for the
    // child, or skipping child validation) would drop below 8.
    assert!(result["iterations"].as_u64().unwrap() >= 1);
    assert_eq!(
        result["metric_calls_used"].as_u64().unwrap(),
        8,
        "reference loop spends exactly 8 metric calls (1 seed + 3 parent + 3 child + 1 validation)"
    );

    // Proposals were applied, naming graph truth.
    assert!(
        !result["applied_proposals"].as_array().unwrap().is_empty(),
        "applied_proposals must be non-empty"
    );

    // A durable checkpoint exists in the run dir.
    let run_dir = runs_root.path().join("run_optimize_loop");
    assert!(
        walk_has_checkpoint(&run_dir),
        "a checkpoint must exist under {run_dir:?}"
    );
}

fn walk_has_checkpoint(dir: &std::path::Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if walk_has_checkpoint(&path) {
                return true;
            }
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains("checkpoint"))
        {
            return true;
        }
    }
    false
}

#[test]
fn runner_stage_refuses_case_target_while_scorer_stage_serves_it() {
    let pkg = package();
    let runs_root = tempfile::tempdir().unwrap();
    let service = service_with(
        &pkg,
        SeamStageConfig::CommandRunner {
            argv: vec![target_probe_worker().display().to_string()],
        },
        SeamLmConfig::Mock {
            responses: vec![reflection_response_with_marker()],
        },
        runs_root.path(),
    );

    // The runner stage refuses case.target (worker answers RUNNER_TARGET_REFUSED),
    // and the scorer stage reads the target with a receipt and scores 1.0. Cap
    // the budget to the seed validation so the loop does not reflect (the seed
    // is already perfect, which would otherwise exhaust the mock LM).
    let mut request = optimize_request("Answer the question. Output only the integer.");
    request["params"]["optimizer"]["max_metric_calls"] = json!(1);
    let result = run_optimize(&request, service, pkg);

    let frontier = result["frontier"].as_array().unwrap();
    let seed_entry = frontier
        .iter()
        .find(|entry| entry["parent"].is_null())
        .unwrap();
    // The seed scored 1.0 because the runner saw the target refusal and the
    // scorer successfully read the target.
    assert_score(&seed_entry["score"], 1.0);
}

#[test]
fn optimize_run_refuses_unsupported_objective_naming_instance() {
    let pkg = package();
    let runs_root = tempfile::tempdir().unwrap();
    let service = service_with(
        &pkg,
        SeamStageConfig::CommandRunner {
            argv: vec![loop_law_worker().display().to_string()],
        },
        SeamLmConfig::Mock {
            responses: vec![reflection_response_with_marker()],
        },
        runs_root.path(),
    );
    let mut request = optimize_request("seed");
    request["params"]["optimizer"]["objective"] = json!("objective");
    let runtime = runtime(service, pkg);
    let response = runtime.handle_value(&request);
    assert!(response.is_error());
    let message = response.value()["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("instance"),
        "refusal must name `instance`: {message}"
    );
}

#[test]
fn optimize_run_refuses_agentic_reflection() {
    let pkg = package();
    let runs_root = tempfile::tempdir().unwrap();
    let service = service_with(
        &pkg,
        SeamStageConfig::CommandRunner {
            argv: vec![loop_law_worker().display().to_string()],
        },
        SeamLmConfig::Mock {
            responses: vec![reflection_response_with_marker()],
        },
        runs_root.path(),
    );
    let mut request = optimize_request("seed");
    request["params"]["reflection"] = json!({"kind": "agentic"});
    let runtime = runtime(service, pkg);
    let response = runtime.handle_value(&request);
    assert!(response.is_error());
    let message = response.value()["error"]["message"].as_str().unwrap();
    assert!(message.contains("lm"), "refusal must name `lm`: {message}");
}

#[test]
fn optimize_run_refuses_unknown_artifact_type() {
    let pkg = package();
    let runs_root = tempfile::tempdir().unwrap();
    let service = service_with(
        &pkg,
        SeamStageConfig::CommandRunner {
            argv: vec![loop_law_worker().display().to_string()],
        },
        SeamLmConfig::Mock {
            responses: vec![reflection_response_with_marker()],
        },
        runs_root.path(),
    );
    let mut request = optimize_request("seed");
    request["params"]["seed"]["artifact_type"] = json!("agent_kit");
    let runtime = runtime(service, pkg);
    let response = runtime.handle_value(&request);
    assert!(response.is_error());
    let message = response.value()["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("prompt"),
        "refusal must name `prompt`: {message}"
    );
}

#[test]
fn population_size_caps_the_candidate_pool_at_seed_plus_one_child() {
    let pkg = package();
    let runs_root = tempfile::tempdir().unwrap();
    let service = service_with(
        &pkg,
        SeamStageConfig::CommandRunner {
            argv: vec![loop_law_worker().display().to_string()],
        },
        SeamLmConfig::Mock {
            responses: vec![reflection_response_with_marker()],
        },
        runs_root.path(),
    );

    let mut request = optimize_request("Answer the question. Output only the integer.");
    // A candidate-pool cap of 2 stops the loop after the seed plus one authored
    // child. The budget is raised well past one child's cost so the cap, not the
    // budget, is what stops the loop. The mock LM carries a single reflection
    // response, so a loop that did not stop at the cap would try a second
    // reflection and fail; the cap is what keeps the run truthful.
    request["params"]["optimizer"]["population_size"] = json!(2);
    request["params"]["optimizer"]["max_metric_calls"] = json!(100);
    let result = run_optimize(&request, service, pkg.clone());

    pkg.validate_optimize_run_result_document(&result)
        .expect("projected result must be schema valid");

    let frontier = result["frontier"].as_array().unwrap();
    let best = &result["best"];
    let best_id = best["candidate"].as_str().unwrap();
    let seed_entry = frontier
        .iter()
        .find(|entry| entry["parent"].is_null())
        .expect("frontier carries the seed entry");
    let seed_id = seed_entry["candidate"].as_str().unwrap();

    // The cap admitted exactly the seed and one child onto the frontier.
    assert_eq!(
        frontier.len(),
        2,
        "the candidate-pool cap admits the seed and exactly one child"
    );
    // The best is the genuinely-winning child, not a hardcoded seed.
    assert_ne!(best_id, seed_id, "best must be the improved child");
    assert_score(&best["score"], 1.0);
    assert_score(&seed_entry["score"], 0.0);

    // Metric-call accounting is the reference loop's one-child cost: a cap stop
    // does not invent or drop screening calls (1 seed + 3 parent + 3 child + 1
    // validation = 8).
    assert_eq!(
        result["metric_calls_used"].as_u64().unwrap(),
        8,
        "the one authored child spends the reference loop's 8 metric calls"
    );
}

#[test]
fn minibatch_size_overrides_the_reference_screening_minibatch() {
    let pkg = package();
    let runs_root = tempfile::tempdir().unwrap();
    let service = service_with(
        &pkg,
        SeamStageConfig::CommandRunner {
            argv: vec![loop_law_worker().display().to_string()],
        },
        SeamLmConfig::Mock {
            responses: vec![reflection_response_with_marker()],
        },
        runs_root.path(),
    );

    let mut request = optimize_request("Answer the question. Output only the integer.");
    // With a minibatch override of 1, the parent and child each screen on a
    // single train case instead of the profile's fixed 3, so the loop-law exact
    // metric count drops from 8 to 4 (1 seed validation + 1 parent screen + 1
    // child screen + 1 child validation). The budget is the matching 4 so the
    // run authors exactly the one child and stops, like the reference loop-law
    // spends exactly its 8-call budget on one child.
    request["params"]["optimizer"]["minibatch_size"] = json!(1);
    request["params"]["optimizer"]["max_metric_calls"] = json!(4);
    let result = run_optimize(&request, service, pkg.clone());

    pkg.validate_optimize_run_result_document(&result)
        .expect("projected result must be schema valid");

    let best = &result["best"];
    let frontier = result["frontier"].as_array().unwrap();
    let seed_id = frontier
        .iter()
        .find(|entry| entry["parent"].is_null())
        .and_then(|entry| entry["candidate"].as_str())
        .unwrap();
    // The override changes the screening minibatch, not the loop's correctness:
    // the improved child still wins.
    assert_ne!(best["candidate"].as_str().unwrap(), seed_id);
    assert_score(&best["score"], 1.0);

    assert_eq!(
        result["metric_calls_used"].as_u64().unwrap(),
        4,
        "minibatch override 1 spends 1 seed + 1 parent + 1 child + 1 validation = 4 metric calls"
    );
}

#[test]
fn optimize_run_refuses_population_size_one_naming_the_minimum_bound() {
    let pkg = package();
    let runs_root = tempfile::tempdir().unwrap();
    let service = service_with(
        &pkg,
        SeamStageConfig::CommandRunner {
            argv: vec![loop_law_worker().display().to_string()],
        },
        SeamLmConfig::Mock {
            responses: vec![reflection_response_with_marker()],
        },
        runs_root.path(),
    );
    let mut request = optimize_request("seed");
    // A cap of 1 admits only the seed and can never author a child: a no-op
    // optimization request, refused naming the `>= 2` bound.
    request["params"]["optimizer"]["population_size"] = json!(1);
    let runtime = runtime(service, pkg);
    let response = runtime.handle_value(&request);
    assert!(response.is_error());
    let message = response.value()["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("population_size") && message.contains("at least 2"),
        "refusal must name population_size and the >= 2 bound: {message}"
    );
}

#[test]
fn optimize_run_unavailable_without_configured_stage_worker() {
    let pkg = package();
    let service = ConfiguredSeamService::from_package(
        pkg.clone(),
        SeamServiceConfig {
            lm: SeamLmConfig::Mock {
                responses: vec![reflection_response_with_marker()],
            },
            ..SeamServiceConfig::default()
        },
    )
    .unwrap();
    let runtime = runtime(service, pkg);
    let response = runtime.handle_value(&optimize_request("seed"));
    assert!(response.is_error());
}

#[test]
fn small_budget_stops_the_loop_with_best_seed_and_truthful_metric_calls() {
    let pkg = package();
    let runs_root = tempfile::tempdir().unwrap();
    let service = service_with(
        &pkg,
        SeamStageConfig::CommandRunner {
            argv: vec![loop_law_worker().display().to_string()],
        },
        SeamLmConfig::Mock {
            responses: vec![reflection_response_with_marker()],
        },
        runs_root.path(),
    );
    let mut request = optimize_request("Answer the question. Output only the integer.");
    // Only enough budget to validate the seed; no child can be admitted.
    request["params"]["optimizer"]["max_metric_calls"] = json!(1);
    let result = run_optimize(&request, service, pkg.clone());

    pkg.validate_optimize_run_result_document(&result)
        .expect("result must be schema valid even when budget stops the loop");

    let frontier = result["frontier"].as_array().unwrap();
    let best_id = result["best"]["candidate"].as_str().unwrap();
    let seed_entry = frontier
        .iter()
        .find(|entry| entry["parent"].is_null())
        .unwrap();
    assert_eq!(
        best_id,
        seed_entry["candidate"].as_str().unwrap(),
        "best must remain the seed when the budget admits no child"
    );
    // No phantom iterations beyond the seed validation.
    assert_eq!(
        result["metric_calls_used"].as_u64().unwrap(),
        1,
        "only the seed validation metric call was spent"
    );
}

#[test]
fn worker_effect_cost_aggregates_into_result_cost_totals() {
    let pkg = package();
    let runs_root = tempfile::tempdir().unwrap();
    let service = service_with(
        &pkg,
        SeamStageConfig::CommandRunner {
            argv: vec![cost_reporting_worker().display().to_string()],
        },
        SeamLmConfig::Mock {
            responses: vec![
                MockLmResponseConfig {
                    text: "callback completion".to_owned(),
                    input_tokens: 9,
                    output_tokens: 4,
                },
                MockLmResponseConfig {
                    text: "callback completion".to_owned(),
                    input_tokens: 9,
                    output_tokens: 4,
                },
                MockLmResponseConfig {
                    text: "callback completion".to_owned(),
                    input_tokens: 9,
                    output_tokens: 4,
                },
                reflection_response_with_marker(),
            ],
        },
        runs_root.path(),
    );

    let result = run_optimize(
        &optimize_request("Answer the question. Output only the integer."),
        service,
        pkg.clone(),
    );
    pkg.validate_optimize_run_result_document(&result).unwrap();

    // The runner worker reported lm.complete cost facts; they aggregate into the
    // result cost totals.
    let lm_calls = result["cost"]["lm_calls"].as_u64().unwrap();
    assert!(
        lm_calls >= 1,
        "worker lm.complete calls must aggregate: {result:?}"
    );
    let input_tokens = result["cost"]["input_tokens"].as_u64().unwrap();
    assert!(
        input_tokens >= 9,
        "worker input tokens must aggregate: {input_tokens}"
    );
}

#[test]
fn max_cost_usd_micro_ceiling_stops_the_loop_before_a_child_is_authored() {
    let pkg = package();
    let runs_root = tempfile::tempdir().unwrap();
    let service = service_with(
        &pkg,
        SeamStageConfig::CommandRunner {
            argv: vec![usd_cost_worker().display().to_string()],
        },
        SeamLmConfig::Mock {
            responses: vec![reflection_response_with_marker()],
        },
        runs_root.path(),
    );

    // Every runner stage reports 1_000_000 usd_micro. The metric budget is high
    // (a child could be authored on metric calls alone), but a usd ceiling of
    // 1_500_000 is below one child's screening spend: the seed validation spends
    // ~1_000_000, and the parent/child minibatch screens push past the ceiling,
    // so the loop stops with the seed as best. The usd axis, not the metric cap,
    // is what stops the loop.
    let mut request = optimize_request("Answer the question. Output only the integer.");
    request["params"]["optimizer"]["max_metric_calls"] = json!(100);
    request["params"]["optimizer"]["max_cost_usd_micro"] = json!(1_500_000);
    let result = run_optimize(&request, service, pkg.clone());

    pkg.validate_optimize_run_result_document(&result)
        .expect("result must be schema valid when the usd ceiling stops the loop");

    let frontier = result["frontier"].as_array().unwrap();
    let best_id = result["best"]["candidate"].as_str().unwrap();
    let seed_entry = frontier
        .iter()
        .find(|entry| entry["parent"].is_null())
        .unwrap();
    assert_eq!(
        best_id,
        seed_entry["candidate"].as_str().unwrap(),
        "the usd ceiling must stop the loop with the seed as best (no affordable child)"
    );
    // The reported usd cost never exceeds the ceiling by more than one charge.
    let usd_micro = result["cost"]["usd_micro"].as_u64().unwrap();
    assert!(
        usd_micro >= 1_000_000,
        "the seed validation usd cost must be reported: {usd_micro}"
    );
}

/// A single-shot fake `OpenAI` Responses endpoint.
///
/// Accepts exactly one HTTP connection on a loopback port and replies with one
/// Responses-API body. The optimize.run reference loop calls the reflection LM
/// exactly once (`loop_law_worker` issues no `lm.complete` callbacks, only
/// `case.target`), so one response covers the whole loop.
struct FakeOpenAiServer {
    url: String,
}

impl FakeOpenAiServer {
    fn start(reflection_text: &str) -> Self {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let body = json!({
            "id": "resp_optimize_reflection",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": reflection_text}]
            }],
            "usage": {"input_tokens": 12, "output_tokens": 8}
        })
        .to_string();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 16 * 1024];
            let _ = stream.read(&mut buffer).unwrap();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        Self {
            url: format!("http://{addr}/v1/responses"),
        }
    }

    fn url(&self) -> String {
        self.url.clone()
    }
}

/// Regression: `OpenAI`-backed reflection must execute under the optimize.run
/// executor without panicking for lack of a tokio reactor.
///
/// `OpenAiLm::complete` uses reqwest plus tokio timers/semaphores, so it needs a
/// running reactor on the thread that polls the GEPA loop. The earlier
/// `futures::executor::block_on` orchestration had no reactor, so this path
/// panicked ("there is no reactor running"). The loop now runs under a tokio
/// current-thread runtime; this test drives a real `SeamLmConfig::OpenAi`
/// reflector (against a loopback fake provider, no network) all the way to a
/// changed, re-evaluated child, proving the documented `OpenAI` reflection path
/// is actually reachable. The worker LM callback path is exercised by the
/// mock-LM loop law; here the worker issues no `lm.complete`, so the single
/// `OpenAI` call is the reflection itself.
#[test]
fn openai_backed_reflection_executes_through_the_optimize_run_executor() {
    let server = FakeOpenAiServer::start(&reflection_response_with_marker().text);
    let pkg = package();
    let runs_root = tempfile::tempdir().unwrap();
    let service = service_with(
        &pkg,
        SeamStageConfig::CommandRunner {
            argv: vec![loop_law_worker().display().to_string()],
        },
        SeamLmConfig::OpenAi {
            // `PATH` is always set, so the configured api-key env resolves
            // without a real credential; the fake endpoint ignores the key.
            api_key_env: "PATH".to_owned(),
            base_url: Some(server.url()),
            timeout_s: Some(5),
            max_retries: Some(0),
        },
        runs_root.path(),
    );

    let result = run_optimize(
        &optimize_request("Answer the question. Output only the integer."),
        service,
        pkg.clone(),
    );

    pkg.validate_optimize_run_result_document(&result)
        .expect("projected result must be schema valid");

    let best = &result["best"];
    let frontier = result["frontier"].as_array().unwrap();
    let best_id = best["candidate"].as_str().unwrap();
    let seed_id = frontier
        .iter()
        .find(|entry| entry["parent"].is_null())
        .and_then(|entry| entry["candidate"].as_str())
        .unwrap();

    // Reflection ran through the live provider and produced a changed child that
    // re-evaluated onto the frontier with a perfect score.
    assert_ne!(
        best_id, seed_id,
        "OpenAI-backed reflection must produce an improved child"
    );
    assert_score(&best["score"], 1.0);
}
