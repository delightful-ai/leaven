use crate::support::workspace_root;
use leaven_public_seam::{EvaluationJobKind, PublicSeamError, PublicSeamPackage};
use serde_json::{Value, json};

#[test]
fn evaluation_job_preserves_identity_for_all_request_shapes() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let independent = package
        .validate_evaluation_job_document(&evaluation_job(&json!({
            "kind": "independent",
            "candidates": ["cand_a", "cand_b"]
        })))
        .unwrap();
    assert_eq!(independent.kind(), EvaluationJobKind::Independent);
    assert_eq!(independent.request_id(), "evalreq_score_output");
    assert_eq!(independent.evaluator_id(), "primary");
    assert_eq!(
        independent.evaluator_fingerprint(),
        "fp_runtime_sha256_evaluator"
    );
    assert_eq!(
        independent.capability_fingerprint(),
        "fp_cap_sha256_evaluator"
    );
    assert_eq!(independent.base_revision(), "rev_base");
    assert_eq!(independent.deadline_at(), "2026-05-23T13:00:00Z");
    assert_eq!(independent.resolved_set_id(), "rset_eval");
    assert_eq!(
        independent.case_ids(),
        &["case_1".to_owned(), "case_2".to_owned()]
    );
    assert_eq!(
        independent.candidate_ids(),
        &["cand_a".to_owned(), "cand_b".to_owned()]
    );
    assert_eq!(independent.case_count(), 2);
    assert_eq!(independent.candidate_count(), 2);
    assert_eq!(independent.pair_count(), 0);

    let pairwise = package
        .validate_evaluation_job_document(&evaluation_job(&json!({
            "kind": "pairwise",
            "pairs": [{"left": "cand_a", "right": "cand_b"}]
        })))
        .unwrap();
    assert_eq!(pairwise.kind(), EvaluationJobKind::Pairwise);
    assert_eq!(
        pairwise.candidate_ids(),
        &["cand_a".to_owned(), "cand_b".to_owned()]
    );
    assert_eq!(pairwise.candidate_count(), 2);
    assert_eq!(pairwise.pair_count(), 1);

    let listwise = package
        .validate_evaluation_job_document(&evaluation_job(&json!({
            "kind": "listwise",
            "candidates": ["cand_a", "cand_b", "cand_c"]
        })))
        .unwrap();
    assert_eq!(listwise.kind(), EvaluationJobKind::Listwise);
    assert_eq!(
        listwise.candidate_ids(),
        &[
            "cand_a".to_owned(),
            "cand_b".to_owned(),
            "cand_c".to_owned()
        ]
    );
    assert_eq!(listwise.candidate_count(), 3);
}

#[test]
fn evaluation_job_rejects_missing_identity_deadline_or_capability() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut missing_deadline = evaluation_job(&json!({
        "kind": "independent",
        "candidates": ["cand_a"]
    }));
    missing_deadline
        .as_object_mut()
        .unwrap()
        .remove("deadline_at");
    assert!(matches!(
        package
            .validate_evaluation_job_document(&missing_deadline)
            .unwrap_err(),
        PublicSeamError::InvalidEvaluationJob { .. }
    ));

    let mut missing_evaluator_fingerprint = evaluation_job(&json!({
        "kind": "independent",
        "candidates": ["cand_a"]
    }));
    missing_evaluator_fingerprint
        .as_object_mut()
        .unwrap()
        .remove("evaluator_fingerprint");
    assert!(matches!(
        package
            .validate_evaluation_job_document(&missing_evaluator_fingerprint)
            .unwrap_err(),
        PublicSeamError::InvalidEvaluationJob { .. }
    ));

    let mut missing_capability = evaluation_job(&json!({
        "kind": "independent",
        "candidates": ["cand_a"]
    }));
    missing_capability
        .as_object_mut()
        .unwrap()
        .remove("capability_fingerprint");
    assert!(matches!(
        package
            .validate_evaluation_job_document(&missing_capability)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    for field in ["evaluation_request_id", "base_revision", "evaluator_id"] {
        let mut missing = evaluation_job(&json!({
            "kind": "independent",
            "candidates": ["cand_a"]
        }));
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            matches!(
                package
                    .validate_evaluation_job_document(&missing)
                    .unwrap_err(),
                PublicSeamError::ExampleValidation { .. }
                    | PublicSeamError::InvalidEvaluationJob { .. }
            ),
            "missing {field} should fail"
        );
    }
}

#[test]
fn evaluation_job_rejects_unresolved_case_sets_and_invalid_pairs() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut mismatched_case_count = evaluation_job(&json!({
        "kind": "independent",
        "candidates": ["cand_a"]
    }));
    mismatched_case_count["resolved_set"]["case_count"] = json!(3);
    assert!(matches!(
        package
            .validate_evaluation_job_document(&mismatched_case_count)
            .unwrap_err(),
        PublicSeamError::InvalidEvaluationJob { .. }
    ));

    let mut unresolved_case_set = evaluation_job(&json!({
        "kind": "independent",
        "candidates": ["cand_a"]
    }));
    unresolved_case_set["resolved_set"]
        .as_object_mut()
        .unwrap()
        .remove("case_ids");
    assert!(matches!(
        package
            .validate_evaluation_job_document(&unresolved_case_set)
            .unwrap_err(),
        PublicSeamError::InvalidEvaluationJob { .. }
    ));

    let mut cursor_only = evaluation_job(&json!({
        "kind": "independent",
        "candidates": ["cand_a"]
    }));
    cursor_only["resolved_set"]
        .as_object_mut()
        .unwrap()
        .remove("case_ids");
    cursor_only["resolved_set"]["case_cursor"] = json!("cur_cases");
    assert!(matches!(
        package
            .validate_evaluation_job_document(&cursor_only)
            .unwrap_err(),
        PublicSeamError::InvalidEvaluationJob { .. }
    ));

    for field in ["case_set_version", "partition_summary"] {
        let mut under_resolved = evaluation_job(&json!({
            "kind": "independent",
            "candidates": ["cand_a"]
        }));
        under_resolved["resolved_set"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            matches!(
                package
                    .validate_evaluation_job_document(&under_resolved)
                    .unwrap_err(),
                PublicSeamError::InvalidEvaluationJob { .. }
            ),
            "missing {field} should fail"
        );
    }

    let same_candidate_pair = evaluation_job(&json!({
        "kind": "pairwise",
        "pairs": [{"left": "cand_a", "right": "cand_a"}]
    }));
    assert!(matches!(
        package
            .validate_evaluation_job_document(&same_candidate_pair)
            .unwrap_err(),
        PublicSeamError::InvalidEvaluationJob { .. }
    ));

    let same_candidate_pair_mixed_ref_shapes = evaluation_job(&json!({
        "kind": "pairwise",
        "pairs": [{
            "left": "cand_a",
            "right": {"kind": "candidate", "id": "cand_a"}
        }]
    }));
    assert!(matches!(
        package
            .validate_evaluation_job_document(&same_candidate_pair_mixed_ref_shapes)
            .unwrap_err(),
        PublicSeamError::InvalidEvaluationJob { .. }
    ));
}

#[test]
fn evaluation_request_receipt_binds_job_candidate_and_case_identity() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let job = package
        .validate_evaluation_job_document(&evaluation_job(&json!({
            "kind": "independent",
            "candidates": ["cand_a", "cand_b"]
        })))
        .unwrap();
    let result = evaluation_request_receipt_result(&job);

    assert!(matches!(
        package.validate_plan_result_document(&result).unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let receipt = package
        .validate_evaluation_request_receipt_document(&job, &result)
        .unwrap();

    assert_eq!(receipt.request_id(), job.request_id());
    assert_eq!(receipt.receipt_id(), "wrec_evalreq");
    assert_eq!(receipt.base_revision(), job.base_revision());
    assert_eq!(receipt.candidate_ids(), job.candidate_ids());
    assert_eq!(receipt.case_ids(), job.case_ids());
    assert!(receipt.request_hash().starts_with("fp_request_sha256_"));
    assert!(receipt.result_hash().starts_with("fp_result_sha256_"));
}

#[test]
fn evaluation_request_receipt_rejects_decorative_or_unbound_hashes() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let job = package
        .validate_evaluation_job_document(&evaluation_job(&json!({
            "kind": "independent",
            "candidates": ["cand_a", "cand_b"]
        })))
        .unwrap();

    let mut wrong_request_id = evaluation_request_receipt_result(&job);
    wrong_request_id["receipts"][0]["evaluation_request_id"] = json!("evalreq_other");
    assert!(matches!(
        package
            .validate_evaluation_request_receipt_document(&job, &wrong_request_id)
            .unwrap_err(),
        PublicSeamError::InvalidEvaluationJob { .. }
    ));

    let mut decorative_hash = evaluation_request_receipt_result(&job);
    decorative_hash["receipts"][0]["request_hash"] = json!("fp_request_sha256_decorative");
    assert!(matches!(
        package
            .validate_evaluation_request_receipt_document(&job, &decorative_hash)
            .unwrap_err(),
        PublicSeamError::InvalidEvaluationJob { .. }
    ));

    let mut decorative_result_hash = evaluation_request_receipt_result(&job);
    decorative_result_hash["receipts"][0]["result_hash"] = json!("fp_result_sha256_decorative");
    assert!(matches!(
        package
            .validate_evaluation_request_receipt_document(&job, &decorative_result_hash)
            .unwrap_err(),
        PublicSeamError::InvalidEvaluationJob { .. }
    ));

    let mut extra_decorative_receipt = evaluation_request_receipt_result(&job);
    extra_decorative_receipt["values"]["extra_evaluation_request"] = json!({
        "kind": "evaluation_request_receipt",
        "receipt": "wrec_extra_evalreq",
        "evaluation_request_id": "evalreq_extra",
        "status": "recorded",
        "graph_revision": job.base_revision(),
        "data_classes": ["public"],
        "replayability": "fully_managed"
    });
    extra_decorative_receipt["receipts"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "kind": "write",
            "write_kind": "request_evaluation",
            "receipt": "wrec_extra_evalreq",
            "started_at": "2026-05-23T12:00:00Z",
            "completed_at": "2026-05-23T12:00:01Z",
            "request_hash": "fp_request_sha256_decorative_extra",
            "result_hash": "fp_result_sha256_decorative_extra",
            "base_revision": job.base_revision(),
            "committed_revision": job.base_revision(),
            "status": "succeeded",
            "evaluation_request_id": "evalreq_extra"
        }));
    assert!(matches!(
        package
            .validate_evaluation_request_receipt_document(&job, &extra_decorative_receipt)
            .unwrap_err(),
        PublicSeamError::InvalidEvaluationJob { .. }
    ));

    let mut wrong_value_revision = evaluation_request_receipt_result(&job);
    wrong_value_revision["values"]["evaluation_request"]["graph_revision"] = json!("rev_other");
    assert!(matches!(
        package
            .validate_evaluation_request_receipt_document(&job, &wrong_value_revision)
            .unwrap_err(),
        PublicSeamError::InvalidEvaluationJob { .. }
    ));

    let mut missing_audit_timing = evaluation_request_receipt_result(&job);
    missing_audit_timing["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("started_at");
    assert!(matches!(
        package
            .validate_evaluation_request_receipt_document(&job, &missing_audit_timing)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

fn evaluation_job(kind: &Value) -> Value {
    json!({
        "schema_version": "leaven.evaluation_job.v1",
        "run": "run_eval",
        "stage_call_id": "sc_eval",
        "evaluation_attempt_id": "evalatt_score_output",
        "evaluation_request_id": "evalreq_score_output",
        "base_revision": "rev_base",
        "deadline_at": "2026-05-23T13:00:00Z",
        "kind": kind,
        "resolved_set": {
            "id": "rset_eval",
            "case_ids": ["case_1", "case_2"],
            "case_count": 2,
            "case_set_version": "cases:v1",
            "partition_summary": {
                "validation": 2
            }
        },
        "granularity": "per_case",
        "purpose": "validation",
        "capability_fingerprint": "fp_cap_sha256_evaluator",
        "evaluator_id": "primary",
        "evaluator_fingerprint": "fp_runtime_sha256_evaluator",
        "target_egress_policy_ref": "fp_policy_sha256_targets"
    })
}

fn evaluation_request_receipt_result(job: &leaven_public_seam::EvaluationJobDocument) -> Value {
    json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": "result_evalreq",
        "capability_fingerprint": job.capability_fingerprint(),
        "policy_fingerprint": "fp_policy_sha256_evalreq",
        "base_revision": job.base_revision(),
        "final_revision": job.base_revision(),
        "replayability_summary": "fully_managed",
        "values": {
            "evaluation_request": {
                "kind": "evaluation_request_receipt",
                "receipt": "wrec_evalreq",
                "evaluation_request_id": job.request_id(),
                "status": "recorded",
                "graph_revision": job.base_revision(),
                "data_classes": ["public"],
                "replayability": "fully_managed"
            }
        },
        "receipts": [
            {
                "kind": "write",
                "write_kind": "request_evaluation",
                "receipt": "wrec_evalreq",
                "started_at": "2026-05-23T12:00:00Z",
                "completed_at": "2026-05-23T12:00:01Z",
                "request_hash": evaluation_request_hash(job),
                "result_hash": evaluation_request_result_hash(job),
                "base_revision": job.base_revision(),
                "committed_revision": job.base_revision(),
                "status": "succeeded",
                "evaluation_request_id": job.request_id()
            }
        ],
        "redactions": [],
        "charges": [],
        "errors": []
    })
}

fn evaluation_request_hash(job: &leaven_public_seam::EvaluationJobDocument) -> String {
    fingerprint(
        "fp_request_sha256_",
        &json!({
            "schema_version": "leaven.evaluation_request_identity.v1",
            "evaluation_request_id": job.request_id(),
            "kind": evaluation_job_kind(job.kind()),
            "candidate_ids": job.candidate_ids(),
            "resolved_set_id": job.resolved_set_id(),
            "case_ids": job.case_ids(),
            "case_count": job.case_count(),
            "base_revision": job.base_revision(),
            "deadline_at": job.deadline_at(),
            "evaluator_id": job.evaluator_id(),
            "evaluator_fingerprint": job.evaluator_fingerprint(),
            "capability_fingerprint": job.capability_fingerprint()
        }),
    )
}

fn evaluation_request_result_hash(job: &leaven_public_seam::EvaluationJobDocument) -> String {
    fingerprint(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.evaluation_request_receipt_result.v1",
            "evaluation_request_id": job.request_id(),
            "status": "recorded",
            "resolved_set_id": job.resolved_set_id(),
            "case_ids": job.case_ids(),
            "candidate_ids": job.candidate_ids()
        }),
    )
}

fn fingerprint(prefix: &str, value: &Value) -> String {
    format!(
        "{prefix}{}",
        jcs_canonicalize::sha256_jcs_hex(value).unwrap()
    )
}

fn evaluation_job_kind(kind: leaven_public_seam::EvaluationJobKind) -> &'static str {
    match kind {
        leaven_public_seam::EvaluationJobKind::Independent => "independent",
        leaven_public_seam::EvaluationJobKind::Pairwise => "pairwise",
        leaven_public_seam::EvaluationJobKind::Listwise => "listwise",
    }
}
