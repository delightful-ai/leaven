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
            "candidate.artifact".to_owned(),
            "candidate.output".to_owned(),
            "case.target".to_owned(),
            "completion.raw".to_owned(),
            "prompt.raw".to_owned(),
            "public".to_owned(),
            "transcript.raw".to_owned()
        ]
    )));
}

#[test]
fn plan_result_accepts_object_form_evidence_receipt_refs() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut result = evidence_backed_result();
    let read_ref = receipt_ref_for_result(&result, "qrec_target");
    let effect_ref = receipt_ref_for_result(&result, "lmrec_score");
    let write_ref = receipt_ref_for_result(&result, "wrec_assessment");
    let evidence = &mut result["values"]["assessment_rows"]["items"][0]["evidence"];
    evidence["source_receipts"]["read"] = json!([read_ref]);
    evidence["source_receipts"]["effect"] = json!([effect_ref]);
    evidence["source_receipts"]["write"] = json!([write_ref]);
    evidence["public"]["trace_refs"][0]["receipt"] = receipt_ref("lmrec_score");
    evidence["trace_refs"][0]["receipt"] = receipt_ref("lmrec_score");

    let result = package.validate_plan_result_document(&result).unwrap();

    assert_eq!(result.value_count(), 2);
    assert_eq!(result.receipt_count(), 3);
}

#[test]
fn plan_result_accepts_unreceipted_evidence_trace_refs() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut result = evidence_backed_result();
    let evidence = &mut result["values"]["assessment_rows"]["items"][0]["evidence"];
    evidence["public"]["trace_refs"][0]
        .as_object_mut()
        .unwrap()
        .remove("receipt");
    evidence["trace_refs"][0]
        .as_object_mut()
        .unwrap()
        .remove("receipt");

    let result = package.validate_plan_result_document(&result).unwrap();

    assert_eq!(result.value_count(), 2);
    assert_eq!(result.receipt_count(), 3);
}

#[test]
fn plan_result_preserves_evidence_source_receipt_trace_visibility() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut result = evidence_backed_result();
    result["receipts"][0]["trace_refs"] =
        json!([receipt_trace_ref("trace_read_target", "case.target")]);
    result["receipts"][1]["trace_refs"] =
        json!([receipt_trace_ref("trace_lm_transcript", "transcript.raw")]);
    result["receipts"][2]["trace_refs"] =
        json!([receipt_trace_ref("trace_write_prompt", "prompt.raw")]);

    let result = package.validate_plan_result_document(&result).unwrap();

    assert_eq!(result.value_count(), 2);
    assert_eq!(result.receipt_count(), 3);
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
        ["effect"] = json!(["qrec_target", "lmrec_score"]);
    assert!(matches!(
        package
            .validate_plan_result_document(&wrong_effect_receipt_kind)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));

    let mut object_form_wrong_effect_receipt_kind = evidence_backed_result();
    let query_ref = receipt_ref_for_result(&object_form_wrong_effect_receipt_kind, "qrec_target");
    let effect_ref = receipt_ref_for_result(&object_form_wrong_effect_receipt_kind, "lmrec_score");
    object_form_wrong_effect_receipt_kind["values"]["assessment_rows"]["items"][0]["evidence"]["source_receipts"]
        ["effect"] = json!([query_ref, effect_ref]);
    assert!(matches!(
        package
            .validate_plan_result_document(&object_form_wrong_effect_receipt_kind)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));

    let mut stale_read_fingerprint = evidence_backed_result();
    let mut stale_ref = receipt_ref_for_result(&stale_read_fingerprint, "qrec_target");
    stale_ref["fingerprint"] = json!("fp_receipt_sha256_stale");
    stale_read_fingerprint["values"]["assessment_rows"]["items"][0]["evidence"]["source_receipts"]
        ["read"] = json!([stale_ref]);
    assert!(matches!(
        package
            .validate_plan_result_document(&stale_read_fingerprint)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut stale_effect_fingerprint = evidence_backed_result();
    let mut stale_ref = receipt_ref_for_result(&stale_effect_fingerprint, "lmrec_score");
    stale_ref["fingerprint"] = json!("fp_receipt_sha256_stale");
    stale_effect_fingerprint["values"]["assessment_rows"]["items"][0]["evidence"]["source_receipts"]
        ["effect"] = json!([stale_ref]);
    assert!(matches!(
        package
            .validate_plan_result_document(&stale_effect_fingerprint)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut stale_write_fingerprint = evidence_backed_result();
    let mut stale_ref = receipt_ref_for_result(&stale_write_fingerprint, "wrec_assessment");
    stale_ref["fingerprint"] = json!("fp_receipt_sha256_stale");
    stale_write_fingerprint["values"]["assessment_rows"]["items"][0]["evidence"]["source_receipts"]
        ["write"] = json!([stale_ref]);
    assert!(matches!(
        package
            .validate_plan_result_document(&stale_write_fingerprint)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut undeclared_trace_receipt = evidence_backed_result();
    undeclared_trace_receipt["values"]["assessment_rows"]["items"][0]["evidence"]["source_receipts"]
        ["effect"] = json!([]);
    assert!(matches!(
        package
            .validate_plan_result_document(&undeclared_trace_receipt)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));
}

#[test]
fn plan_result_rejects_evidence_receipt_trace_visibility_conflicts() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut hidden_receipt_class = evidence_backed_result();
    hidden_receipt_class["receipts"][1]["trace_refs"] = json!([receipt_trace_ref(
        "trace_lm_provider_secret",
        "external.secret"
    )]);
    let error = package
        .validate_plan_result_document(&hidden_receipt_class)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("trace data class `external.secret` is not covered by evidence data_classes"),
        "{error}"
    );

    let mut value_visibility_gap = evidence_backed_result();
    value_visibility_gap["receipts"][1]["trace_refs"] = json!([receipt_trace_ref(
        "trace_lm_provider_secret",
        "external.secret"
    )]);
    value_visibility_gap["values"]["assessment_rows"]["items"][0]["evidence"]["data_classes"]
        .as_array_mut()
        .unwrap()
        .push(json!("external.secret"));
    let error = package
        .validate_plan_result_document(&value_visibility_gap)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("nested visibility data class `external.secret`"),
        "{error}"
    );
}

#[test]
fn plan_result_rejects_nested_score_output_data_class_gaps() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut missing_candidate_output = evidence_backed_result();
    missing_candidate_output["values"]["assessment_rows"]["data_classes"] = json!([
        "candidate.artifact",
        "case.target",
        "completion.raw",
        "prompt.raw",
        "public",
        "transcript.raw"
    ]);

    assert!(matches!(
        package
            .validate_plan_result_document(&missing_candidate_output)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn plan_result_rejects_nested_score_blob_ref_data_class_gaps() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut missing_blob_class = evidence_backed_result();
    missing_blob_class["values"]["assessment_rows"]["data_classes"] = json!([
        "candidate.output",
        "case.target",
        "completion.raw",
        "prompt.raw",
        "public",
        "transcript.raw"
    ]);

    assert!(matches!(
        package
            .validate_plan_result_document(&missing_blob_class)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn plan_result_rejects_nested_trace_ref_data_class_gaps() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut missing_score_trace_class = evidence_backed_result();
    missing_score_trace_class["values"]["assessment_rows"]["data_classes"] = json!([
        "candidate.output",
        "candidate.artifact",
        "case.target",
        "prompt.raw",
        "public",
        "transcript.raw"
    ]);

    assert!(matches!(
        package
            .validate_plan_result_document(&missing_score_trace_class)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut missing_evidence_trace_class = evidence_backed_result();
    missing_evidence_trace_class["values"]["assessment_rows"]["data_classes"] = json!([
        "candidate.output",
        "candidate.artifact",
        "case.target",
        "completion.raw",
        "prompt.raw",
        "public"
    ]);

    assert!(matches!(
        package
            .validate_plan_result_document(&missing_evidence_trace_class)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut missing_evidence_top_level_trace_class = evidence_backed_result();
    missing_evidence_top_level_trace_class["values"]["assessment_rows"]["data_classes"] = json!([
        "candidate.output",
        "candidate.artifact",
        "case.target",
        "completion.raw",
        "public",
        "transcript.raw"
    ]);

    assert!(matches!(
        package
            .validate_plan_result_document(&missing_evidence_top_level_trace_class)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn plan_result_rejects_nested_evidence_hidden_target_derivation_flag() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut hidden_target = evidence_backed_result();
    hidden_target["values"]["assessment_rows"]["items"][0]["evidence"]["target_derived"] =
        json!(false);

    assert!(matches!(
        package
            .validate_plan_result_document(&hidden_target)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));
}

#[test]
fn plan_result_rejects_submit_assessment_result_hashes_that_do_not_bind_values() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut wrong_submit_assessments_request_hash = evidence_backed_result();
    wrong_submit_assessments_request_hash["receipts"][2]["request_hash"] =
        json!("fp_request_sha256_same_prefix_wrong_submit_assessments_scope");
    assert!(matches!(
        package
            .validate_plan_result_document(&wrong_submit_assessments_request_hash)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut wrong_submit_assessments_hash = evidence_backed_result();
    wrong_submit_assessments_hash["receipts"][2]["result_hash"] =
        json!("fp_result_sha256_same_prefix_wrong_submit_assessments_value");

    assert!(matches!(
        package
            .validate_plan_result_document(&wrong_submit_assessments_hash)
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
                            "output": score_output_record()
                        },
                        "evidence": target_derived_evidence()
                    }
                ],
                "graph_revision": "rev_final",
                "data_classes": [
                    "candidate.output",
                    "candidate.artifact",
                    "case.target",
                    "completion.raw",
                    "prompt.raw",
                    "public",
                    "transcript.raw"
                ],
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

fn score_output_record() -> Value {
    json!({
        "kind": "text",
        "summary": "model answer matched target",
        "value": "model answer matched target",
        "visibility": "public",
        "data_classes": ["candidate.output"],
        "blob_ref": {
            "kind": "blob_ref",
            "id": "blob_candidate_artifact",
            "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "bytes": 32,
            "data_classes": ["candidate.artifact"]
        },
        "trace_refs": [
            {
                "kind": "runner_completion",
                "id": "trace_candidate_output",
                "visibility": "redacted_completion",
                "data_classes": ["completion.raw"],
                "receipt": "lmrec_score"
            }
        ]
    })
}

fn bind_result_hashes(mut result: Value) -> Value {
    let values = result["values"].as_object().unwrap().clone();
    for receipt in result["receipts"].as_array_mut().unwrap() {
        if receipt["kind"].as_str() == Some("write")
            && receipt["write_kind"].as_str() == Some("submit_assessments")
        {
            receipt["request_hash"] = json!(prefixed_jcs_hash(
                "fp_request_sha256_",
                &json!({
                    "schema_version": "leaven.submit_assessments_request.v1",
                    "evaluation_request_id": receipt["evaluation_request_id"],
                    "assessment_ids": receipt["assessment_ids"]
                }),
            ));
        }
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
        "data_classes": ["case.target", "prompt.raw", "transcript.raw"],
        "public": {
            "feedback": "matched target",
            "data_classes": ["case.target"],
            "trace_refs": [
                {
                    "kind": "judge_trace",
                    "id": "trace_target_feedback",
                    "visibility": "redacted_transcript",
                    "data_classes": ["transcript.raw"],
                    "receipt": "lmrec_score"
                }
            ]
        },
        "trace_refs": [
            {
                "kind": "prompt_trace",
                "id": "trace_target_prompt",
                "visibility": "redacted_prompt",
                "data_classes": ["prompt.raw"],
                "receipt": "lmrec_score"
            }
        ],
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

fn receipt_ref(id: &str) -> Value {
    json!({
        "kind": "receipt",
        "id": id,
        "fingerprint": "fp_receipt_sha256_evidence"
    })
}

fn receipt_trace_ref(id: &str, data_class: &str) -> Value {
    json!({
        "kind": "receipt_trace",
        "id": id,
        "visibility": "redacted_transcript",
        "data_classes": [data_class]
    })
}

fn receipt_ref_for_result(result: &Value, id: &str) -> Value {
    let receipt = result["receipts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|receipt| receipt["receipt"].as_str() == Some(id))
        .unwrap();
    json!({
        "kind": "receipt",
        "id": id,
        "fingerprint": prefixed_jcs_hash("fp_receipt_sha256_", receipt)
    })
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}
