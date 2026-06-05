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
                "context": {
                    "evaluation_run": "run_demo",
                    "evaluation_request_id": "evalreq_01",
                    "case_partition": "validation"
                },
                "capability": seam_capability(),
                "graph": {
                    "items": [{
                        "kind": "event_summary",
                        "event_kind": "case.loaded",
                        "revision": "rev_cli"
                    }],
                    "data_classes": ["public"]
                },
                "run_context": {
                    "enabled": true,
                    "seed_value": 1,
                    "proposal_delta": 41,
                    "proposal_batch_alias": "pb_cli_run_context",
                    "final_revision": "rev_cli_run_context_applied",
                    "readback_plan_id": "runcontextgraphreadbackcli001"
                },
                "cases": {
                    "case_1": {
                        "case": "case_1",
                        "input": {"question": "2 + 3"},
                        "target": {"answer": 5},
                        "metadata": {"partition": "validation"},
                        "data_classes": ["case.input", "case.target", "case.metadata"]
                    }
                },
                "workspace": {
                    "seed_files": {
                        "README.md": "seeded workspace readme\n",
                        "src/lib.rs": "pub fn answer() -> u8 { 42 }\n"
                    },
                    "git": {
                        "initialize": true,
                        "post_commit_files": {
                            "src/lib.rs": "pub fn answer() -> u8 { 43 }\n"
                        }
                    }
                },
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
    let workspace_queries = workspace_query_requests();
    let graph_case_queries = graph_case_query_requests();

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
        write_json_line(&mut stdin, &unknown_locked_method_request());
        write_json_line(&mut stdin, &workspace_release_request());
        write_json_line(&mut stdin, &workspace_query_after_release_request());
        for request in &workspace_queries {
            write_json_line(&mut stdin, request);
        }
        for request in &graph_case_queries {
            write_json_line(&mut stdin, request);
        }
        write_json_line(&mut stdin, &proposal_apply_request());
        write_json_line(&mut stdin, &run_context_graph_readback_request());
        write_json_line(&mut stdin, &run_context_event_emit_request());
        write_json_line(
            &mut stdin,
            &run_context_graph_readback_after_event_request(),
        );
        write_json_line(&mut stdin, &run_context_evaluation_request_request());
        write_json_line(&mut stdin, &run_context_assessment_submit_request());
        write_json_line(
            &mut stdin,
            &run_context_graph_readback_after_assessment_request(),
        );
        write_json_line(&mut stdin, &assessment_submit_request());
        write_json_line(&mut stdin, &evaluation_request_request());
        write_json_line(&mut stdin, &event_emit_request());
        write_json_line(&mut stdin, &graph_write_readback_request());
        write_json_line(&mut stdin, &sandbox_exec_request());
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
    assert_eq!(
        responses.len(),
        18 + workspace_queries.len() + graph_case_queries.len(),
        "one response per request line"
    );

    assert_eq!(responses[0]["id"], json!("bad-envelope"));
    assert_eq!(responses[0]["error"]["code"], json!(-32600));
    assert!(
        responses[0]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("must carry method")
    );

    assert_eq!(responses[1]["id"], json!("unknown-locked-method"));
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

    assert_eq!(
        responses[3]["id"],
        json!("workspace-query-after-release-cli")
    );
    assert_eq!(responses[3]["error"]["code"], json!(-32006));
    assert!(
        responses[3]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("workspace_query refused already released workspace"),
        "unexpected released workspace error: {:?}",
        responses[3]
    );

    for (offset, request) in workspace_queries.iter().enumerate() {
        let response = &responses[4 + offset];
        let query_name = request["params"]["return"][0].as_str().unwrap();
        assert_eq!(
            response["id"],
            json!(format!("workspace-query-cli-{query_name}"))
        );
        assert!(
            response.get("error").is_none(),
            "unexpected workspace query error: {response:?}"
        );
        assert_eq!(
            response["result"]["receipts"][0]["call_kind"],
            "workspace_materialize"
        );
        assert_eq!(response["result"]["receipts"][1]["kind"], "query");
        match query_name {
            "file" => {
                assert_eq!(
                    response["result"]["primary"]["content"],
                    "seeded workspace readme\n"
                );
            }
            "listing" => {
                assert_eq!(
                    response["result"]["primary"]["entries"][0]["path"],
                    "README.md"
                );
            }
            "stat" => {
                assert_eq!(
                    response["result"]["primary"]["entries"][0]["bytes"],
                    "seeded workspace readme\n".len()
                );
            }
            "digest" => {
                assert!(
                    response["result"]["primary"]["digest"]
                        .as_str()
                        .unwrap()
                        .starts_with("sha256:")
                );
            }
            "snapshot" => {
                assert!(
                    response["result"]["primary"]["digest"]
                        .as_str()
                        .unwrap()
                        .starts_with("blake3:")
                );
            }
            "captured" => {
                assert_eq!(
                    response["result"]["primary"]["entries"][0]["bytes"],
                    "seeded workspace readme\n".len()
                );
                assert_eq!(
                    response["result"]["primary"]["entries"][0]["content_base64"],
                    "c2VlZGVkIHdvcmtzcGFjZSByZWFkbWUK"
                );
                assert_eq!(
                    response["result"]["primary"]["entries"][0]["blob_ref"]["bytes"],
                    "seeded workspace readme\n".len()
                );
                assert_eq!(
                    response["result"]["primary"]["entries"][0]["blob_ref"]["sha256"],
                    response["result"]["primary"]["entries"][0]["sha256"]
                );
            }
            "gitlog" => {
                assert!(
                    response["result"]["primary"]["text"]
                        .as_str()
                        .unwrap()
                        .contains("leaven workspace seed")
                );
                assert_eq!(
                    response["result"]["primary"]["source_refs"][0]["namespace"],
                    "leaven.workspace.git_log.max_entries"
                );
            }
            "gitdiff" => {
                let text = response["result"]["primary"]["text"].as_str().unwrap();
                assert!(text.contains("-pub fn answer() -> u8 { 42 }"));
                assert!(text.contains("+pub fn answer() -> u8 { 43 }"));
                assert_eq!(
                    response["result"]["primary"]["source_refs"][0]["namespace"],
                    "leaven.workspace.git_diff.against"
                );
            }
            "gitstatus" => {
                assert!(
                    response["result"]["primary"]["text"]
                        .as_str()
                        .unwrap()
                        .contains(" M src/lib.rs")
                );
                assert_eq!(
                    response["result"]["primary"]["source_refs"][0]["namespace"],
                    "leaven.workspace.git_status.porcelain"
                );
            }
            other => panic!("unexpected workspace query `{other}`"),
        }
    }

    let graph_case_start = 4 + workspace_queries.len();
    for (offset, request) in graph_case_queries.iter().enumerate() {
        let response = &responses[graph_case_start + offset];
        assert_eq!(response["id"], request["id"]);
        assert!(
            response.get("error").is_none(),
            "unexpected graph/case query error: {response:?}"
        );
        assert_eq!(response["result"]["receipts"][0]["kind"], "query");
        match request["method"].as_str().unwrap() {
            "leaven/graph.query" => {
                assert_eq!(response["result"]["primary"]["kind"], "graph_set");
                assert_eq!(
                    response["result"]["primary"]["items"][0]["event_kind"],
                    "case.loaded"
                );
            }
            "leaven/case.load" => {
                assert_eq!(response["result"]["primary"]["kind"], "case_record");
                assert_eq!(response["result"]["primary"]["input"]["question"], "2 + 3");
                assert_eq!(response["result"]["primary"]["target"]["answer"], 5);
                assert_eq!(
                    response["result"]["primary"]["metadata"]["partition"],
                    "validation"
                );
            }
            "leaven/case.input" => {
                assert_eq!(response["result"]["primary"]["input"]["question"], "2 + 3");
                assert!(response["result"]["primary"].get("target").is_none());
                assert!(response["result"]["primary"].get("metadata").is_none());
            }
            "leaven/case.target" => {
                assert_eq!(response["result"]["primary"]["target"]["answer"], 5);
                assert!(response["result"]["primary"].get("input").is_none());
            }
            "leaven/case.metadata" => {
                assert_eq!(
                    response["result"]["primary"]["metadata"]["partition"],
                    "validation"
                );
                assert!(response["result"]["primary"].get("input").is_none());
            }
            other => panic!("unexpected graph/case method {other}"),
        }
    }

    let proposal_apply_index = graph_case_start + graph_case_queries.len();
    assert_eq!(
        responses[proposal_apply_index]["id"],
        json!("proposal-apply-cli")
    );
    assert!(
        responses[proposal_apply_index].get("error").is_none(),
        "unexpected proposal apply response: {:?}",
        responses[proposal_apply_index]
    );
    assert_eq!(
        responses[proposal_apply_index]["result"]["primary"]["kind"],
        "apply_receipt"
    );
    assert!(
        responses[proposal_apply_index]["result"]["receipts"]
            .as_array()
            .expect("proposal apply carries receipts")
            .iter()
            .any(|receipt| receipt["write_kind"].as_str() == Some("apply_proposal_batch")),
        "proposal apply response must carry an apply_proposal_batch receipt: {:?}",
        responses[proposal_apply_index]
    );
    assert_eq!(
        responses[proposal_apply_index]["result"]["primary"]["graph_revision"],
        "rev_cli_run_context_applied"
    );
    let run_context_created =
        responses[proposal_apply_index]["result"]["primary"]["created_candidates"]
            .as_array()
            .expect("RunContext apply returns created candidates");
    assert_eq!(run_context_created.len(), 1);
    assert_ne!(
        run_context_created[0],
        json!("cand_pb_cli_run_context_applied"),
        "created candidate must come from RunContext projection, not configured string synthesis"
    );

    let run_context_readback_index = proposal_apply_index + 1;
    assert_eq!(
        responses[run_context_readback_index]["id"],
        json!("run-context-graph-readback-cli")
    );
    assert!(
        responses[run_context_readback_index].get("error").is_none(),
        "unexpected RunContext readback response: {:?}",
        responses[run_context_readback_index]
    );
    let run_context_summary =
        &responses[run_context_readback_index]["result"]["primary"]["items"][0];
    assert_eq!(run_context_summary["kind"], "event_summary");
    assert_eq!(run_context_summary["event_kind"], "proposal.apply");
    assert_eq!(
        run_context_summary["payload"]["source"],
        "leaven-seam-service-run-context"
    );
    assert_eq!(run_context_summary["payload"]["applied"], true);
    assert_eq!(run_context_summary["payload"]["candidate_count"], 2);
    assert_eq!(
        run_context_summary["payload"]["created_candidates"][0],
        run_context_created[0]
    );

    let run_context_event_index = run_context_readback_index + 1;
    assert_eq!(
        responses[run_context_event_index]["id"],
        json!("run-context-event-emit-cli")
    );
    assert!(
        responses[run_context_event_index].get("error").is_none(),
        "unexpected RunContext event response: {:?}",
        responses[run_context_event_index]
    );
    assert_eq!(
        responses[run_context_event_index]["result"]["primary"]["kind"],
        "emit_run_event"
    );
    assert_eq!(
        responses[run_context_event_index]["result"]["receipts"][0]["write_kind"],
        "emit_run_event"
    );

    let run_context_event_readback_index = run_context_event_index + 1;
    assert_eq!(
        responses[run_context_event_readback_index]["id"],
        json!("run-context-graph-readback-after-event-cli")
    );
    let event_readback =
        &responses[run_context_event_readback_index]["result"]["primary"]["items"][0]["payload"];
    assert_eq!(event_readback["event_count"], 5);
    assert_eq!(
        event_readback["emitted_events"][0]["event_kind"],
        "run_context.checked"
    );
    assert_eq!(
        event_readback["emitted_events"][0]["payload"]["ok"],
        json!(true)
    );

    let run_context_evaluation_index = run_context_event_readback_index + 1;
    assert_eq!(
        responses[run_context_evaluation_index]["id"],
        json!("run-context-evaluation-request-cli")
    );
    assert!(
        responses[run_context_evaluation_index]
            .get("error")
            .is_none(),
        "unexpected RunContext evaluation response: {:?}",
        responses[run_context_evaluation_index]
    );
    assert_eq!(
        responses[run_context_evaluation_index]["result"]["primary"]["kind"],
        "evaluation_request_receipt"
    );
    let run_context_eval =
        responses[run_context_evaluation_index]["result"]["primary"]["evaluation_request_id"]
            .as_str()
            .expect("RunContext evaluation returns request id")
            .to_owned();

    let run_context_assessment_index = run_context_evaluation_index + 1;
    assert_eq!(
        responses[run_context_assessment_index]["id"],
        json!("run-context-assessment-submit-cli")
    );
    assert!(
        responses[run_context_assessment_index]
            .get("error")
            .is_none(),
        "unexpected RunContext assessment response: {:?}",
        responses[run_context_assessment_index]
    );
    assert_eq!(
        responses[run_context_assessment_index]["result"]["primary"]["kind"],
        "assessment_batch_receipt"
    );
    assert_eq!(
        responses[run_context_assessment_index]["result"]["primary"]["evaluation_request_id"],
        run_context_eval
    );
    assert_eq!(
        responses[run_context_assessment_index]["result"]["primary"]["assessment_ids"]
            .as_array()
            .expect("RunContext assessment ids")
            .len(),
        1
    );

    let run_context_assessment_readback_index = run_context_assessment_index + 1;
    assert_eq!(
        responses[run_context_assessment_readback_index]["id"],
        json!("run-context-graph-readback-after-assessment-cli")
    );
    let assessment_readback = &responses[run_context_assessment_readback_index]["result"]["primary"]
        ["items"][0]["payload"];
    assert_eq!(
        assessment_readback["evaluation_request_id"],
        run_context_eval
    );
    assert_eq!(
        assessment_readback["assessment_ids"]
            .as_array()
            .expect("RunContext readback assessment ids")
            .len(),
        1
    );

    let assessment_submit_index = run_context_assessment_readback_index + 1;
    assert_eq!(
        responses[assessment_submit_index]["id"],
        json!("assessment-submit-cli")
    );
    assert!(
        responses[assessment_submit_index].get("error").is_none(),
        "unexpected assessment submit response: {:?}",
        responses[assessment_submit_index]
    );
    assert_eq!(
        responses[assessment_submit_index]["result"]["primary"]["kind"],
        "assessment_batch_receipt"
    );
    assert_eq!(
        responses[assessment_submit_index]["result"]["receipts"][0]["write_kind"],
        "submit_assessments"
    );

    let evaluation_request_index = assessment_submit_index + 1;
    assert_eq!(
        responses[evaluation_request_index]["id"],
        json!("evaluation-request-cli")
    );
    assert!(
        responses[evaluation_request_index].get("error").is_none(),
        "unexpected evaluation request response: {:?}",
        responses[evaluation_request_index]
    );
    assert_eq!(
        responses[evaluation_request_index]["result"]["primary"]["kind"],
        "evaluation_request_receipt"
    );
    assert_eq!(
        responses[evaluation_request_index]["result"]["receipts"][0]["write_kind"],
        "request_evaluation"
    );

    let event_index = evaluation_request_index + 1;
    assert_eq!(responses[event_index]["id"], json!("event-emit-cli"));
    assert!(
        responses[event_index].get("error").is_none(),
        "unexpected event response: {:?}",
        responses[event_index]
    );
    assert_eq!(
        responses[event_index]["result"]["primary"]["kind"],
        "emit_run_event"
    );
    assert_eq!(
        responses[event_index]["result"]["receipts"][0]["write_kind"],
        "emit_run_event"
    );

    let readback_index = event_index + 1;
    assert_eq!(responses[readback_index]["id"], json!("graph-readback-cli"));
    assert!(
        responses[readback_index].get("error").is_none(),
        "unexpected graph readback response: {:?}",
        responses[readback_index]
    );
    let readback_items = responses[readback_index]["result"]["primary"]["items"]
        .as_array()
        .expect("graph readback returns items");
    for expected_event_kind in ["assessment.submit", "evaluation.request", "cli.checked"] {
        assert!(
            readback_items
                .iter()
                .any(|item| item["event_kind"].as_str() == Some(expected_event_kind)),
            "graph readback missing {expected_event_kind}: {readback_items:?}"
        );
    }
    assert!(
        readback_items
            .iter()
            .any(|item| item["payload"]["value"].get("ok").and_then(Value::as_bool) == Some(true)),
        "graph readback missing emitted event payload: {readback_items:?}"
    );

    let sandbox_index = readback_index + 1;
    assert_eq!(responses[sandbox_index]["id"], json!("sandbox-exec-cli"));
    assert!(
        responses[sandbox_index].get("error").is_none(),
        "unexpected sandbox response: {:?}",
        responses[sandbox_index]
    );
    assert_eq!(
        responses[sandbox_index]["result"]["primary"]["kind"],
        "sandbox_exec"
    );
    assert_eq!(
        responses[sandbox_index]["result"]["primary"]["files"]["reports/out.txt"]["bytes"],
        "sandbox artifact\n".len()
    );
    assert_eq!(
        responses[sandbox_index]["result"]["receipts"][1]["call_kind"],
        "sandbox_exec"
    );

    let lm_index = sandbox_index + 1;
    assert_eq!(responses[lm_index]["id"], json!("lm-cli"));
    assert!(
        responses[lm_index].get("error").is_none(),
        "unexpected lm response: {:?}",
        responses[lm_index]
    );
    assert_eq!(
        responses[lm_index]["result"]["primary"]["message"]["content"][0]["text"],
        "cli seam configured lm ok"
    );
    assert_eq!(
        responses[lm_index]["result"]["primary"]["cost"],
        json!({
                "input_tokens": 13,
            "output_tokens": 5,
            "lm_calls": 1
        })
    );
    assert_eq!(
        responses[lm_index]["result"]["receipts"][0]["call_kind"],
        "lm_complete"
    );

    let stage_index = lm_index + 1;
    assert_eq!(responses[stage_index]["id"], json!("stage-unwired"));
    assert_eq!(responses[stage_index]["error"]["code"], json!(-32006));
    assert!(
        responses[stage_index]["error"]["message"]
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

fn unknown_locked_method_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "unknown-locked-method",
        "method": "leaven/not_a_locked_method",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "unknownlockedmethod001",
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

fn workspace_query_after_release_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "workspace-query-after-release-cli",
        "method": "leaven/workspace.read_file",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "workspacereleasedquerycli001",
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
                    "idempotency_key": "workspace-release-query-cli-0001",
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
                    "idempotency_key": "workspace-release-query-cli-0002",
                    "call": {
                        "kind": "workspace_release",
                        "workspace": "ws_cli_materialized",
                        "force": false
                    }
                },
                {
                    "kind": "let",
                    "name": "file",
                    "deps": ["release"],
                    "expr": {
                        "kind": "workspace_query",
                        "workspace": "ws_cli_materialized",
                        "op": {
                            "kind": "read_file",
                            "path": "README.md",
                            "expected_data_classes": ["workspace.file"]
                        }
                    }
                }
            ],
            "return": ["file"],
            "commit": {
                "kind": "graph_writes_atomic",
                "on_stale": "reject"
            }
        }
    })
}

fn workspace_query_requests() -> Vec<Value> {
    [
        (
            "leaven/workspace.read_file",
            "file",
            json!({
                "kind": "read_file",
                "path": "README.md",
                "expected_data_classes": ["workspace.file"]
            }),
        ),
        (
            "leaven/workspace.list",
            "listing",
            json!({"kind": "list", "path": ".", "recursive": false, "max_entries": 10}),
        ),
        (
            "leaven/workspace.stat",
            "stat",
            json!({"kind": "stat", "path": "README.md"}),
        ),
        (
            "leaven/workspace.digest",
            "digest",
            json!({"kind": "digest", "path": "README.md", "algorithm": "sha256"}),
        ),
        (
            "leaven/workspace.snapshot",
            "snapshot",
            json!({"kind": "snapshot"}),
        ),
        (
            "leaven/workspace.capture_artifacts",
            "captured",
            json!({"kind": "capture_artifacts", "paths": ["README.md"], "max_bytes": 4096}),
        ),
        (
            "leaven/workspace.git_log",
            "gitlog",
            json!({"kind": "git_log", "max_entries": 5}),
        ),
        (
            "leaven/workspace.git_diff",
            "gitdiff",
            json!({"kind": "git_diff", "against": "seed", "max_bytes": 4096}),
        ),
        (
            "leaven/workspace.git_status",
            "gitstatus",
            json!({"kind": "git_status", "porcelain": true}),
        ),
    ]
    .into_iter()
    .map(|(method, name, op)| workspace_query_request(method, name, op))
    .collect()
}

fn workspace_query_request(method: &str, name: &str, op: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": format!("workspace-query-cli-{name}"),
        "method": method,
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": format!("workspacecli{name}001"),
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
                    "idempotency_key": format!("workspace-query-cli-{name}-0001"),
                    "call": {
                        "kind": "workspace_materialize",
                        "candidate": "cand_cli",
                        "surface": "program",
                        "mode": "copy_on_write",
                        "lifetime": "manual_release"
                    }
                },
                {
                    "kind": "let",
                    "name": name,
                    "deps": ["workspace"],
                    "expr": {
                        "kind": "workspace_query",
                        "workspace": "ws_cli_materialized",
                        "op": op
                    }
                }
            ],
            "return": [name],
            "commit": {
                "kind": "graph_writes_atomic",
                "on_stale": "reject"
            }
        }
    })
}

fn graph_case_query_requests() -> Vec<Value> {
    vec![
        graph_query_request(),
        case_query_request(
            "leaven/case.load",
            "case-load-cli",
            "case_load",
            &["input", "target", "metadata"],
        ),
        case_query_request(
            "leaven/case.input",
            "case-input-cli",
            "case_input",
            &["input"],
        ),
        case_query_request(
            "leaven/case.target",
            "case-target-cli",
            "case_target",
            &["target"],
        ),
        case_query_request(
            "leaven/case.metadata",
            "case-metadata-cli",
            "case_metadata",
            &["metadata"],
        ),
    ]
}

fn graph_query_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "graph-query-cli",
        "method": "leaven/graph.query",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "graphquerycli001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [{
                "kind": "let",
                "name": "events",
                "expr": {
                    "kind": "graph_query",
                    "source": {
                        "kind": "events"
                    },
                    "projection": {
                        "kind": "ids"
                    },
                    "page": {
                        "limit": 100
                    }
                }
            }],
            "return": ["events"],
            "commit": {
                "kind": "no_graph_writes"
            }
        }
    })
}

fn graph_write_readback_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "graph-readback-cli",
        "method": "leaven/graph.query",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "graphreadbackcli001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [{
                "kind": "let",
                "name": "events",
                "expr": {
                    "kind": "graph_query",
                    "source": {
                        "kind": "events"
                    },
                    "projection": {
                        "kind": "ids"
                    },
                    "page": {
                        "limit": 100
                    }
                }
            }],
            "return": ["events"],
            "commit": {
                "kind": "no_graph_writes"
            }
        }
    })
}

fn run_context_graph_readback_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "run-context-graph-readback-cli",
        "method": "leaven/graph.query",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "runcontextgraphreadbackcli001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [{
                "kind": "let",
                "name": "run_context_graph",
                "expr": {
                    "kind": "graph_query",
                    "source": {
                        "kind": "events",
                        "filter": {
                            "kind": "run_context"
                        }
                    },
                    "projection": {
                        "kind": "ids"
                    },
                    "page": {
                        "limit": 100
                    }
                }
            }],
            "return": ["run_context_graph"],
            "commit": {
                "kind": "no_graph_writes"
            }
        }
    })
}

fn run_context_graph_readback_after_event_request() -> Value {
    let mut request = run_context_graph_readback_request();
    request["id"] = json!("run-context-graph-readback-after-event-cli");
    request
}

fn run_context_graph_readback_after_assessment_request() -> Value {
    let mut request = run_context_graph_readback_request();
    request["id"] = json!("run-context-graph-readback-after-assessment-cli");
    request
}

fn run_context_evaluation_request_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "run-context-evaluation-request-cli",
        "method": "leaven/evaluation.request",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "runcontextevaluationrequestcli001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [{
                "kind": "write",
                "name": "evaluation_request",
                "idempotency_key": "run-context-evaluation-request-cli-0001",
                "write": {
                    "kind": "request_evaluation",
                    "request": {
                        "shape": "independent",
                        "evaluator": "eval_run_context",
                        "candidates": ["cand_run_context_child"],
                        "granularity": "per_case",
                        "purpose": "validation",
                        "set": {
                            "kind": "named",
                            "name": "validation"
                        }
                    }
                }
            }],
            "return": ["evaluation_request"],
            "commit": {
                "kind": "graph_writes_atomic",
                "on_stale": "reject"
            }
        }
    })
}

fn run_context_assessment_submit_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "run-context-assessment-submit-cli",
        "method": "leaven/assessment.submit",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "runcontextassessmentsubmitcli001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [{
                "kind": "write",
                "name": "assessment_batch",
                "idempotency_key": "run-context-assessment-submit-cli-0001",
                "write": {
                    "kind": "submit_assessments",
                    "evaluation_request_id": "evalreq_run_context_latest",
                    "assessments": [{
                        "kind": "independent",
                        "candidate": "cand_run_context_child",
                        "target": {
                            "case": "case_1"
                        },
                        "score": {
                            "value": 1.0,
                            "output": {
                                "kind": "structured",
                                "summary": "RunContext child assessed",
                                "value": {
                                    "candidate": "cand_run_context_child",
                                    "output": "RunContext child assessed",
                                    "ok": true
                                },
                                "visibility": "public",
                                "data_classes": ["candidate.output"]
                            }
                        },
                        "evidence": {
                            "schema_version": "leaven.evidence_envelope.v1",
                            "target_derived": false,
                            "public": {
                                "summary": "RunContext child assessed",
                                "data_classes": ["public"]
                            },
                            "redaction_policy": {
                                "optimizer": "score_only",
                                "reflector": "score_only",
                                "operator": "score_only"
                            },
                            "producer": {
                                "stage_call_id": "sc_run_context_assessment_cli"
                            },
                            "source_receipts": {
                                "read": ["qrec_run_context_assessment_source"],
                                "effect": []
                            }
                        },
                        "replayability": "pure_read"
                    }]
                }
            }],
            "return": ["assessment_batch"],
            "commit": {
                "kind": "graph_writes_atomic",
                "on_stale": "reject"
            }
        }
    })
}

fn run_context_event_emit_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "run-context-event-emit-cli",
        "method": "leaven/event.emit",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "runcontexteventemitcli001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [{
                "kind": "write",
                "name": "run_context_status",
                "idempotency_key": "run-context-event-emit-cli-0001",
                "write": {
                    "kind": "emit_run_event",
                    "event_kind": "run_context.checked",
                    "payload_schema": "fp_schema_sha256_run_context_event",
                    "payload": {"ok": true},
                    "visibility": "public"
                }
            }],
            "return": ["run_context_status"],
            "commit": {
                "kind": "graph_writes_atomic",
                "on_stale": "reject"
            }
        }
    })
}

fn case_query_request(method: &str, id: &str, name: &str, include: &[&str]) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": format!("{name}cli001"),
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [{
                "kind": "let",
                "name": name,
                "expr": {
                    "kind": "case_query",
                    "query": {
                        "kind": "load",
                        "case": {
                            "kind": "case",
                            "run": "run_demo",
                            "id": "case_1"
                        },
                        "include": include,
                        "projection_schema": "fp_schema_sha256_case_projection"
                    }
                }
            }],
            "return": [name],
            "commit": {
                "kind": "no_graph_writes"
            }
        }
    })
}

fn proposal_apply_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "proposal-apply-cli",
        "method": "leaven/proposal.apply",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "proposalapplycli001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [{
                "kind": "write",
                "name": "apply",
                "idempotency_key": "proposal-apply-cli-0001",
                "write": {
                    "kind": "apply_proposal_batch",
                    "proposal_batch": "pb_cli_run_context",
                    "policy": "apply_first_valid"
                }
            }],
            "return": ["apply"],
            "commit": {
                "kind": "graph_writes_atomic",
                "on_stale": "reject"
            }
        }
    })
}

fn assessment_submit_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "assessment-submit-cli",
        "method": "leaven/assessment.submit",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "assessmentsubmitcli001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [{
                "kind": "write",
                "name": "assessments",
                "idempotency_key": "assessment-submit-cli-0001",
                "write": {
                    "kind": "submit_assessments",
                    "evaluation_request_id": "evalreq_cli",
                    "assessments": [{
                        "kind": "independent",
                        "candidate": "cand_cli",
                        "target": {
                            "case": "case_1"
                        },
                        "score": {
                            "value": 1.0,
                            "output": {
                                "kind": "structured",
                                "summary": "cli candidate answered correctly",
                                "value": {
                                    "candidate": "cand_cli",
                                    "output": "cli candidate answered correctly"
                                },
                                "visibility": "public",
                                "data_classes": ["candidate.output"]
                            }
                        },
                        "evidence": {
                            "schema_version": "leaven.evidence_envelope.v1",
                            "target_derived": false,
                            "public": {
                                "summary": "cli candidate answered correctly",
                                "data_classes": ["public"]
                            },
                            "redaction_policy": {
                                "optimizer": "score_only",
                                "reflector": "score_only",
                                "operator": "score_only"
                            },
                            "producer": {
                                "stage_call_id": "sc_assessment_cli"
                            },
                            "source_receipts": {
                                "read": ["qrec_assessment_cli_source"],
                                "effect": []
                            }
                        },
                        "replayability": "pure_read"
                    }]
                }
            }],
            "return": ["assessments"],
            "commit": {
                "kind": "graph_writes_atomic",
                "on_stale": "reject"
            }
        }
    })
}

fn evaluation_request_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "evaluation-request-cli",
        "method": "leaven/evaluation.request",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "evaluationrequestcli001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [{
                "kind": "write",
                "name": "evaluation",
                "idempotency_key": "evaluation-request-cli-0001",
                "write": {
                    "kind": "request_evaluation",
                    "request": {
                        "shape": "independent",
                        "candidates": ["cand_cli"],
                        "set": {
                            "kind": "named",
                            "name": "validation"
                        },
                        "granularity": "per_case",
                        "purpose": "validation",
                        "evaluator": "eval_cli"
                    }
                }
            }],
            "return": ["evaluation"],
            "commit": {
                "kind": "graph_writes_atomic",
                "on_stale": "reject"
            }
        }
    })
}

fn event_emit_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "event-emit-cli",
        "method": "leaven/event.emit",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "eventemitcli001",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "execute"
            },
            "ops": [
                {
                    "kind": "write",
                    "name": "status",
                    "idempotency_key": "event-emit-cli-0001",
                    "write": {
                        "kind": "emit_run_event",
                        "event_kind": "cli.checked",
                        "payload_schema": "fp_schema_sha256_event",
                        "payload": {
                            "ok": true
                        },
                        "visibility": "public"
                    }
                }
            ],
            "return": ["status"],
            "commit": {
                "kind": "graph_writes_atomic",
                "on_stale": "reject"
            }
        }
    })
}

fn sandbox_exec_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "sandbox-exec-cli",
        "method": "leaven/sandbox.exec",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "sandboxexeccli001",
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
                    "idempotency_key": "sandbox-exec-cli-0001",
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
                    "name": "sandboxed",
                    "deps": ["workspace"],
                    "idempotency_key": "sandbox-exec-cli-0002",
                    "call": {
                        "kind": "sandbox_exec",
                        "workspace": "ws_cli_materialized",
                        "argv": [
                            "sh",
                            "-c",
                            "mkdir -p reports && printf 'sandbox artifact\n' > reports/out.txt && printf 'sandbox stdout\n'"
                        ],
                        "timeout_s": 5,
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
            "return": ["sandboxed"],
            "commit": {
                "kind": "graph_writes_atomic",
                "on_stale": "reject"
            }
        }
    })
}

fn seam_capability() -> Value {
    json!({
        "schema_version": "leaven.capability.v1",
        "jti": "jti_cli_seam_stdio",
        "capability_fingerprint": "fp_cap_sha256_leaven_seam_local",
        "policy_fingerprint": "fp_policy_sha256_leaven_seam_local",
        "subject_fingerprint": "fp_subject_sha256_cli",
        "issuer": {
            "kind": "run_engine",
            "id": "engine_cli"
        },
        "subject": {
            "kind": "stage_call",
            "run": "run_cli",
            "stage_call_id": "sc_cli",
            "role": "scorer"
        },
        "audience": ["leaven.acp.worker"],
        "issued_at": "2026-01-01T00:00:00Z",
        "expires_at": "2026-01-01T00:20:00Z",
        "expiry_behavior": "drain_inflight_no_new_ops",
        "token_binding": {
            "kind": "opaque_lookup",
            "token_id": "ltok_cli"
        },
        "revocation": {
            "mode": "issuer_epoch",
            "revocation_epoch": 1,
            "check": "on_every_request"
        },
        "renewal": {
            "mode": "renew_before_expiry",
            "max_extensions": 0,
            "max_total_lifetime_s": 1200
        },
        "grants": [
            {
                "action": "workspace.materialize",
                "resource": {
                    "candidate_ids": ["cand_cli"]
                },
                "constraints": {
                    "workspace_ops": ["materialize"]
                }
            },
            {
                "action": "workspace.release",
                "resource": {
                    "workspace_ids": ["ws_cli_materialized"]
                },
                "constraints": {
                    "workspace_ops": ["release"]
                }
            },
            {
                "action": "workspace.read",
                "resource": {
                    "workspace_ids": ["ws_cli_materialized"]
                },
                "constraints": {
                    "allowed_input_classes": ["candidate.artifact", "workspace.file"],
                    "workspace_ops": [
                        "read_file",
                        "list",
                        "stat",
                        "digest",
                        "snapshot",
                        "capture_artifacts",
                        "git_log",
                        "git_diff",
                        "git_status"
                    ]
                }
            },
            {
                "action": "lm.complete",
                "resource": {},
                "constraints": {
                    "purposes": ["test.cli_seam_stdio"],
                    "models": ["gpt-4.1-mini"],
                    "allowed_input_classes": ["public"]
                }
            },
            {
                "action": "case.read",
                "resource": {
                    "run": "run_demo",
                    "evaluation_request_id": "evalreq_01"
                },
                "constraints": {
                    "case_fields": ["input", "target", "metadata"],
                    "partitions": ["validation"],
                    "allowed_input_classes": ["case.input", "case.target", "case.metadata"]
                }
            },
            {
                "action": "proposal.apply_batch",
                "resource": {},
                "constraints": {
                    "may_apply": true
                }
            },
            {
                "action": "assessment.submit",
                "resource": {
                    "evaluation_request_id": "evalreq_cli"
                },
                "constraints": {},
                "limits": {
                    "max_rows": 1
                }
            },
            {
                "action": "evaluation.request",
                "resource": {
                    "candidate_ids": ["cand_cli"]
                },
                "constraints": {
                    "purposes": ["validation"]
                }
            },
            {
                "action": "event.emit",
                "resource": {},
                "constraints": {}
            },
            {
                "action": "sandbox.exec",
                "resource": {
                    "workspace_ids": ["ws_cli_materialized"]
                },
                "constraints": {
                    "allowed_input_classes": ["public"],
                    "workspace_ops": ["exec"],
                    "allowed_commands": ["sh"]
                },
                "limits": {
                    "timeout_s": 5
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
