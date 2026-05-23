use leaven_public_seam::{PublicSeamError, PublicSeamPackage};
use serde_json::{Value, json};

#[test]
fn plan_result_accepts_typed_success_and_failure_envelopes() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let success = package
        .validate_plan_result_document(&typed_success_result())
        .unwrap();
    assert_eq!(success.plan_id(), "result001");
    assert_eq!(success.base_revision(), "rev_base");
    assert_eq!(success.final_revision(), "rev_base");
    assert_eq!(success.value_count(), 1);
    assert_eq!(success.receipt_count(), 1);
    assert_eq!(success.error_count(), 0);
    assert_eq!(success.charge_count(), 0);
    assert_eq!(success.value_kinds(), &["graph_set"]);
    assert_eq!(success.receipt_kinds(), &["query"]);

    let failure = package
        .validate_plan_result_document(&typed_failure_result())
        .unwrap();
    assert_eq!(failure.plan_id(), "resultfail001");
    assert_eq!(failure.value_count(), 0);
    assert_eq!(failure.receipt_count(), 1);
    assert_eq!(failure.error_count(), 1);
    assert_eq!(failure.charge_count(), 1);
    assert_eq!(failure.receipt_kinds(), &["call"]);
}

#[test]
fn plan_result_rejects_generic_or_untyped_result_payloads() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    assert!(matches!(
        package
            .validate_plan_result_document(&json!({
                "status": "ok",
                "output": {
                    "whatever": true
                }
            }))
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut missing_fingerprint = typed_success_result();
    missing_fingerprint
        .as_object_mut()
        .unwrap()
        .remove("capability_fingerprint");
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_fingerprint)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut untyped_error = typed_failure_result();
    untyped_error["errors"] = json!(["provider exploded"]);
    assert!(matches!(
        package
            .validate_plan_result_document(&untyped_error)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut unknown_error = typed_failure_result();
    unknown_error["errors"][0]["code"] = json!("made_up_error");
    assert!(matches!(
        package
            .validate_plan_result_document(&unknown_error)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn plan_result_rejects_receipts_without_audit_timing() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut missing_started_at = typed_success_result();
    missing_started_at["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("started_at");
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_started_at)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut missing_completed_at = typed_success_result();
    missing_completed_at["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("completed_at");
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_completed_at)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

fn typed_success_result() -> Value {
    json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": "result001",
        "capability_fingerprint": "fp_cap_sha256_resultcap",
        "policy_fingerprint": "fp_policy_sha256_resultpolicy",
        "base_revision": "rev_base",
        "final_revision": "rev_base",
        "replayability_summary": "pure_read",
        "values": {
            "rows": {
                "kind": "graph_set",
                "items": [
                    {
                        "kind": "candidate_summary",
                        "candidate": "cand_alpha",
                        "artifact_identity": "artifact_sha256_alpha"
                    }
                ],
                "graph_revision": "rev_base",
                "data_classes": ["public"],
                "replayability": "pure_read",
                "receipt": "qrec_rows"
            }
        },
        "receipts": [
            {
                "kind": "query",
                "receipt": "qrec_rows",
                "op_var": "rows",
                "started_at": "2026-05-23T12:00:00Z",
                "completed_at": "2026-05-23T12:00:01Z",
                "op_hash": "fp_query_sha256_rows",
                "result_hash": "fp_result_sha256_rows",
                "graph_revision": "rev_base",
                "status": "succeeded",
                "read_scope_fingerprint": "fp_scope_sha256_read",
                "projection_fingerprint": "fp_projection_sha256_rows"
            }
        ],
        "redactions": [],
        "charges": [],
        "errors": []
    })
}

fn typed_failure_result() -> Value {
    json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": "resultfail001",
        "capability_fingerprint": "fp_cap_sha256_resultcap",
        "policy_fingerprint": "fp_policy_sha256_resultpolicy",
        "base_revision": "rev_base",
        "final_revision": "rev_base",
        "replayability_summary": "has_declared_external_effects",
        "values": {},
        "receipts": [
            {
                "kind": "call",
                "receipt": "lmrec_failed",
                "op_var": "completion",
                "started_at": "2026-05-23T12:00:00Z",
                "completed_at": "2026-05-23T12:00:02Z",
                "call_kind": "lm_complete",
                "request_hash": "fp_request_sha256_lm",
                "result_hash": "fp_result_sha256_lm_failed",
                "runtime_fingerprint": "fp_runtime_sha256_lm",
                "status": "failed",
                "error": {
                    "code": "provider_error",
                    "message": "provider failed",
                    "receipt": "lmrec_failed",
                    "retryable": true
                },
                "cost": {
                    "usd_micro": 100
                },
                "charge_receipts": ["chargerec_lm_failed"]
            }
        ],
        "redactions": [],
        "charges": [
            {
                "receipt": "chargerec_lm_failed",
                "source_receipt": "lmrec_failed",
                "cost": {
                    "usd_micro": 100
                },
                "ledger_scope": "plan",
                "charged_at": "2026-05-23T12:00:02Z"
            }
        ],
        "errors": [
            {
                "code": "provider_error",
                "message": "provider failed",
                "receipt": "lmrec_failed",
                "retryable": true
            }
        ]
    })
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}
