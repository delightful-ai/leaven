//! `leaven serve --stdio` drives a real optimization over the bidirectional seam.
//!
//! This is the inverse of the bridge's `example_03_prompt_optimize`: there the
//! Rust test is the ACP client and the Python worker is the agent. Here the test
//! is the **stand-in parent agent** and `leaven serve --stdio` is the client that
//! DRIVES the optimization. The test spawns the `leaven` binary (the same binary
//! the Python SDK spawns), injects the locked capability env, and serves the
//! runner stage `leaven serve` dispatches:
//!
//!   1. `leaven serve` dispatches `leaven/stage.run` (host->worker) — the test
//!      reads it from the child's stdout.
//!   2. The test runs the runner rollout by initiating `leaven/lm.complete`
//!      (worker->host) back to the child's stdin; `leaven serve`'s deterministic
//!      mock host LM answers on the child's stdout.
//!   3. The test returns the worker's text `stage_run_result` on the child's
//!      stdin.
//!
//! `leaven serve` runs the tiny but real GEPA-shaped accept loop over those live
//! rollouts and writes a real `Optimized` to its `--out` file. The seam, dispatch,
//! and accept loop are real; the LM is a deterministic mock (no spend, no network).

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn serve_stdio_optimizes_a_prompt_driving_a_stand_in_parent_agent() {
    let temp = TempDir::new().unwrap();
    let plan_path = temp.path().join("plan.json");
    let out_path = temp.path().join("result.json");
    std::fs::write(
        &plan_path,
        serde_json::to_vec_pretty(&optimize_plan()).unwrap(),
    )
    .unwrap();

    // The test is the parent: it spawns `leaven serve --stdio` and injects the
    // locked capability env the seam requires.
    let mut child = Command::new(env!("CARGO_BIN_EXE_leaven"))
        .arg("serve")
        .arg("--stdio")
        .arg("--root")
        .arg(workspace_root())
        .arg("--plan")
        .arg(&plan_path)
        .arg("--out")
        .arg(&out_path)
        .env("LEAVEN_CAPABILITY_TOKEN", "secret-token")
        .env("LEAVEN_ENDPOINT", "stdio://serve/session")
        .env("LEAVEN_CAPABILITY_FINGERPRINT", "fp_cap_sha256_serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn leaven serve --stdio");

    let mut agent = ParentAgent::new(&mut child);
    let stage_runs = agent.serve_until_eof();

    let status = child.wait().expect("wait for leaven serve");
    assert!(status.success(), "leaven serve exited with {status:?}");
    assert!(
        stage_runs > 0,
        "leaven serve must dispatch at least one runner stage over the seam"
    );

    // The real Optimized result `leaven serve` wrote: a strict improvement was
    // accepted because the reflector surfaced the question to the host's mock LM.
    let result: Value = serde_json::from_slice(&std::fs::read(&out_path).unwrap())
        .expect("leaven serve wrote a JSON Optimized result");

    let frontier = result["frontier"].as_array().expect("frontier is an array");
    let seed = frontier
        .iter()
        .find(|candidate| candidate["parent_id"].is_null())
        .expect("the seed is on the frontier");
    assert!(
        seed["score"].as_f64().unwrap().abs() < f64::EPSILON,
        "the seed never surfaces the question, so it scores zero: {seed}"
    );
    assert!(
        !result["best"]["parent_id"].is_null(),
        "best must be a reflected child, not the seed: {}",
        result["best"]
    );
    assert!(
        (result["best"]["score"].as_f64().unwrap() - 1.0).abs() < f64::EPSILON,
        "the accepted child solves every case exactly: {}",
        result["best"]
    );
    assert!(
        result["iterations"].as_u64().unwrap() >= 1,
        "at least one reflect/accept ran: {result}"
    );
    assert!(
        result["best"]["template"]
            .as_str()
            .unwrap()
            .contains("{question}"),
        "the optimized prompt surfaces the question: {}",
        result["best"]["template"]
    );
}

/// The stand-in parent agent over `leaven serve`'s inherited stdio.
///
/// `leaven serve` is the ACP client; this agent serves the runner stage it
/// dispatches and initiates `leaven/lm.complete` back into `leaven serve`'s host
/// LM, exactly like `serve_stage_runner.py` does in the bridge example.
struct ParentAgent {
    reader: BufReader<ChildStdout>,
    writer: ChildStdin,
}

impl ParentAgent {
    fn new(child: &mut Child) -> Self {
        Self {
            reader: BufReader::new(child.stdout.take().expect("child exposes stdout")),
            writer: child.stdin.take().expect("child exposes stdin"),
        }
    }

    /// Serves runner-stage dispatches until `leaven serve` closes the seam.
    ///
    /// Returns the number of runner stages served.
    fn serve_until_eof(&mut self) -> u64 {
        let mut stage_runs = 0;
        while let Some(message) = self.read_message() {
            let method = message["method"].as_str();
            assert_eq!(
                method,
                Some("leaven/stage.run"),
                "the agent only expects runner-stage dispatches: {message}"
            );
            self.serve_runner_stage(&message);
            stage_runs += 1;
        }
        stage_runs
    }

    /// Runs one target-free runner rollout: call the LM back, return the output.
    fn serve_runner_stage(&mut self, request: &Value) {
        let payload = &request["params"]["payload"];
        assert_eq!(payload["role"], json!("runner"), "{payload}");
        assert_eq!(payload["target_forbidden"], json!(true), "{payload}");
        // The engine projected the rendered, model-facing prompt into case_input.
        let prompt = payload["case_input"]["prompt"]
            .as_str()
            .expect("runner case input carries the rendered prompt");
        let stage_call_id = payload["stage_call_id"].as_str().expect("stage call id");

        let completion = self.lm_complete(prompt, &format!("{stage_call_id}::lm"));

        self.write_message(&json!({
            "jsonrpc": "2.0",
            "id": request["id"],
            "result": {
                "schema_version": "leaven.stage_run.v1",
                "message": "stage_run_result",
                "stage": "runner",
                "stage_call_id": stage_call_id,
                "output": {
                    "kind": "text",
                    "summary": format!("runner output for {}", payload["case"]),
                    "value": completion.trim(),
                    "visibility": "optimizer_visible",
                    "data_classes": ["candidate.output"],
                },
            },
        }));
    }

    /// Worker-initiated `leaven/lm.complete`: bind the prompt, read the host reply.
    fn lm_complete(&mut self, prompt: &str, request_id: &str) -> String {
        self.write_message(&json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "leaven/lm.complete",
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": "plan_runner_lm_complete",
                "consistency": {"kind": "latest_at_start"},
                "mode": {"kind": "dry_run"},
                "ops": [{
                    "kind": "let",
                    "name": "prompt",
                    "expr": {"kind": "literal", "value": prompt, "data_classes": ["public"]},
                }],
                "return": ["prompt"],
                "commit": {"kind": "no_graph_writes"},
            },
        }));
        let reply = self
            .read_message()
            .expect("host answered leaven/lm.complete before closing the seam");
        let result = &reply["result"];
        assert_eq!(result["method"], json!("leaven/lm.complete"), "{reply}");
        // The transport stamps the launched session fingerprint onto the reply.
        assert_eq!(
            result["capability_fingerprint"],
            json!("fp_cap_sha256_serve"),
            "{reply}"
        );
        result["primary"]["message"]["content"]
            .as_array()
            .expect("lm_response content is an array")
            .iter()
            .filter(|part| part["kind"] == json!("text"))
            .filter_map(|part| part["text"].as_str())
            .collect()
    }

    fn read_message(&mut self) -> Option<Value> {
        let mut line = String::new();
        let count = self.reader.read_line(&mut line).expect("read seam line");
        if count == 0 {
            return None;
        }
        Some(serde_json::from_str(&line).expect("seam line is JSON"))
    }

    fn write_message(&mut self, value: &Value) {
        serde_json::to_writer(&mut self.writer, value).expect("write seam line");
        self.writer.write_all(b"\n").expect("write newline");
        self.writer.flush().expect("flush seam line");
    }
}

/// The optimize plan `leaven serve` drives: a seed that hides the question, the
/// arithmetic cases, exact-match reward, and the surface-question reflector.
fn optimize_plan() -> Value {
    json!({
        "run_id": "serve_stdio",
        // The seed never surfaces the question to the model, so the mock LM has
        // nothing to evaluate and the seed scores zero — real headroom to improve.
        "seed_template": "You are a calculator. Always answer 0.",
        "cases": arithmetic_cases(),
        "minibatch": 4,
        "max_iterations": 2,
        "reward": "exact_match",
        "reflect": "surface_question"
    })
}

fn arithmetic_cases() -> Vec<Value> {
    let fixture = workspace_root().join("docs/specs/leaven_py/examples/fixtures/arithmetic.jsonl");
    let text = std::fs::read_to_string(&fixture)
        .unwrap_or_else(|error| panic!("read fixture {}: {error}", fixture.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let row: Value = serde_json::from_str(line).expect("fixture row is JSON");
            let raw_id = row["id"].as_str().expect("fixture row has id");
            json!({
                // The wire CaseId pattern is `^case_[A-Za-z0-9_.:-]+$`.
                "case_id": format!("case_{}", raw_id.replace('-', "_")),
                "input": row["input"].clone(),
                "target": row["target"].clone(),
            })
        })
        .collect()
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under workspace/crates/leaven-cli")
        .to_path_buf()
}
