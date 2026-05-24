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
        &[
            "case.target".to_owned(),
            "prompt.raw".to_owned(),
            "scorer.private".to_owned(),
            "transcript.raw".to_owned()
        ]
    );
    assert_eq!(envelope.public_data_classes(), &["case.target".to_owned()]);
    assert_eq!(
        envelope.private_data_classes(),
        Some(&["scorer.private".to_owned()][..])
    );
    assert_eq!(
        envelope.trace_data_classes(),
        &["prompt.raw".to_owned(), "transcript.raw".to_owned()]
    );
    assert_eq!(envelope.read_receipts(), &["qrec_target".to_owned()]);
    assert_eq!(envelope.effect_receipts(), &["lmrec_score".to_owned()]);
    assert_eq!(envelope.write_receipts(), &["wrec_assessment".to_owned()]);
    assert_eq!(envelope.trace_receipts(), &["lmrec_score".to_owned()]);
}

#[test]
fn evidence_envelope_accepts_object_receipt_refs_and_binds_trace_receipts_to_sources() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut envelope = target_derived_envelope();
    envelope["source_receipts"]["read"] = json!([receipt_ref("qrec_target")]);
    envelope["source_receipts"]["effect"] = json!([receipt_ref("lmrec_score")]);
    envelope["source_receipts"]["write"] = json!([receipt_ref("wrec_assessment")]);
    envelope["public"]["trace_refs"][0]["receipt"] = receipt_ref("lmrec_score");
    envelope["trace_refs"][0]["receipt"] = receipt_ref("lmrec_score");

    let envelope = package
        .validate_evidence_envelope_document(&envelope)
        .unwrap();

    assert_eq!(envelope.read_receipts(), &["qrec_target".to_owned()]);
    assert_eq!(envelope.effect_receipts(), &["lmrec_score".to_owned()]);
    assert_eq!(envelope.write_receipts(), &["wrec_assessment".to_owned()]);
    assert_eq!(envelope.trace_receipts(), &["lmrec_score".to_owned()]);
}

#[test]
fn evidence_envelope_accepts_schema_valid_trace_refs_without_receipts() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut envelope = target_derived_envelope();
    envelope["public"]["trace_refs"][0]
        .as_object_mut()
        .unwrap()
        .remove("receipt");
    envelope["trace_refs"][0]
        .as_object_mut()
        .unwrap()
        .remove("receipt");

    let envelope = package
        .validate_evidence_envelope_document(&envelope)
        .unwrap();

    assert!(envelope.trace_receipts().is_empty());
    assert_eq!(
        envelope.trace_data_classes(),
        &["prompt.raw".to_owned(), "transcript.raw".to_owned()]
    );
}

#[test]
fn evidence_envelope_rejects_source_receipt_family_mismatches() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    for (field, value, expected) in [
        (
            "read",
            json!(["lmrec_score"]),
            "source_receipts.read must contain read receipt refs",
        ),
        (
            "effect",
            json!(["qrec_target"]),
            "source_receipts.effect must contain effect receipt refs",
        ),
        (
            "write",
            json!(["qrec_target"]),
            "source_receipts.write must contain write receipt refs",
        ),
    ] {
        let mut envelope = target_derived_envelope();
        envelope["source_receipts"][field] = value;
        let error = package
            .validate_evidence_envelope_document(&envelope)
            .unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected `{expected}` in {error:?}"
        );
    }

    let mut object_form_wrong_effect = target_derived_envelope();
    object_form_wrong_effect["source_receipts"]["effect"] = json!([receipt_ref("qrec_target")]);
    let error = package
        .validate_evidence_envelope_document(&object_form_wrong_effect)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("source_receipts.effect must contain effect receipt refs"),
        "unexpected error: {error:?}"
    );
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

    let mut missing_trace_projection = target_derived_envelope();
    missing_trace_projection["data_classes"] = json!(["case.target", "scorer.private"]);
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&missing_trace_projection)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));

    let mut missing_top_level_trace_projection = target_derived_envelope();
    missing_top_level_trace_projection["data_classes"] =
        json!(["case.target", "scorer.private", "transcript.raw"]);
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&missing_top_level_trace_projection)
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

    let mut missing_private_payload_ref_projection = target_derived_envelope();
    missing_private_payload_ref_projection["private"]["payload_ref"] = json!({
        "kind": "blob_ref",
        "id": "blob_private_payload",
        "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "bytes": 32,
        "data_classes": ["external.secret"]
    });
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&missing_private_payload_ref_projection)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));

    let mut missing_top_level_private_payload_ref_projection = target_derived_envelope();
    missing_top_level_private_payload_ref_projection["private"]["data_classes"] =
        json!(["scorer.private", "external.secret"]);
    missing_top_level_private_payload_ref_projection["private"]["payload_ref"] = json!({
        "kind": "blob_ref",
        "id": "blob_private_payload",
        "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "bytes": 32,
        "data_classes": ["external.secret"]
    });
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&missing_top_level_private_payload_ref_projection)
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

    let mut undeclared_trace_receipt = target_derived_envelope();
    undeclared_trace_receipt["source_receipts"]["effect"] = json!([]);
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&undeclared_trace_receipt)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));
}

#[test]
fn evidence_envelope_rejects_hidden_target_derivation_flag() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut top_level_target_class = target_derived_envelope();
    top_level_target_class["target_derived"] = json!(false);
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&top_level_target_class)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));

    let mut public_projection_target_class = target_derived_envelope();
    public_projection_target_class["target_derived"] = json!(false);
    public_projection_target_class
        .as_object_mut()
        .unwrap()
        .remove("data_classes");
    public_projection_target_class["public"]["data_classes"] = json!(["case.target"]);
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&public_projection_target_class)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));

    let mut private_projection_target_class = target_derived_envelope();
    private_projection_target_class["target_derived"] = json!(false);
    private_projection_target_class
        .as_object_mut()
        .unwrap()
        .remove("data_classes");
    private_projection_target_class["public"]["data_classes"] = json!(["public"]);
    private_projection_target_class["private"]["data_classes"] = json!(["case.target"]);
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&private_projection_target_class)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));

    let mut public_trace_target_class = target_derived_envelope();
    public_trace_target_class["target_derived"] = json!(false);
    public_trace_target_class
        .as_object_mut()
        .unwrap()
        .remove("data_classes");
    public_trace_target_class["public"]["data_classes"] = json!(["public"]);
    public_trace_target_class["public"]["trace_refs"][0]["data_classes"] = json!(["case.target"]);
    public_trace_target_class
        .as_object_mut()
        .unwrap()
        .remove("trace_refs");
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&public_trace_target_class)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));

    let mut top_level_trace_target_class = target_derived_envelope();
    top_level_trace_target_class["target_derived"] = json!(false);
    top_level_trace_target_class
        .as_object_mut()
        .unwrap()
        .remove("data_classes");
    top_level_trace_target_class["public"]["data_classes"] = json!(["public"]);
    top_level_trace_target_class["trace_refs"][0]["data_classes"] = json!(["case.target"]);
    assert!(matches!(
        package
            .validate_evidence_envelope_document(&top_level_trace_target_class)
            .unwrap_err(),
        PublicSeamError::InvalidEvidence { .. }
    ));
}

fn target_derived_envelope() -> Value {
    json!({
        "schema_version": "leaven.evidence_envelope.v1",
        "target_derived": true,
        "data_classes": ["case.target", "prompt.raw", "scorer.private", "transcript.raw"],
        "public": {
            "feedback": "matched expected answer",
            "data_classes": ["case.target"],
            "trace_refs": [
                {
                    "kind": "judge_trace",
                    "id": "trace_score",
                    "visibility": "redacted_transcript",
                    "data_classes": ["transcript.raw"],
                    "receipt": "lmrec_score"
                }
            ]
        },
        "private": {
            "visibility": "scorer_private",
            "payload": {
                "rubric": "target-derived private notes"
            },
            "data_classes": ["scorer.private"]
        },
        "trace_refs": [
            {
                "kind": "prompt_trace",
                "id": "trace_prompt",
                "visibility": "redacted_prompt",
                "data_classes": ["prompt.raw"],
                "receipt": "lmrec_score"
            }
        ],
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

fn receipt_ref(id: &str) -> Value {
    json!({
        "kind": "receipt",
        "id": id,
        "fingerprint": "fp_receipt_sha256_evidence"
    })
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}
