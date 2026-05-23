use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use leaven_public_seam::{MatrixRowStatus, PublicSeamError, PublicSeamPackage, SchemaFingerprint};
use serde_json::{Value, json};

#[test]
fn active_package_loader_accepts_only_locked_public_seam_v1_package() {
    let root = workspace_root();
    let package = PublicSeamPackage::active_from_repo(&root).unwrap();

    assert_eq!(package.root(), root.join("docs/specs/public-seam-v1"));
    assert_eq!(package.manifest().name, "leaven-public-seam-v1");
    assert_eq!(package.manifest().status, "locked");
    assert_eq!(package.manifest().mcp_status, "not_in_v1");
    assert_eq!(package.manifest().watch_status, "deferred_to_v1.1");
    assert_eq!(
        package.manifest().worker_protocol_status,
        "deprecated_replaced_by_acp_profile"
    );

    let archived = root.join("docs/specs/public-seam-v1-lock-draft.archived");
    let error = PublicSeamPackage::from_path(archived).unwrap_err();
    assert!(matches!(error, PublicSeamError::InactivePackage { .. }));
}

#[test]
fn manifest_inventory_drives_contract_file_loading() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let inventory = package.inventory().unwrap();

    assert_eq!(
        inventory.schema_paths.len(),
        package.manifest().schemas.len()
    );
    assert!(inventory.goal_gate.ends_with("goal-readiness-gate.yaml"));
    assert!(inventory.matrix.ends_with("conformance-matrix.yaml"));
    assert!(
        inventory
            .profiles
            .iter()
            .any(|path| { path.ends_with("profiles/leaven_acp_profile_v1_v0.3.md") })
    );
    assert!(
        inventory
            .schemas_used_by_harness
            .contains("common.schema.json")
    );
    assert!(
        inventory
            .schemas_used_by_harness
            .contains("leaven.plan.v1.schema.json")
    );

    let missing = package
        .inventory_with_manifest_override(json!({
            "name": "leaven-public-seam-v1",
            "version": "1.0",
            "status": "locked",
            "goal_gate": "goal-readiness-gate.yaml",
            "conformance_matrix": "conformance-matrix.yaml",
            "schemas": ["missing.schema.json"],
            "profiles": [],
            "watch_status": "deferred_to_v1.1",
            "worker_protocol_status": "deprecated_replaced_by_acp_profile",
            "mcp_status": "not_in_v1",
            "key_decisions": [],
            "notes": []
        }))
        .unwrap_err();
    assert!(matches!(
        missing,
        PublicSeamError::MissingContractFile { .. }
    ));
}

#[test]
fn active_schemas_compile_and_examples_validate_against_manifest_targets() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let report = package.validate_contract_package().unwrap();

    assert_eq!(
        report.compiled_schemas.len(),
        package.manifest().schemas.len()
    );
    assert!(report.validated_examples.iter().any(|example| {
        example
            .example
            .ends_with("evaluator_capability.v0.3.example.json")
            && example.schema == "leaven.capability.v1.schema.json"
    }));
    assert!(report.validated_examples.iter().any(|example| {
        example
            .example
            .ends_with("reflect_then_propose.example.json")
            && example.schema == "leaven.stage_payloads.v1.schema.json"
            && example.pointer == "/reflection_result"
    }));

    let broken_schema = json!({ "type": "not-a-json-schema-type" });
    let error = package
        .compile_schema_value("broken.schema.json", &broken_schema)
        .unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidSchema { .. }));
}

#[test]
fn schema_fingerprints_use_jcs_sha256_not_pretty_printed_bytes() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let common = package.schema_json("common.schema.json").unwrap();
    let pretty_common: Value =
        serde_json::from_str(&serde_json::to_string_pretty(&common).unwrap()).unwrap();

    let first = SchemaFingerprint::for_json_value(&common).unwrap();
    let pretty = SchemaFingerprint::for_json_value(&pretty_common).unwrap();
    assert_eq!(first, pretty);
    assert!(first.as_str().starts_with("fp_schema_sha256_"));
    assert_eq!(
        first.as_str().trim_start_matches("fp_schema_sha256_").len(),
        64
    );

    let mut changed = common;
    changed["x-leaven-test-semantic-change"] = json!(true);
    let changed = SchemaFingerprint::for_json_value(&changed).unwrap();
    assert_ne!(first, changed);
}

#[test]
fn conformance_matrix_rows_are_unique_pending_and_reference_real_files() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let matrix = package.conformance_matrix().unwrap();

    assert_eq!(matrix.rows.len(), 39);
    assert!(
        matrix
            .rows
            .iter()
            .all(|row| row.status == MatrixRowStatus::Pending)
    );
    assert!(matrix.proven_rows().is_empty());

    let ids = matrix
        .rows
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), matrix.rows.len());
    assert!(ids.contains("ps1.authority.active_package_only"));
    assert!(ids.contains("ps1.harness.negative_denominator"));

    package.validate_matrix_references(&matrix).unwrap();
}

#[test]
fn v1_scope_markers_refuse_mcp_watch_runtime_and_legacy_worker_protocol() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let scope = package.v1_scope().unwrap();

    assert!(!scope.mcp_over_acp_enabled);
    assert!(!scope.watch_runtime_enabled);
    assert!(!scope.legacy_worker_protocol_enabled);
    assert_eq!(scope.worker_transport, "acp_profile");
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}
