//! `leaven seam serve --stdio` is the durable public-seam server route.
//!
//! This test is process-level proof for the CLI boundary: the binary loads a
//! typed service config, reserves stdout for line-delimited JSON-RPC responses,
//! validates request envelopes, rejects removed methods before service execution,
//! executes a configured public-seam Plan method, and reports an unwired provider
//! honestly.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn seam_serve_stdio_executes_configured_methods_and_reports_unwired_providers() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("seam-service.json");
    std::fs::write(
        &config_path,
        serde_json::to_vec_pretty(&json!({
            "lm": {
                "kind": "mock",
                "responses": [{
                    "text": "cli seam configured lm ok",
                    "input_tokens": 13,
                    "output_tokens": 5
                }]
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_leaven"))
        .arg("seam")
        .arg("serve")
        .arg("--stdio")
        .arg("--root")
        .arg(workspace_root())
        .arg("--config")
        .arg(&config_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn leaven seam serve --stdio");

    {
        let mut stdin = child.stdin.take().expect("child stdin is piped");
        write_json_line(&mut stdin, &invalid_envelope());
        write_json_line(&mut stdin, &removed_human_review_request());
        write_json_line(&mut stdin, &workspace_release_request());
        write_json_line(&mut stdin, &lm_complete_request());
        write_json_line(&mut stdin, &stage_run_request());
    }

    let output = child
        .wait_with_output()
        .expect("wait for leaven seam serve --stdio");
    assert!(
        output.status.success(),
        "server exits successfully after stdin EOF; status={:?}, stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "protocol command must not need diagnostics on stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let responses = response_lines(&output.stdout);
    assert_eq!(responses.len(), 5, "one response per request line");

    assert_eq!(responses[0]["id"], json!("bad-envelope"));
    assert_eq!(responses[0]["error"]["code"], json!(-32600));
    assert!(
        responses[0]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("must carry method")
    );

    assert_eq!(responses[1]["id"], json!("removed-human-review"));
    assert_eq!(responses[1]["error"]["code"], json!(-32601));
    assert!(
        responses[1]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("is not in the locked Leaven worker profile")
    );

    assert_eq!(responses[2]["id"], json!("workspace-release-cli"));
    assert_eq!(
        responses[2]["result"]["primary"]["kind"],
        "workspace_handle"
    );
    assert_eq!(responses[2]["result"]["primary"]["released"], true);
    assert_eq!(
        responses[2]["result"]["receipts"][1]["call_kind"],
        "workspace_release"
    );

    assert_eq!(responses[3]["id"], json!("lm-cli"));
    assert_eq!(
        responses[3]["result"]["primary"]["message"]["content"][0]["text"],
        "cli seam configured lm ok"
    );
    assert_eq!(
        responses[3]["result"]["primary"]["cost"],
        json!({
            "input_tokens": 13,
            "output_tokens": 5,
            "lm_calls": 1
        })
    );
    assert_eq!(
        responses[3]["result"]["receipts"][0]["call_kind"],
        "lm_complete"
    );

    assert_eq!(responses[4]["id"], json!("stage-unwired"));
    assert_eq!(responses[4]["error"]["code"], json!(-32006));
    assert!(
        responses[4]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("does not provide a stage runner")
    );
}

fn write_json_line(stdin: &mut impl Write, value: &Value) {
    serde_json::to_writer(&mut *stdin, value).unwrap();
    stdin.write_all(b"\n").unwrap();
}

fn response_lines(stdout: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn workspace_root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crate lives under workspace/crates")
}

fn invalid_envelope() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "bad-envelope",
        "params": {}
    })
}

fn removed_human_review_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "removed-human-review",
        "method": "leaven/human.review",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "removedhumanreview001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [],
            "return": [],
            "commit": {
                "kind": "no_graph_writes"
            }
        }
    })
}

fn workspace_release_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "workspace-release-cli",
        "method": "leaven/workspace.release",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "workspacecli001",
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
                    "idempotency_key": "workspace-cli-0001",
                    "call": {
                        "kind": "workspace_materialize",
                        "candidate": "cand_cli",
                        "surface": "program",
                        "mode": "copy_on_write",
                        "lifetime": "manual_release"
                    }
                },
                {
                    "kind": "call",
                    "name": "release",
                    "deps": ["workspace"],
                    "idempotency_key": "workspace-cli-0002",
                    "call": {
                        "kind": "workspace_release",
                        "workspace": "ws_cli_materialized",
                        "force": false
                    }
                }
            ],
            "return": ["release"],
            "commit": {
                "kind": "graph_writes_atomic",
                "on_stale": "reject"
            }
        }
    })
}

fn lm_complete_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "lm-cli",
        "method": "leaven/lm.complete",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "planlmcliconfigured001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [
                {
                    "kind": "call",
                    "name": "completion",
                    "idempotency_key": "lm-cli-configured-0001",
                    "call": {
                        "kind": "lm_complete",
                        "purpose": "test.cli_seam_stdio",
                        "model": "gpt-4.1-mini",
                        "messages": [
                            {
                                "role": "developer",
                                "content": [{"kind": "text", "text": "return the final answer"}]
                            },
                            {
                                "role": "user",
                                "content": [{"kind": "text", "text": "solve"}]
                            }
                        ],
                        "output": {
                            "kind": "final_message",
                            "max_bytes": 256
                        },
                        "input_classes": ["public"]
                    }
                }
            ],
            "return": ["completion"],
            "commit": {
                "kind": "no_graph_writes"
            }
        }
    })
}

fn stage_run_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "stage-unwired",
        "method": "leaven/stage.run",
        "params": {
            "schema_version": "leaven.stage_run.v1",
            "message": "stage_run_request",
            "stage": "runner",
            "payload": {
                "schema_version": "leaven.stage_payloads.v1",
                "role": "runner",
                "run": "run_stage_cli",
                "stage_call_id": "sc_cli_unwired",
                "candidate": "cand_cli_unwired",
                "case": "case_cli_unwired",
                "case_input": {"question": "2 + 2"},
                "target_forbidden": true
            }
        }
    })
}
