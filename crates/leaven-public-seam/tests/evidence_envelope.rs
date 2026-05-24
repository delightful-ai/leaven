use leaven_public_seam::{PublicSeamError, PublicSeamPackage};
use serde_json::{Value, json};

#[test]
fn evidence_envelope_preserves_visibility_data_classes_and_receipts() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let envelope = package
        .validate_evidence_envelope_document(&target_derived_envelope())
        .unwrap();

    assert!(envelope.is_target_derived());
    assert_eq!(
        envelope.data_classes(),
        &["case.target".to_owned(), "scorer.private".to_owned()]
    );
    assert_eq!(envelope.public_data_classes(), &["case.target".to_owned()]);
    assert_eq!(
        envelope.private_data_classes(),
        Some(&["scorer.private".to_owned()][..])
    );
    assert_eq!(envelope.read_receipts(), &["qrec_target".to_owned()]);
    assert_eq!(envelope.effect_receipts(), &["lmrec_score".to_owned()]);
    assert_eq!(envelope.write_receipts(), &["wrec_assessment".to_owned()]);
}

#[test]
fn evidence_envelope_rejects_target_derived_data_class_gaps() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut missing_top_level = target_derived_envelope();
    missing_top_level
        .as_object_mut()
        .unwrap()
        .remove("data_classes");
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&missing_top_level)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut missing_private_projection = target_derived_envelope();
    missing_private_projection["data_classes"] = json!(["case.target"]);
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&missing_private_projection)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));

    let mut missing_target_class = target_derived_envelope();
    missing_target_class["data_classes"] = json!(["scorer.private"]);
    missing_target_class["public"]["data_classes"] = json!(["scorer.private"]);
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&missing_target_class)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));
}

#[test]
fn evidence_envelope_rejects_unreceipted_target_derived_evidence() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut missing_target_read = target_derived_envelope();
    missing_target_read["source_receipts"]["read"] = json!([]);
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&missing_target_read)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));

    let mut no_receipt_sources = target_derived_envelope();
    no_receipt_sources["source_receipts"]["read"] = json!([]);
    no_receipt_sources["source_receipts"]["effect"] = json!([]);
    no_receipt_sources["source_receipts"]["write"] = json!([]);
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&no_receipt_sources)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));
}

fn target_derived_envelope() -> Value {
    json!({
        "schema_version": "leaven.evidence_envelope.v1",
        "target_derived": true,
        "data_classes": ["case.target", "scorer.private"],
        "public": {
            "feedback": "matched expected answer",
            "data_classes": ["case.target"]
        },
        "private": {
            "visibility": "scorer_private",
            "payload": {
                "rubric": "target-derived private notes"
            },
            "data_classes": ["scorer.private"]
        },
        "redaction_policy": {
            "optimizer": "score_only",
            "reflector": "score_and_feedback",
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
