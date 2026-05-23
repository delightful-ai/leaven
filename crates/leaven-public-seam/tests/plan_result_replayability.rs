use leaven_public_seam::{PublicSeamError, PublicSeamPackage, Replayability};
use serde_json::{Value, json};

#[test]
fn per_assessment_replayability_rolls_up_from_each_assessment() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let result = package
        .validate_plan_result_document(&mixed_replayability_result(
            "has_declared_external_effects",
            [
                ("assess_replay_1", "pure_read"),
                ("assess_replay_2", "has_declared_external_effects"),
            ],
        ))
        .unwrap();

    assert_eq!(
        result.assessment_batch_replayability(),
        &[
            ("assess_replay_1".to_owned(), Replayability::PureRead),
            (
                "assess_replay_2".to_owned(),
                Replayability::HasDeclaredExternalEffects,
            ),
        ]
    );
    assert_eq!(
        result.replayability_summary(),
        Replayability::HasDeclaredExternalEffects
    );
}

#[test]
fn replayability_summary_cannot_hide_non_replayable_assessment() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    assert!(matches!(
        package
            .validate_plan_result_document(&mixed_replayability_result(
                "pure_read",
                [
                    ("assess_replay_1", "pure_read"),
                    ("assess_replay_2", "has_untracked_external_effects"),
                ],
            ))
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut missing_per_assessment =
        mixed_replayability_result("pure_read", [("assess_replay_1", "pure_read")]);
    missing_per_assessment["values"]["assessment_batch"]
        .as_object_mut()
        .unwrap()
        .remove("per_assessment");
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_per_assessment)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn assessment_write_receipts_require_per_assessment_result_facts() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut result = mixed_replayability_result(
        "pure_read",
        [
            ("assess_replay_1", "pure_read"),
            ("assess_replay_2", "pure_read"),
        ],
    );
    result["values"] = json!({});

    assert!(matches!(
        package.validate_plan_result_document(&result).unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn replayability_summary_rolls_up_all_result_values() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut result = mixed_replayability_result("pure_read", [("assess_replay_1", "pure_read")]);
    result["values"]["external_value"] = json!({
        "kind": "graph_set",
        "items": [],
        "graph_revision": "rev_replayability_final",
        "data_classes": ["public"],
        "replayability": "has_untracked_external_effects"
    });

    assert!(matches!(
        package.validate_plan_result_document(&result).unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

fn mixed_replayability_result<const N: usize>(
    summary: &str,
    per_assessment: [(&str, &str); N],
) -> Value {
    let assessment_ids = per_assessment
        .iter()
        .map(|(assessment, _)| json!(*assessment))
        .collect::<Vec<_>>();
    let per_assessment = per_assessment
        .iter()
        .map(|(assessment, replayability)| {
            json!({
                "assessment": *assessment,
                "replayability": *replayability,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": "planresult_replayability",
        "capability_fingerprint": "fp_cap_sha256_replayability",
        "policy_fingerprint": "fp_policy_sha256_replayability",
        "base_revision": "rev_replayability_base",
        "final_revision": "rev_replayability_final",
        "replayability_summary": summary,
        "values": {
            "assessment_batch": {
                "kind": "assessment_batch_receipt",
                "assessment_ids": assessment_ids,
                "evaluation_request_id": "evalreq_replayability",
                "per_assessment": per_assessment,
                "status": "committed",
                "graph_revision": "rev_replayability_final",
                "data_classes": ["public"],
                "replayability": summary
            }
        },
        "receipts": [
            {
                "kind": "write",
                "receipt": "wrec_replayability",
                "started_at": "2026-05-23T12:00:00Z",
                "completed_at": "2026-05-23T12:00:01Z",
                "write_kind": "submit_assessments",
                "request_hash": "fp_req_sha256_replayability",
                "result_hash": "fp_result_sha256_replayability",
                "base_revision": "rev_replayability_base",
                "committed_revision": "rev_replayability_final",
                "status": "succeeded",
                "evaluation_request_id": "evalreq_replayability",
                "assessment_ids": assessment_ids
            }
        ],
        "redactions": [],
        "charges": [],
        "errors": []
    })
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}
