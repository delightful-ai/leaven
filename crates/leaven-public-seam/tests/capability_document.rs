use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use leaven_public_seam::{
    CapabilityDenialKind, CapabilityDocument, CapabilityError, CapabilityGrantRequest,
    CapabilityLimitUsage, CapabilityRegistry, PublicSeamPackage,
};
use serde_json::{Value, json};

#[test]
fn opaque_token_resolves_to_structured_capability_document() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let document = CapabilityDocument::from_value(example_capability(&package)).unwrap();
    let mut registry = CapabilityRegistry::default();
    registry.insert(document).unwrap();

    let resolved = registry
        .resolve_opaque_for_new_operation("ltok_eval_01", "2026-05-23T00:10:00Z")
        .unwrap();

    assert_eq!(resolved.jti(), "jti_eval_01");
    assert_eq!(
        resolved.subject_fingerprint(),
        "fp_subject_sha256_evalsubject"
    );
    assert_eq!(resolved.capability_fingerprint(), "fp_cap_sha256_eval01");
    assert_eq!(resolved.policy_fingerprint(), "fp_policy_sha256_evalpolicy");
    assert_eq!(resolved.opaque_token_id(), Some("ltok_eval_01"));
    assert_eq!(resolved.expires_at(), "2026-05-23T00:20:00Z");
    assert_eq!(resolved.revocation_mode(), Some("issuer_epoch"));
    assert_eq!(resolved.renewal_mode(), Some("renew_before_expiry"));
    assert_eq!(resolved.max_total_usd_micro(), Some(300_000));
    assert_eq!(
        resolved.audience(),
        &["leaven.acp.worker", "leaven.plan.eval"]
    );
    assert_eq!(resolved.issuer_kind(), "run_engine");
    assert_eq!(resolved.execution_policy_profile(), "managed_sandbox");
    assert_eq!(resolved.max_lm_usd_micro(), Some(150_000));
    assert_eq!(resolved.max_agent_usd_micro(), Some(150_000));
    assert_eq!(resolved.max_concurrent_calls(), Some(4));
    assert_eq!(resolved.delegation_allowed_actions(), &[] as &[String]);
    assert_eq!(
        resolved
            .grant("case.read")
            .unwrap()
            .resource
            .get("evaluation_request_id")
            .and_then(Value::as_str),
        Some("evalreq_01")
    );
    assert_eq!(
        resolved
            .grant("lm.complete")
            .unwrap()
            .limits
            .as_ref()
            .unwrap()
            .get("max_calls")
            .and_then(Value::as_u64),
        Some(20)
    );
    assert_eq!(
        resolved.grant_actions().collect::<BTreeSet<_>>(),
        BTreeSet::from(["assessment.submit", "case.read", "lm.complete"])
    );
}

#[test]
fn capability_resolution_rejects_bare_missing_expired_revoked_and_mismatched_tokens() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let document = CapabilityDocument::from_value(example_capability(&package)).unwrap();
    let mut registry = CapabilityRegistry::default();
    registry.insert(document).unwrap();

    assert!(matches!(
        registry
            .resolve_opaque_for_new_operation("ltok_missing", "2026-05-23T00:10:00Z")
            .unwrap_err(),
        CapabilityError::UnknownToken { .. }
    ));

    let mut missing_subject = example_capability(&package);
    missing_subject
        .as_object_mut()
        .unwrap()
        .remove("subject_fingerprint");
    assert!(matches!(
        CapabilityDocument::from_value(missing_subject).unwrap_err(),
        CapabilityError::InvalidDocument { .. }
    ));

    assert!(matches!(
        registry
            .resolve_opaque_for_new_operation("ltok_eval_01", "2026-05-23T00:20:01Z")
            .unwrap_err(),
        CapabilityError::Expired { .. }
    ));

    registry.revoke_jti("jti_eval_01");
    assert!(matches!(
        registry
            .resolve_opaque_for_new_operation("ltok_eval_01", "2026-05-23T00:10:00Z")
            .unwrap_err(),
        CapabilityError::Revoked { .. }
    ));

    let mismatched = CapabilityDocument::from_value({
        let mut value = example_capability(&package);
        value["token_binding"]["token_id"] = json!("ltok_other");
        value
    })
    .unwrap();
    let mut registry = CapabilityRegistry::default();
    registry
        .insert_with_opaque_handle("ltok_eval_01", mismatched)
        .unwrap();
    assert!(matches!(
        registry
            .resolve_opaque_for_new_operation("ltok_eval_01", "2026-05-23T00:10:00Z")
            .unwrap_err(),
        CapabilityError::BindingMismatch { .. }
    ));
}

#[test]
fn capability_documents_must_satisfy_locked_schema_not_partial_struct_shape() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    for field in ["issuer", "audience", "issued_at", "execution_policy"] {
        let mut value = example_capability(&package);
        value.as_object_mut().unwrap().remove(field);
        assert!(matches!(
            CapabilityDocument::from_value(value).unwrap_err(),
            CapabilityError::InvalidDocument { .. }
        ));
    }

    let mut extra = example_capability(&package);
    extra["unexpected"] = json!(true);
    assert!(matches!(
        CapabilityDocument::from_value(extra).unwrap_err(),
        CapabilityError::InvalidDocument { .. }
    ));

    let mut invalid_revocation = example_capability(&package);
    invalid_revocation["revocation"]["mode"] = json!("sometimes");
    assert!(matches!(
        CapabilityDocument::from_value(invalid_revocation).unwrap_err(),
        CapabilityError::InvalidDocument { .. }
    ));

    let mut invalid_grant_action = example_capability(&package);
    invalid_grant_action["grants"][0]["action"] = json!("Case Read");
    assert!(matches!(
        CapabilityDocument::from_value(invalid_grant_action).unwrap_err(),
        CapabilityError::InvalidDocument { .. }
    ));
}

#[test]
fn allowed_grant_request_returns_capability_fingerprint_and_effective_limits() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let document = CapabilityDocument::from_value(example_capability(&package)).unwrap();

    let allowed = document
        .authorize_grant(
            CapabilityGrantRequest::for_action("lm.complete")
                .with_resource("run", json!("run_demo"))
                .with_resource("lm_pool", json!("trusted-grader"))
                .with_purpose("evaluation_judge")
                .with_model_role("grader")
                .with_input_class("case.input")
                .with_input_class("case.target")
                .with_input_class("candidate.output")
                .with_input_class("workspace.file")
                .with_limits(CapabilityLimitUsage {
                    usd_micro: Some(15_000),
                    calls: Some(2),
                    ..CapabilityLimitUsage::default()
                }),
        )
        .unwrap();

    assert_eq!(allowed.capability_fingerprint(), "fp_cap_sha256_eval01");
    assert_eq!(allowed.policy_fingerprint(), "fp_policy_sha256_evalpolicy");
    assert_eq!(allowed.grant_action(), "lm.complete");
    assert_eq!(allowed.max_usd_micro(), Some(150_000));
    assert_eq!(allowed.max_calls(), Some(20));
    assert_eq!(allowed.max_concurrent(), None);
    assert_eq!(allowed.timeout_s(), None);
    assert_eq!(allowed.max_rows(), None);
    assert_eq!(allowed.max_materialized_bytes(), None);
}

#[test]
fn grant_enforcement_rejects_under_specified_requests() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let document = CapabilityDocument::from_value(enforcement_capability(&package)).unwrap();

    assert_denied(
        document.authorize_grant(CapabilityGrantRequest::for_action("agent.run")),
        CapabilityDenialKind::Action,
    );

    assert_denied(
        document.authorize_grant(CapabilityGrantRequest::for_action("lm.complete")),
        CapabilityDenialKind::Resource,
    );

    assert_denied(
        document.authorize_grant(
            CapabilityGrantRequest::for_action("lm.complete")
                .with_resource("run", json!("run_demo"))
                .with_resource("lm_pool", json!("trusted-grader")),
        ),
        CapabilityDenialKind::DataClass,
    );

    assert_denied(
        document.authorize_grant(
            CapabilityGrantRequest::for_action("lm.complete")
                .with_resource("run", json!("run_demo"))
                .with_resource("lm_pool", json!("trusted-grader"))
                .with_purpose("evaluation_judge")
                .with_model_role("grader")
                .with_input_class("case.input"),
        ),
        CapabilityDenialKind::Limit,
    );

    assert_denied(
        document.authorize_grant(
            CapabilityGrantRequest::for_action("case.read")
                .with_resource("run", json!("run_demo"))
                .with_resource("evaluation_request_id", json!("evalreq_01"))
                .with_case_field("input"),
        ),
        CapabilityDenialKind::Partition,
    );

    assert_denied(
        document.authorize_grant(
            CapabilityGrantRequest::for_action("proposal.submit_batch")
                .with_resource("run", json!("run_demo"))
                .with_schema("fp_schema_sha256_allowed"),
        ),
        CapabilityDenialKind::Surface,
    );

    assert_denied(
        document.authorize_grant(
            CapabilityGrantRequest::for_action("human.review")
                .with_resource("run", json!("run_demo")),
        ),
        CapabilityDenialKind::Limit,
    );
}

#[test]
fn grant_enforcement_rejects_action_resource_case_schema_data_class_and_limits() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let document = CapabilityDocument::from_value(enforcement_capability(&package)).unwrap();

    assert_denied(
        document.authorize_grant(
            CapabilityGrantRequest::for_action("lm.complete")
                .with_resource("run", json!("run_demo"))
                .with_resource("lm_pool", json!("untrusted-writer"))
                .with_purpose("evaluation_judge")
                .with_model_role("grader")
                .with_input_class("case.input"),
        ),
        CapabilityDenialKind::Resource,
    );

    assert_denied(
        document.authorize_grant(
            CapabilityGrantRequest::for_action("case.read")
                .with_resource("run", json!("run_demo"))
                .with_resource("evaluation_request_id", json!("evalreq_01"))
                .with_case_field("target")
                .with_partition("validation"),
        ),
        CapabilityDenialKind::CaseField,
    );

    assert_denied(
        document.authorize_grant(
            CapabilityGrantRequest::for_action("case.read")
                .with_resource("run", json!("run_demo"))
                .with_resource("evaluation_request_id", json!("evalreq_01"))
                .with_case_field("input")
                .with_partition("train"),
        ),
        CapabilityDenialKind::Partition,
    );

    assert_denied(
        document.authorize_grant(
            CapabilityGrantRequest::for_action("proposal.submit_batch")
                .with_resource("run", json!("run_demo"))
                .with_schema("fp_schema_sha256_other")
                .with_surface("fp_surface_sha256_allowed"),
        ),
        CapabilityDenialKind::Schema,
    );

    assert_denied(
        document.authorize_grant(
            CapabilityGrantRequest::for_action("proposal.submit_batch")
                .with_resource("run", json!("run_demo"))
                .with_schema("fp_schema_sha256_allowed")
                .with_surface("fp_surface_sha256_other"),
        ),
        CapabilityDenialKind::Surface,
    );

    let denial = document
        .authorize_grant(
            CapabilityGrantRequest::for_action("lm.complete")
                .with_resource("run", json!("run_demo"))
                .with_resource("lm_pool", json!("trusted-grader"))
                .with_purpose("evaluation_judge")
                .with_model_role("grader")
                .with_input_class("case.input")
                .with_input_class("external.secret"),
        )
        .unwrap_err();
    assert_eq!(denial.kind(), CapabilityDenialKind::DataClass);
    assert_eq!(denial.redactions(), &["external.secret"]);

    assert_denied(
        document.authorize_grant(
            CapabilityGrantRequest::for_action("lm.complete")
                .with_resource("run", json!("run_demo"))
                .with_resource("lm_pool", json!("trusted-grader"))
                .with_purpose("evaluation_judge")
                .with_model_role("grader")
                .with_input_class("case.input")
                .with_limits(CapabilityLimitUsage {
                    usd_micro: Some(150_001),
                    calls: Some(1),
                    ..CapabilityLimitUsage::default()
                }),
        ),
        CapabilityDenialKind::Limit,
    );
}

#[test]
fn grant_enforcement_rejects_timeout_and_row_limit_overruns() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let document = CapabilityDocument::from_value(enforcement_capability(&package)).unwrap();

    for limits in [
        CapabilityLimitUsage {
            timeout_s: Some(31),
            rows: Some(10),
            ..CapabilityLimitUsage::default()
        },
        CapabilityLimitUsage {
            timeout_s: Some(30),
            rows: Some(11),
            ..CapabilityLimitUsage::default()
        },
    ] {
        assert_denied(
            document.authorize_grant(
                CapabilityGrantRequest::for_action("human.review")
                    .with_resource("run", json!("run_demo"))
                    .with_limits(limits),
            ),
            CapabilityDenialKind::Limit,
        );
    }
}

fn example_capability(package: &PublicSeamPackage) -> Value {
    let path = package
        .root()
        .join("examples")
        .join("evaluator_capability.v0.3.example.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn enforcement_capability(package: &PublicSeamPackage) -> Value {
    let mut value = example_capability(package);
    value["grants"][0]["constraints"]["forbidden_case_fields"] = json!(["target"]);
    value["grants"].as_array_mut().unwrap().push(json!({
        "action": "proposal.submit_batch",
        "resource": {
            "run": "run_demo"
        },
        "constraints": {
            "allowed_surfaces": ["fp_surface_sha256_allowed"],
            "change_schemas": ["fp_schema_sha256_allowed"]
        }
    }));
    value["grants"].as_array_mut().unwrap().push(json!({
        "action": "human.review",
        "resource": {
            "run": "run_demo"
        },
        "constraints": {},
        "limits": {
            "timeout_s": 30,
            "max_rows": 10
        }
    }));
    value
}

fn assert_denied<T: std::fmt::Debug>(
    result: Result<T, leaven_public_seam::CapabilityDenial>,
    kind: CapabilityDenialKind,
) {
    let denial = result.unwrap_err();
    assert_eq!(denial.kind(), kind, "{denial:?}");
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}
