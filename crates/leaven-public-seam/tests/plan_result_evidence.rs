use leaven_public_seam::{PublicSeamError, PublicSeamPackage};
use serde_json::{Value, json};

#[test]
fn plan_result_preserves_nested_evidence_visibility_data_classes_and_receipts() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let result = package
        .validate_plan_result_document(&evidence_backed_result())
        .unwrap();

    assert_eq!(result.value_count(), 2);
    assert_eq!(result.receipt_count(), 3);
    assert!(result.value_data_classes().contains(&(
        "assessment_rows".to_owned(),
        vec![
            "candidate.output".to_owned(),
            "case.target".to_owned(),
            "public".to_owned()
        ]
    )));
}

#[test]
fn plan_result_rejects_evidence_source_receipts_that_are_missing_or_wrong_kind() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut missing_read_receipt = evidence_backed_result();
    missing_read_receipt["values"]["assessment_rows"]["items"][0]["evidence"]["source_receipts"]
        ["read"] = json!(["qrec_missing"]);
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_read_receipt)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut wrong_effect_receipt_kind = evidence_backed_result();
    wrong_effect_receipt_kind["values"]["assessment_rows"]["items"][0]["evidence"]["source_receipts"]
        ["effect"] = json!(["qrec_target"]);
    assert!(matches!(
        package
            .validate_plan_result_document(&wrong_effect_receipt_kind)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn plan_result_rejects_nested_score_output_data_class_gaps() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut missing_candidate_output = evidence_backed_result();
    missing_candidate_output["values"]["assessment_rows"]["data_classes"] =
        json!(["case.target", "public"]);

    assert!(matches!(
        package
            .validate_plan_result_document(&missing_candidate_output)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

fn evidence_backed_result() -> Value {
    bind_result_hashes(json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": "result_evidence001",
        "capability_fingerprint": "fp_cap_sha256_evidence",
        "policy_fingerprint": "fp_policy_sha256_evidence",
        "base_revision": "rev_base",
        "final_revision": "rev_final",
        "replayability_summary": "has_declared_external_effects",
        "values": {
            "assessment_rows": {
                "kind": "graph_set",
                "items": [
                    {
                        "kind": "assessment_summary",
                        "assessment": "assess_evidence",
                        "score": {
                            "value": 1.0,
                            "output": {
                                "kind": "text",
                                "summary": "model answer matched target",
                                "value": "model answer matched target",
                                "visibility": "public",
                                "data_classes": ["candidate.output"]
                            }
                        },
                        "evidence": target_derived_evidence()
                    }
                ],
                "graph_revision": "rev_final",
                "data_classes": ["candidate.output", "case.target", "public"],
                "replayability": "has_declared_external_effects"
            },
            "assessment_batch": {
                "kind": "assessment_batch_receipt",
                "assessment_ids": ["assess_evidence"],
                "evaluation_request_id": "evalreq_evidence",
                "per_assessment": [
                    {
                        "assessment": "assess_evidence",
                        "replayability": "has_declared_external_effects",
                        "effect_receipts": ["lmrec_score"]
                    }
                ],
                "status": "committed",
                "graph_revision": "rev_final",
                "data_classes": ["public"],
                "replayability": "has_declared_external_effects",
                "receipt": "wrec_assessment"
            }
        },
        "receipts": [
            {
                "kind": "query",
                "receipt": "qrec_target",
                "started_at": "2026-05-23T12:00:00Z",
                "completed_at": "2026-05-23T12:00:01Z",
                "op_hash": "fp_query_sha256_target",
                "result_hash": "fp_result_sha256_target",
                "graph_revision": "rev_base",
                "status": "succeeded",
                "read_scope_fingerprint": "fp_scope_sha256_target",
                "projection_fingerprint": "fp_projection_sha256_target"
            },
            {
                "kind": "call",
                "receipt": "lmrec_score",
                "started_at": "2026-05-23T12:00:01Z",
                "completed_at": "2026-05-23T12:00:02Z",
                "call_kind": "lm_complete",
                "request_hash": "fp_request_sha256_score",
                "result_hash": "fp_result_sha256_score",
                "runtime_fingerprint": "fp_runtime_sha256_score",
                "status": "succeeded"
            },
            {
                "kind": "write",
                "receipt": "wrec_assessment",
                "started_at": "2026-05-23T12:00:02Z",
                "completed_at": "2026-05-23T12:00:03Z",
                "write_kind": "submit_assessments",
                "request_hash": "fp_request_sha256_assessment",
                "result_hash": "fp_result_sha256_assessment",
                "base_revision": "rev_base",
                "committed_revision": "rev_final",
                "status": "succeeded",
                "evaluation_request_id": "evalreq_evidence",
                "assessment_ids": ["assess_evidence"]
            }
        ],
        "redactions": [],
        "charges": [],
        "errors": []
    }))
}

fn bind_result_hashes(mut result: Value) -> Value {
    let values = result["values"].as_object().unwrap().clone();
    for receipt in result["receipts"].as_array_mut().unwrap() {
        let receipt_id = receipt["receipt"].as_str().unwrap();
        let Some((name, value)) = values.iter().find(|(_, value)| {
            value
                .as_object()
                .and_then(|object| object.get("receipt"))
                .and_then(Value::as_str)
                == Some(receipt_id)
        }) else {
            continue;
        };
        let schema_version = match receipt["kind"].as_str().unwrap() {
            "query" => "leaven.plan_query_result.v1",
            "call" => "leaven.plan_call_result.v1",
            "write" => "leaven.plan_write_result.v1",
            other => panic!("unexpected receipt kind {other}"),
        };
        let op_name = receipt["op_var"].as_str().unwrap_or(name);
        receipt["result_hash"] = json!(prefixed_jcs_hash(
            "fp_result_sha256_",
            &json!({
                "schema_version": schema_version,
                "name": op_name,
                "value": value
            }),
        ));
    }
    result
}

fn prefixed_jcs_hash(prefix: &str, value: &Value) -> String {
    format!(
        "{prefix}{}",
        jcs_canonicalize::sha256_jcs_hex(value).unwrap()
    )
}

fn target_derived_evidence() -> Value {
    json!({
        "schema_version": "leaven.evidence_envelope.v1",
        "target_derived": true,
        "data_classes": ["case.target"],
        "public": {
            "feedback": "matched target",
            "data_classes": ["case.target"]
        },
        "redaction_policy": {
            "optimizer": "score_only",
            "reflector": "score_only",
            "operator": "full"
        },
        "producer": {
            "stage_call_id": "sc_evidence"
        },
        "source_receipts": {
            "read": ["qrec_target"],
            "effect": ["lmrec_score"],
            "write": ["wrec_assessment"]
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
