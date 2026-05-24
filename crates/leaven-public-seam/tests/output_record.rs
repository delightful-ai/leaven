use leaven_evidence::{
    DataClass, DataClassSet, OutputBlobAudit, OutputMetadata, OutputRecord, OutputVisibility,
};
use leaven_kernel::BlobRef;
use leaven_public_seam::{PublicBlobRef, PublicSeamError, PublicSeamPackage};
use serde_json::json;

#[test]
fn output_record_lowers_inline_visibility_and_data_classes_to_active_schema() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let output = OutputRecord::inline("answer 42").with_metadata(OutputMetadata::new(
        OutputVisibility::ReflectorVisible,
        DataClassSet::new([DataClass::candidate_output(), DataClass::public()]),
    ));

    let public = package
        .project_output_record(&output, None)
        .expect("inline output carries enough public seam facts");

    assert_eq!(
        public.as_value(),
        &json!({
            "kind": "text",
            "summary": "answer 42",
            "value": "answer 42",
            "visibility": "reflector_visible",
            "data_classes": ["candidate.output", "public"]
        })
    );
}

#[test]
fn output_record_rejects_placeholder_or_under_described_blob_outputs() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let placeholder = OutputRecord::inline(" \n\t");
    assert!(matches!(
        package
            .project_output_record(&placeholder, None)
            .unwrap_err(),
        PublicSeamError::InvalidOutputRecord { .. }
    ));

    let blob = OutputRecord::blob(BlobRef {
        store: "file".to_owned(),
        key: "answers/42.txt".to_owned(),
    });
    assert!(matches!(
        package.project_output_record(&blob, None).unwrap_err(),
        PublicSeamError::InvalidOutputRecord { .. }
    ));

    let public_blob = PublicBlobRef::new(
        "blob_answer_42",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        42,
        DataClassSet::public(),
    )
    .with_media_type("text/plain")
    .with_uri("leaven-blob://answers/42.txt");
    let projected = package
        .project_output_record(&blob, Some(&public_blob))
        .expect("blob output needs public blob identity and audit facts");
    assert_eq!(projected.kind(), "blob_ref");

    let invalid_public_blob =
        PublicBlobRef::new("blob_answer_42", "not-a-sha256", 42, DataClassSet::public());
    assert!(matches!(
        package
            .project_output_record(&blob, Some(&invalid_public_blob))
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn output_record_projects_audited_blob_without_external_metadata() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let blob = OutputRecord::audited_blob(
        BlobRef {
            store: "file".to_owned(),
            key: "answers/42.txt".to_owned(),
        },
        OutputBlobAudit::new(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            42,
        )
        .unwrap()
        .with_media_type("text/plain")
        .with_uri("leaven-blob://answers/42.txt"),
    )
    .with_metadata(OutputMetadata::new(
        OutputVisibility::Public,
        DataClassSet::new([DataClass::candidate_output(), DataClass::public()]),
    ));

    let projected = package
        .project_output_record(&blob, None)
        .expect("audited blob output carries enough public seam facts");

    assert_eq!(
        projected.as_value(),
        &json!({
            "kind": "blob_ref",
            "blob_ref": {
                "kind": "blob_ref",
                "id": "blob_cd28ffaa6d6a549defa9f69964204e156c1f8ddfc65f39729643df3b6557e54d",
                "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "bytes": 42,
                "media_type": "text/plain",
                "uri": "leaven-blob://answers/42.txt",
                "data_classes": ["candidate.output", "public"]
            },
            "visibility": "public",
            "data_classes": ["candidate.output", "public"]
        })
    );
}

#[test]
fn output_record_value_validation_rejects_schema_only_shortcuts() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    assert!(matches!(
        package
            .validate_output_record_value(&json!({
                "kind": "text",
                "summary": "missing visibility and data classes"
            }))
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}
