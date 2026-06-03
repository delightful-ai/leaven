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
                "capability": seam_capability(),
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
        for request in &workspace_queries {
            write_json_line(&mut stdin, request);
        }
        write_json_line(&mut stdin, &event_emit_request());
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
        6 + workspace_queries.len(),
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

    for (offset, request) in workspace_queries.iter().enumerate() {
        let response = &responses[3 + offset];
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
    }

    let event_index = 3 + workspace_queries.len();
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

    let lm_index = event_index + 1;
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
            "role": "runner"
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
                "action": "event.emit",
                "resource": {},
                "constraints": {}
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
