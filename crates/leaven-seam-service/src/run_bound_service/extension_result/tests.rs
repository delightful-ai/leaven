use serde_json::json;

use super::{
    assessment_submit_extension_result, evaluation_request_extension_result,
    proposal_apply_extension_result,
};

#[test]
fn apply_extension_projection_rejects_wrong_primary_kind() {
    let mut result = apply_plan_result();
    result["values"]["apply"]["kind"] = json!("proposal_batch_receipt");

    let error = proposal_apply_extension_result(&result).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("graph-backed public-seam projection field `/values/apply/kind`"),
        "{error}"
    );
}

#[test]
fn evaluation_extension_projection_requires_matching_write_receipt() {
    let mut result = evaluation_request_plan_result();
    result["receipts"][0]["write_kind"] = json!("submit_assessments");

    let error = evaluation_request_extension_result(&result).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("expected at least one matching write receipt"),
        "{error}"
    );
}

#[test]
fn assessment_extension_projection_rejects_untyped_per_assessment_rows() {
    let mut result = assessment_submit_plan_result();
    result["values"]["assessment_batch"]["per_assessment"][0] =
        json!({"assessment": {"id": "assess_1"}, "replayability": "fully_managed"});

    let error = assessment_submit_extension_result(&result).unwrap_err();

    assert!(
        error.to_string().contains("assessment must be a string"),
        "{error}"
    );
}

fn apply_plan_result() -> serde_json::Value {
    json!({
        "capability_fingerprint": "fp_cap_sha256_run_bound",
        "policy_fingerprint": "fp_policy_sha256_run_bound",
        "values": {
            "apply": {
                "kind": "apply_receipt",
                "created_candidates": ["cand_1"],
                "status": "committed",
                "graph_revision": "rev_final",
                "data_classes": ["public"],
                "replayability": "fully_managed",
                "receipt": "wrec_apply"
            }
        },
        "receipts": [{
            "kind": "write",
            "receipt": "wrec_apply",
            "op_var": "apply",
            "started_at": "2026-06-04T00:00:00Z",
            "completed_at": "2026-06-04T00:00:01Z",
            "write_kind": "apply_proposal_batch",
            "request_hash": "fp_request_sha256_apply",
            "result_hash": "fp_result_sha256_apply",
            "base_revision": "rev_base",
            "committed_revision": "rev_final",
            "status": "succeeded",
            "created_candidates": ["cand_1"]
        }],
        "redactions": []
    })
}

fn evaluation_request_plan_result() -> serde_json::Value {
    json!({
        "capability_fingerprint": "fp_cap_sha256_run_bound",
        "policy_fingerprint": "fp_policy_sha256_run_bound",
        "values": {
            "evaluation_request": {
                "kind": "evaluation_request_receipt",
                "receipt": "wrec_evalreq_1",
                "evaluation_request_id": "evalreq_1",
                "status": "recorded",
                "graph_revision": "rev_base",
                "data_classes": ["public"],
                "replayability": "fully_managed"
            }
        },
        "receipts": [{
            "kind": "write",
            "write_kind": "request_evaluation",
            "receipt": "wrec_evalreq_1",
            "started_at": "2026-06-04T00:00:00Z",
            "completed_at": "2026-06-04T00:00:01Z",
            "request_hash": "fp_request_sha256_eval",
            "result_hash": "fp_result_sha256_eval",
            "base_revision": "rev_base",
            "committed_revision": "rev_base",
            "status": "succeeded",
            "evaluation_request_id": "evalreq_1"
        }],
        "redactions": []
    })
}

fn assessment_submit_plan_result() -> serde_json::Value {
    json!({
        "capability_fingerprint": "fp_cap_sha256_run_bound",
        "policy_fingerprint": "fp_policy_sha256_run_bound",
        "values": {
            "assessment_batch": {
                "kind": "assessment_batch_receipt",
                "assessment_ids": ["assess_1"],
                "evaluation_request_id": "evalreq_1",
                "per_assessment": [{
                    "assessment": "assess_1",
                    "replayability": "fully_managed"
                }],
                "status": "committed",
                "graph_revision": "rev_final",
                "data_classes": ["public"],
                "replayability": "fully_managed",
                "receipt": "wrec_assess"
            }
        },
        "receipts": [{
            "kind": "write",
            "receipt": "wrec_assess",
            "op_var": "assessment_batch",
            "started_at": "2026-06-04T00:00:00Z",
            "completed_at": "2026-06-04T00:00:01Z",
            "write_kind": "submit_assessments",
            "request_hash": "fp_request_sha256_assess",
            "result_hash": "fp_result_sha256_assess",
            "base_revision": "rev_base",
            "committed_revision": "rev_final",
            "status": "succeeded",
            "evaluation_request_id": "evalreq_1",
            "assessment_ids": ["assess_1"]
        }],
        "redactions": []
    })
}
