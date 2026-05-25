use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use leaven_public_seam::{
    MatrixRowStatus, PublicSeamError, PublicSeamPackage, SchemaFingerprint, WorkerTransportKind,
    WorkerTransportRequest,
};
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

    let copied_repo = tempfile::tempdir().unwrap();
    let copied_package = copied_repo.path().join("docs/specs/public-seam-v1");
    std::fs::create_dir_all(copied_package.parent().unwrap()).unwrap();
    copy_dir_all(root.join("docs/specs/public-seam-v1"), &copied_package).unwrap();
    let error = PublicSeamPackage::from_path(copied_package).unwrap_err();
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
fn acp_profile_routes_callbacks_without_mcp_negotiation() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let methods = package.acp_extension_methods().unwrap();
    let scope = package.v1_scope().unwrap();

    assert!(methods.contains(&"leaven/graph.query".to_owned()));
    assert!(methods.contains(&"leaven/lm.complete".to_owned()));
    assert!(methods.contains(&"leaven/proposal.submit_batch".to_owned()));
    assert!(methods.iter().all(|method| method.starts_with("leaven/")));
    assert!(methods.iter().all(|method| !method.contains("mcp")));

    let authorized = scope
        .authorize_worker_transport(WorkerTransportRequest::acp_profile(methods))
        .unwrap();
    assert_eq!(authorized.worker_transport(), "acp_profile");
    assert!(
        authorized
            .extension_methods()
            .contains(&"leaven/lm.complete")
    );
}

#[test]
fn acp_profile_rejects_mcp_bridge_legacy_worker_protocol_and_watch_runtime() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let scope = package.v1_scope().unwrap();

    assert!(matches!(
        scope
            .authorize_worker_transport(WorkerTransportRequest::new(
                WorkerTransportKind::McpOverAcp,
                ["mcp/tools/list"]
            ))
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    assert!(matches!(
        scope
            .authorize_worker_transport(WorkerTransportRequest::new(
                WorkerTransportKind::LegacyWorkerProtocol,
                ["leaven/lm.complete"]
            ))
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut mcp_method =
        WorkerTransportRequest::new(WorkerTransportKind::AcpProfile, ["leaven/lm.complete"]);
    mcp_method.add_extension_method("mcp/tools/call");
    assert!(matches!(
        scope.authorize_worker_transport(mcp_method).unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    assert!(matches!(
        scope
            .authorize_worker_transport(WorkerTransportRequest::new(
                WorkerTransportKind::AcpProfile,
                ["leaven/tools.list"]
            ))
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    assert!(matches!(
        scope
            .authorize_worker_transport(WorkerTransportRequest::new(
                WorkerTransportKind::AcpProfile,
                [] as [&str; 0]
            ))
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    assert!(matches!(
        scope
            .authorize_worker_transport(WorkerTransportRequest::new(
                WorkerTransportKind::AcpProfile,
                ["private/lm.complete"]
            ))
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    assert!(matches!(
        scope
            .authorize_worker_transport(WorkerTransportRequest::new(
                WorkerTransportKind::AcpProfile,
                ["leaven/not-mcp-but-mentions-mcp"]
            ))
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut watch_runtime =
        WorkerTransportRequest::new(WorkerTransportKind::AcpProfile, ["leaven/lm.complete"]);
    watch_runtime.enable_watch_runtime();
    assert!(matches!(
        scope.authorize_worker_transport(watch_runtime).unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let archived = workspace_root().join("docs/specs/public-seam-v1-lock-draft.archived");
    let error = PublicSeamPackage::from_path(archived).unwrap_err();
    assert!(matches!(error, PublicSeamError::InactivePackage { .. }));
}

#[test]
fn deprecated_worker_protocol_marker_routes_to_acp_profile() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let marker = json!({
        "schema_version": "leaven.worker_protocol.v1.deprecated",
        "replacement": "leaven.acp_profile.v1"
    });

    package
        .compile_schema_value(
            "leaven.worker_protocol.v1.schema.json",
            &package
                .schema_json("leaven.worker_protocol.v1.schema.json")
                .unwrap(),
        )
        .unwrap();
    package
        .validate_arbitrary_value("leaven.worker_protocol.v1.schema.json", "/marker", &marker)
        .unwrap();

    let scope = package.v1_scope().unwrap();
    let authorized = scope
        .authorize_worker_transport(WorkerTransportRequest::acp_profile(
            package.acp_extension_methods().unwrap(),
        ))
        .unwrap();
    assert_eq!(authorized.worker_transport(), "acp_profile");
}

#[test]
fn deprecated_worker_protocol_rejects_runtime_protocol_and_revival_claims() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let scope = package.v1_scope().unwrap();

    assert!(
        package
            .acp_extension_methods()
            .unwrap()
            .iter()
            .all(|method| !method.contains("worker_protocol"))
    );

    assert!(matches!(
        scope
            .authorize_worker_transport(WorkerTransportRequest::new(
                WorkerTransportKind::LegacyWorkerProtocol,
                ["leaven/worker_protocol.run"]
            ))
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    assert!(matches!(
        scope
            .authorize_worker_transport(WorkerTransportRequest::new(
                WorkerTransportKind::AcpProfile,
                ["leaven/worker_protocol.run"]
            ))
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let revived_marker = json!({
        "schema_version": "leaven.worker_protocol.v1",
        "replacement": "leaven.worker_protocol.v1"
    });
    assert!(matches!(
        package
            .validate_arbitrary_value(
                "leaven.worker_protocol.v1.schema.json",
                "/revived",
                &revived_marker
            )
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn deferred_watch_marker_routes_to_since_revision_plan_diff() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let marker = json!({
        "schema_version": "leaven.watch.v1.deferred",
        "use_instead": "leaven.plan.v1 consistency.since_revision"
    });

    package
        .validate_arbitrary_value("leaven.watch.v1.schema.json", "/marker", &marker)
        .unwrap();

    let finite_diff_plan = json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "watchdiff001",
        "consistency": {
            "kind": "since_revision",
            "since": "rev_base",
            "until": "rev_tip"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "let",
                "name": "events",
                "expr": {
                    "kind": "graph_query",
                    "source": {
                        "kind": "events",
                        "since_revision": "rev_base",
                        "until_revision": "rev_tip"
                    },
                    "projection": {
                        "kind": "ids"
                    },
                    "page": {
                        "limit": 100
                    }
                }
            }
        ],
        "return": ["events"],
        "commit": {
            "kind": "no_graph_writes"
        }
    });
    package
        .validate_arbitrary_value(
            "leaven.plan.v1.schema.json",
            "/finite_diff_plan",
            &finite_diff_plan,
        )
        .unwrap();
    let replacement = package
        .validate_deferred_watch_replacement(&marker, &finite_diff_plan)
        .unwrap();
    assert!(replacement.plan().is_since_revision_event_diff());
    assert_eq!(replacement.plan().consistency_kind(), "since_revision");
    assert_eq!(replacement.plan().since_revision(), Some("rev_base"));
    assert_eq!(replacement.plan().until_revision(), Some("rev_tip"));
    assert_eq!(replacement.plan().events_since_revision_queries(), 1);
}

#[test]
fn deferred_watch_rejects_schema_valid_non_diff_replacements() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let marker = json!({
        "schema_version": "leaven.watch.v1.deferred",
        "use_instead": "leaven.plan.v1 consistency.since_revision"
    });

    let mut latest_at_start = finite_diff_plan();
    latest_at_start["consistency"] = json!({
        "kind": "latest_at_start"
    });
    assert!(matches!(
        package
            .validate_deferred_watch_replacement(&marker, &latest_at_start)
            .unwrap_err(),
        PublicSeamError::InvalidWatch { .. }
    ));

    let mut no_event_diff = finite_diff_plan();
    no_event_diff["ops"][0]["expr"]["source"] = json!({
        "kind": "candidate_set"
    });
    assert!(matches!(
        package
            .validate_deferred_watch_replacement(&marker, &no_event_diff)
            .unwrap_err(),
        PublicSeamError::InvalidWatch { .. }
    ));

    let mut at_revision = finite_diff_plan();
    at_revision["consistency"] = json!({
        "kind": "at_revision",
        "revision": "rev_base"
    });
    assert!(matches!(
        package
            .validate_deferred_watch_replacement(&marker, &at_revision)
            .unwrap_err(),
        PublicSeamError::InvalidWatch { .. }
    ));

    let mut mismatched_base = finite_diff_plan();
    mismatched_base["ops"][0]["expr"]["source"]["since_revision"] = json!("rev_other");
    assert!(matches!(
        package
            .validate_deferred_watch_replacement(&marker, &mismatched_base)
            .unwrap_err(),
        PublicSeamError::InvalidWatch { .. }
    ));

    let mut missing_event_diff = finite_diff_plan();
    missing_event_diff["ops"][0]["expr"] = json!({
        "kind": "literal",
        "value": [],
        "data_classes": ["public"]
    });
    assert!(matches!(
        package
            .validate_deferred_watch_replacement(&marker, &missing_event_diff)
            .unwrap_err(),
        PublicSeamError::InvalidWatch { .. }
    ));
}

#[test]
fn deferred_watch_rejects_runtime_subscription_and_success_claims() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let scope = package.v1_scope().unwrap();

    assert!(
        package
            .acp_extension_methods()
            .unwrap()
            .iter()
            .all(|method| !method.contains("watch"))
    );

    let watch_runtime_request = json!({
        "schema_version": "leaven.watch.v1",
        "watch_id": "watch_runtime",
        "source": {
            "kind": "events",
            "since_revision": "rev_base"
        }
    });
    assert!(matches!(
        package
            .validate_arbitrary_value(
                "leaven.watch.v1.schema.json",
                "/watch_runtime_request",
                &watch_runtime_request
            )
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let watch_success_claim = json!({
        "schema_version": "leaven.watch.v1.deferred",
        "use_instead": "leaven.plan.v1 consistency.since_revision",
        "status": "supported"
    });
    assert!(matches!(
        package
            .validate_arbitrary_value(
                "leaven.watch.v1.schema.json",
                "/watch_success_claim",
                &watch_success_claim
            )
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    for method in [
        "leaven/watch.start",
        "leaven/watch.subscribe",
        "leaven/watch.stream",
        "leaven/watch.next",
        "leaven/watch.ack",
        "leaven/watch.close",
    ] {
        assert!(matches!(
            scope
                .authorize_worker_transport(WorkerTransportRequest::new(
                    WorkerTransportKind::AcpProfile,
                    [method]
                ))
                .unwrap_err(),
            PublicSeamError::InvalidScope { .. }
        ));
    }

    let mut watch_runtime =
        WorkerTransportRequest::new(WorkerTransportKind::AcpProfile, ["leaven/graph.query"]);
    watch_runtime.enable_watch_runtime();
    assert!(matches!(
        scope.authorize_worker_transport(watch_runtime).unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));
}

#[test]
fn schema_fingerprints_reject_pretty_printed_hashing_and_track_semantic_changes() {
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
fn conformance_matrix_rows_are_unique_honest_and_reference_real_files() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let matrix = package.conformance_matrix().unwrap();

    assert_eq!(matrix.rows.len(), 39);
    let proven = matrix
        .proven_rows()
        .into_iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        proven,
        BTreeSet::from([
            "ps1.authority.active_package_only",
            "ps1.authority.manifest_inventory",
            "ps1.agent.contract",
            "ps1.capability.aggregate_budgets",
            "ps1.capability.document_truth",
            "ps1.capability.delegation_attenuates",
            "ps1.capability.grant_enforcement",
            "ps1.acp.auth_permissions",
            "ps1.acp.extension_results",
            "ps1.acp.lifecycle_backpressure",
            "ps1.acp.no_mcp_v1",
            "ps1.acp.transport_profile",
            "ps1.evaluator.assessment_scope",
            "ps1.evaluator.job_identity",
            "ps1.evaluator.score_output",
            "ps1.evaluator.target_reads",
            "ps1.evidence.visibility_receipts",
            "ps1.graph.runcontext_mutation_only",
            "ps1.harness.negative_denominator",
            "ps1.lm.contract",
            "ps1.plan.ir_family",
            "ps1.plan.execution_modes",
            "ps1.plan.pinned_dialects",
            "ps1.plan.revision_modes",
            "ps1.proposal.surface_authority",
            "ps1.public_routes.maturity_classified",
            "ps1.result.typed_envelope",
            "ps1.receipts.audit_currency",
            "ps1.receipts.failed_costs",
            "ps1.replay.per_assessment",
            "ps1.schema.fingerprints",
            "ps1.sandbox.exec_streaming",
            "ps1.stage.reflection_proposal_split",
            "ps1.stage.payload_receipts",
            "ps1.visibility.data_class_propagation",
            "ps1.visibility.reflector_target_safe",
            "ps1.watch.deferred",
            "ps1.worker_protocol.deprecated",
            "ps1.workspace.handles_lifecycle"
        ])
    );

    let ids = matrix
        .rows
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), matrix.rows.len());
    assert!(ids.contains("ps1.authority.active_package_only"));
    assert!(ids.contains("ps1.harness.negative_denominator"));
    assert!(
        matrix
            .rows
            .iter()
            .filter(|row| row.status == MatrixRowStatus::Proven)
            .all(|row| !row.implementation_evidence.is_empty() && !row.review_evidence.is_empty())
    );

    package.validate_matrix_references(&matrix).unwrap();
    package.audit_conformance_evidence(&matrix).unwrap();
}

#[test]
fn public_seam_routes_reject_ordinary_facade_leaks() {
    let root = workspace_root();
    let package = PublicSeamPackage::active_from_repo(root.clone()).unwrap();
    let matrix = package.conformance_matrix().unwrap();
    let route_row = matrix
        .rows
        .iter()
        .find(|row| row.id == "ps1.public_routes.maturity_classified")
        .unwrap();
    assert_eq!(route_row.status, MatrixRowStatus::Proven);

    let public_seam_agents = fs::read_to_string(root.join("crates/leaven-public-seam/AGENTS.md"))
        .expect("public seam AGENTS.md must classify public exports");
    for phrase in [
        "advanced public contract",
        "CapabilityDocument",
        "CapabilityRegistry",
        "ConformanceRow",
        "ConformanceTest",
        "not routed through `leaven::prelude`",
    ] {
        assert!(
            public_seam_agents.contains(phrase),
            "public seam AGENTS.md must contain route maturity phrase `{phrase}`"
        );
    }

    let umbrella_manifest = fs::read_to_string(root.join("crates/leaven/Cargo.toml")).unwrap();
    assert!(
        !umbrella_manifest.contains("leaven-public-seam"),
        "public seam crate must not enter the ordinary umbrella dependency set"
    );
    for route in ["lib.rs", "prelude.rs", "extend.rs", "plumbing.rs"] {
        let source = fs::read_to_string(root.join("crates/leaven/src").join(route)).unwrap();
        for forbidden in [
            "leaven_public_seam",
            "public_seam",
            "CapabilityDocument",
            "CapabilityRegistry",
            "ConformanceRow",
            "ConformanceTest",
        ] {
            assert!(
                !source.contains(forbidden),
                "`{route}` must not expose immature public-seam name `{forbidden}`"
            );
        }
    }
}

#[test]
fn conformance_evidence_audit_rejects_fake_closeout_for_denial_rows() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.schema.fingerprints")
        .unwrap();

    row.implementation_evidence = vec![
        "docs/specs/public-seam-v1/schemas/common.schema.json".to_owned(),
        "crates/leaven/tests/topology_contract.rs::leaven_dependency_edges_match_corrected_topology"
            .to_owned(),
    ];
    row.positive_test_evidence.clear();
    row.negative_test_evidence.clear();

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.schema.fingerprints"));
    assert!(
        error
            .to_string()
            .contains("schema/example/topology/matrix proof")
    );
}

#[test]
fn conformance_matrix_reference_check_rejects_stale_pending_test_symbols() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.acp.transport_profile")
        .unwrap();
    row.status = MatrixRowStatus::Pending;
    row.blocked_on.clear();
    assert_eq!(row.status, MatrixRowStatus::Pending);
    row.positive_test_evidence = vec![
        "crates/leaven-public-seam/tests/public_seam_contract/stage_payloads.rs::stage_payloads_preserve_object_form_info_refs"
            .to_owned(),
    ];

    let error = package.validate_matrix_references(&matrix).unwrap_err();

    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.acp.transport_profile"));
    assert!(
        error
            .to_string()
            .contains("stage_payloads_preserve_object_form_info_refs")
    );
}

#[test]
fn conformance_evidence_audit_rejects_pending_rows_with_closeout_evidence_fields() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.acp.transport_profile")
        .unwrap();
    row.status = MatrixRowStatus::Pending;
    row.blocked_on.clear();
    assert_eq!(row.status, MatrixRowStatus::Pending);
    row.positive_test_evidence = vec![
        "crates/leaven-public-seam/tests/public_seam_contract/stage_payloads.rs::stage_payloads_validate_all_role_specific_payload_shapes_with_provenance"
            .to_owned(),
    ];

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();

    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.acp.transport_profile"));
    assert!(error.to_string().contains("partial_contract evidence"));
}

#[test]
fn conformance_evidence_audit_rejects_blocked_rows_without_named_prerequisites() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.acp.transport_profile")
        .unwrap();
    row.status = MatrixRowStatus::Blocked;
    row.blocked_on.clear();

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();

    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.acp.transport_profile"));
    assert!(error.to_string().contains("blocked_on prerequisites"));

    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.acp.transport_profile")
        .unwrap();
    row.blocked_on = vec!["   ".to_owned()];

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.acp.transport_profile"));
    assert!(error.to_string().contains("blocked_on prerequisites"));
}

#[test]
fn conformance_evidence_audit_rejects_stale_blocked_on_for_non_blocked_rows() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.acp.transport_profile")
        .unwrap();
    row.status = MatrixRowStatus::Pending;
    assert_eq!(row.status, MatrixRowStatus::Pending);
    row.blocked_on = vec!["stale prerequisite".to_owned()];

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();

    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.acp.transport_profile"));
    assert!(
        error
            .to_string()
            .contains("blocked_on prerequisites but is not blocked")
    );
}

#[test]
fn conformance_evidence_audit_requires_positive_tests_for_structural_rows() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.authority.manifest_inventory")
        .unwrap();

    row.positive_test_evidence.clear();

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(
        error
            .to_string()
            .contains("ps1.authority.manifest_inventory")
    );
    assert!(error.to_string().contains("positive test evidence"));
}

#[test]
fn conformance_evidence_audit_rejects_proven_rows_without_row_specific_review_signoff() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.evaluator.assessment_scope")
        .unwrap();

    row.review_evidence =
        vec!["docs/specs/public-seam-v1/reviews/2026-05-23-assessment-scope-review.md".to_owned()];

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.evaluator.assessment_scope"));
    assert!(
        error
            .to_string()
            .contains("row-specific adversarial sign-off")
    );
}

#[test]
fn conformance_evidence_audit_rejects_same_row_blocker_review_as_signoff() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.acp.auth_permissions")
        .unwrap();

    row.review_evidence = vec![
        "docs/specs/public-seam-v1/reviews/2026-05-23-acp-auth-permissions-blocker-review.md"
            .to_owned(),
    ];

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.acp.auth_permissions"));
    assert!(
        error
            .to_string()
            .contains("row-specific adversarial sign-off")
    );
}

#[test]
fn conformance_evidence_audit_rejects_partial_review_preamble_signoff_language() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.lm.contract")
        .unwrap();

    row.review_evidence = vec![
        "docs/specs/public-seam-v1/reviews/2026-05-24-lm-result-contract-partial-review.md"
            .to_owned(),
    ];

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.lm.contract"));
    assert!(
        error
            .to_string()
            .contains("row-specific adversarial sign-off")
    );
}

#[test]
fn conformance_evidence_audit_rejects_negated_row_signoff_language() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.receipts.audit_currency")
        .unwrap();

    row.review_evidence = vec![
        "docs/specs/public-seam-v1/reviews/2026-05-23-result-receipt-replay-review.md".to_owned(),
    ];

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.receipts.audit_currency"));
    assert!(
        error
            .to_string()
            .contains("row-specific adversarial sign-off")
    );
}

#[test]
fn conformance_evidence_audit_rejects_provenance_substring_as_signoff() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.stage.payload_receipts")
        .unwrap();

    row.review_evidence = vec![
        "docs/specs/public-seam-v1/reviews/2026-05-23-stage-payload-current-blocker-review.md"
            .to_owned(),
    ];

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.stage.payload_receipts"));
    assert!(
        error
            .to_string()
            .contains("row-specific adversarial sign-off")
    );
}

#[test]
fn conformance_evidence_audit_rejects_signoff_section_pending_rows() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.acp.extension_results")
        .unwrap();

    row.review_evidence = vec![
        "docs/specs/public-seam-v1/reviews/2026-05-24-agent-sandbox-receipt-stream-review.md"
            .to_owned(),
    ];

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.acp.extension_results"));
    assert!(
        error
            .to_string()
            .contains("row-specific adversarial sign-off")
    );
}

#[test]
fn conformance_evidence_audit_rejects_signoff_section_remains_pending_rows() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.workspace.handles_lifecycle")
        .unwrap();

    row.review_evidence = vec![
        "docs/specs/public-seam-v1/reviews/2026-05-24-workspace-view-query-helper-review.md"
            .to_owned(),
    ];

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(
        error
            .to_string()
            .contains("ps1.workspace.handles_lifecycle")
    );
    assert!(
        error
            .to_string()
            .contains("row-specific adversarial sign-off")
    );
}

#[test]
fn conformance_evidence_audit_rejects_happy_path_only_denial_rows() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.schema.fingerprints")
        .unwrap();

    row.negative_test_evidence.clear();

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.schema.fingerprints"));
    assert!(error.to_string().contains("negative test evidence"));
}

#[test]
fn conformance_evidence_audit_rejects_matrix_only_closeout_even_with_test_links() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.schema.fingerprints")
        .unwrap();

    row.implementation_evidence =
        vec!["docs/specs/public-seam-v1/conformance-matrix.yaml".to_owned()];

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.schema.fingerprints"));
    assert!(error.to_string().contains("matrix proof"));
}

#[test]
fn conformance_evidence_audit_maps_every_note_case_to_matrix_rows() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let denominator = package.conformance_test_denominator().unwrap();

    assert_eq!(denominator.cases.len(), 17);
    assert!(
        denominator
            .cases
            .iter()
            .any(|case| { case.id == "reject_reflector_reads_case_target" && case.is_negative() })
    );
    assert!(denominator.cases.iter().any(|case| {
        case.id == "accept_per_assessment_mixed_replayability_compute_plan_roll_up_summary"
            && !case.is_negative()
    }));

    let mut matrix = package.conformance_matrix().unwrap();
    matrix
        .rows
        .iter_mut()
        .for_each(|row| row.conformance_tests.clear());
    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(
        error
            .to_string()
            .contains("conformance test `reject_reflector_reads_case_target` is not mapped")
    );
}

#[test]
fn conformance_evidence_audit_rejects_weak_test_function_as_negative_evidence() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.schema.fingerprints")
        .unwrap();

    row.negative_test_evidence = vec![
        "crates/leaven-public-seam/tests/public_seam_contract/contract_package.rs::active_schemas_compile_and_examples_validate_against_manifest_targets"
            .to_owned(),
    ];

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.schema.fingerprints"));
    assert!(
        error
            .to_string()
            .contains("does not look like denial evidence")
    );
}

#[test]
fn conformance_evidence_audit_requires_denial_evidence_for_integrated_surface_rows() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut matrix = package.conformance_matrix().unwrap();
    let row = matrix
        .rows
        .iter_mut()
        .find(|row| row.id == "ps1.acp.transport_profile")
        .unwrap();

    row.status = MatrixRowStatus::Proven;
    row.blocked_on.clear();
    row.implementation_evidence =
        vec!["crates/leaven-public-seam/src/package.rs::PublicSeamPackage::v1_scope".to_owned()];
    row.review_evidence = vec![
        "docs/specs/public-seam-v1/reviews/2026-05-24-acp-stdio-transport-heisenberg-review.md"
            .to_owned(),
    ];
    row.positive_test_evidence = vec![
        "crates/leaven-public-seam/tests/public_seam_contract/contract_package.rs::v1_scope_markers_refuse_mcp_watch_runtime_and_legacy_worker_protocol"
            .to_owned(),
    ];
    row.negative_test_evidence.clear();

    let error = package.audit_conformance_evidence(&matrix).unwrap_err();
    assert!(matches!(error, PublicSeamError::InvalidMatrix { .. }));
    assert!(error.to_string().contains("ps1.acp.transport_profile"));
    assert!(error.to_string().contains("negative test evidence"));
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

fn finite_diff_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "watchdiff001",
        "consistency": {
            "kind": "since_revision",
            "since": "rev_base",
            "until": "rev_tip"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "let",
                "name": "events",
                "expr": {
                    "kind": "graph_query",
                    "source": {
                        "kind": "events",
                        "since_revision": "rev_base",
                        "until_revision": "rev_tip"
                    },
                    "projection": {
                        "kind": "ids"
                    },
                    "page": {
                        "limit": 100
                    }
                }
            }
        ],
        "return": ["events"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn copy_dir_all(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = to.as_ref().join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(entry.path(), target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
