use leaven_public_seam::{PlanOperationKind, PublicSeamError, PublicSeamPackage};
use serde_json::{Value, json};

#[test]
fn plan_ir_family_accepts_typed_let_call_write_documents() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let plan = typed_let_call_write_plan();
    let document = package.validate_plan_document(&plan).unwrap();

    assert_eq!(
        document.operation_kinds(),
        &[
            PlanOperationKind::Let,
            PlanOperationKind::Call,
            PlanOperationKind::Write,
        ]
    );
    assert_eq!(document.return_names(), &["status"]);
    assert_eq!(document.consistency_kind(), "latest_at_start");
    assert_eq!(document.mode_kind(), "dry_run");
    assert_eq!(document.commit_kind(), "no_graph_writes");
}

#[test]
fn plan_ir_family_rejects_unknown_core_call_write_and_escape_hatch_ops() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut unknown_core = typed_let_call_write_plan();
    unknown_core["ops"][0]["kind"] = json!("compute");
    assert!(matches!(
        package.validate_plan_document(&unknown_core).unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut unknown_call = typed_let_call_write_plan();
    unknown_call["ops"][1]["call"]["kind"] = json!("provider_magic");
    assert!(matches!(
        package.validate_plan_document(&unknown_call).unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut unknown_write = typed_let_call_write_plan();
    unknown_write["ops"][2]["write"]["kind"] = json!("mutate_graph_anyhow");
    assert!(matches!(
        package.validate_plan_document(&unknown_write).unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut escape_hatch = typed_let_call_write_plan();
    escape_hatch["ops"][0] = json!({
        "kind": "extension",
        "namespace": "x.any",
        "op": "opaque.plan.node",
        "schema_fingerprint": "fp_schema_sha256_any",
        "payload": {
            "runtime_decides": true
        }
    });
    let error = package.validate_plan_document(&escape_hatch).unwrap_err();
    assert!(matches!(error, PublicSeamError::ExampleValidation { .. }));
}

fn typed_let_call_write_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plankind001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "let",
                "name": "prompt",
                "expr": {
                    "kind": "literal",
                    "value": "Say ok",
                    "data_classes": ["public"]
                }
            },
            {
                "kind": "call",
                "name": "completion",
                "deps": ["prompt"],
                "idempotency_key": "plan-call-0001",
                "call": {
                    "kind": "lm_complete",
                    "purpose": "test.plan_ir",
                    "messages": [
                        {
                            "role": "user",
                            "content": [
                                {
                                    "kind": "text",
                                    "text": "Say ok"
                                }
                            ]
                        }
                    ],
                    "output": {
                        "kind": "final_message",
                        "max_bytes": 1024
                    },
                    "input_classes": ["public"]
                }
            },
            {
                "kind": "write",
                "name": "status",
                "deps": ["completion"],
                "idempotency_key": "plan-write-0001",
                "write": {
                    "kind": "emit_run_event",
                    "event_kind": "plan.ir.checked",
                    "payload_schema": "fp_schema_sha256_planir",
                    "payload": {
                        "ok": true
                    },
                    "visibility": "public"
                }
            }
        ],
        "return": ["status"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}
