use leaven_public_seam::{
    AcpAuthenticateRequest, AcpPermissionRequest, AcpSessionLifecycle, AcpWorkerSession,
    CapabilityDocument, CapabilityRegistry, PublicSeamError, PublicSeamPackage,
};
use serde_json::{Value, json};

#[test]
fn acp_profile_validates_pinned_stdio_leaven_methods_and_bounded_updates() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();

    assert_eq!(profile.pinned_acp_version(), "0.4.0");
    assert_eq!(
        profile.transports(),
        &["stdio_jsonrpc".to_owned(), "unix_socket_jsonrpc".to_owned()]
    );
    assert_eq!(profile.default_max_inflight_updates(), 32);
    assert_eq!(
        profile.extension_methods().len(),
        locked_profile_methods().len()
    );
    assert_eq!(
        profile
            .method("leaven/proposal.apply")
            .unwrap()
            .required_action(),
        "proposal.apply_batch"
    );
    assert_eq!(
        profile
            .method("leaven/lm.complete")
            .unwrap()
            .params_schema(),
        "leaven.plan.v1.schema.json"
    );
    assert!(
        profile
            .method("leaven/lm.complete")
            .unwrap()
            .produces_receipt()
    );
}

#[test]
fn acp_worker_session_uses_engine_client_worker_agent_inversion_and_bounded_updates() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();
    let mut session = AcpWorkerSession::start(&profile).unwrap();

    assert_eq!(session.pinned_acp_version(), "0.4.0");
    assert_eq!(session.transport(), "stdio_jsonrpc");
    assert_eq!(session.engine_role(), "engine_client");
    assert_eq!(session.worker_role(), "worker_agent");
    assert_eq!(session.lifecycle().max_inflight_updates(), 32);

    let update = session
        .lifecycle_mut()
        .enqueue_progress("scorer started")
        .unwrap();
    assert_eq!(update.sequence(), 0);
    assert_eq!(update.message(), "scorer started");
    assert_eq!(session.lifecycle().inflight_updates(), 1);
    assert_eq!(
        session
            .lifecycle_mut()
            .acknowledge_oldest_update()
            .unwrap()
            .message(),
        "scorer started"
    );

    let cancellation = session.lifecycle_mut().cancel("operator cancelled");
    assert_eq!(cancellation.reason(), "operator cancelled");
    assert!(session.lifecycle().is_cancelled());
    assert!(matches!(
        session
            .lifecycle_mut()
            .enqueue_progress("late progress")
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));
}

#[test]
fn acp_lifecycle_rejects_unbounded_or_overproducing_progress_queues() {
    assert!(matches!(
        AcpSessionLifecycle::bounded(0).unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut lifecycle = AcpSessionLifecycle::bounded(1).unwrap();
    lifecycle.enqueue_progress("first").unwrap();
    assert!(matches!(
        lifecycle.enqueue_progress("second").unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));
    assert_eq!(
        lifecycle.acknowledge_oldest_update().unwrap().message(),
        "first"
    );
    lifecycle.enqueue_progress("after ack").unwrap();
}

#[test]
fn acp_profile_rejects_mcp_latest_nonstdio_human_granting_and_unbounded_updates() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut mcp_method = acp_profile();
    mcp_method["extension_methods"][0]["method"] = json!("mcp/tools.call");
    assert!(matches!(
        package
            .validate_acp_profile_document(&mcp_method)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. } | PublicSeamError::InvalidScope { .. }
    ));

    let mut private_process = acp_profile();
    private_process["extension_methods"]
        .as_array_mut()
        .unwrap()
        .push(extension_method("leaven/private.process", "extension.call"));
    assert!(matches!(
        package
            .validate_acp_profile_document(&private_process)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut missing_locked_method = acp_profile();
    missing_locked_method["extension_methods"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(matches!(
        package
            .validate_acp_profile_document(&missing_locked_method)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut latest = acp_profile();
    latest["pinned_acp_version"] = json!("latest");
    assert!(matches!(
        package.validate_acp_profile_document(&latest).unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut nonstdio = acp_profile();
    nonstdio["transports"] = json!(["unix_socket_jsonrpc", "stdio_jsonrpc"]);
    assert!(matches!(
        package
            .validate_acp_profile_document(&nonstdio)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut human_grants = acp_profile();
    human_grants["permission_model"]["answer"] = json!("ask a human");
    assert!(matches!(
        package
            .validate_acp_profile_document(&human_grants)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. } | PublicSeamError::InvalidScope { .. }
    ));

    let mut unbounded = acp_profile();
    unbounded["flow_control"]["bounded_channel_required"] = json!(false);
    assert!(matches!(
        package
            .validate_acp_profile_document(&unbounded)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. } | PublicSeamError::InvalidScope { .. }
    ));

    let mut bare_result = acp_profile();
    bare_result["extension_methods"][1]["produces_receipt"] = json!(false);
    assert!(matches!(
        package
            .validate_acp_profile_document(&bare_result)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut wrong_params_schema = acp_profile();
    wrong_params_schema["extension_methods"][0]["params_schema"] =
        json!("archived.worker_protocol.v1.schema.json");
    assert!(matches!(
        package
            .validate_acp_profile_document(&wrong_params_schema)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut wrong_result_schema = acp_profile();
    wrong_result_schema["extension_methods"][0]["result_schema"] =
        json!("bare.worker_result.schema.json");
    assert!(matches!(
        package
            .validate_acp_profile_document(&wrong_result_schema)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));
}

#[test]
fn acp_permissions_use_capability_grants_and_return_planerror_redactions() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();
    let capability = CapabilityDocument::from_value(acp_capability()).unwrap();
    let mut registry = CapabilityRegistry::default();
    registry.insert(capability.clone()).unwrap();

    let authenticated = package
        .authenticate_acp_session(
            &profile,
            &registry,
            AcpAuthenticateRequest::opaque("ltok_acp", "2026-05-23T00:10:00Z", "fp_cap_sha256_acp"),
        )
        .unwrap();
    assert_eq!(authenticated.capability_fingerprint(), "fp_cap_sha256_acp");
    assert_eq!(authenticated.policy_fingerprint(), "fp_policy_sha256_acp");

    let allowed = package.authorize_acp_permission(
        &profile,
        &capability,
        &authenticated,
        AcpPermissionRequest::new("leaven/lm.complete")
            .with_input_class("case.input")
            .with_model("gpt-test")
            .with_resource("run", json!("run_demo")),
    );
    assert!(allowed.allowed());
    assert_eq!(allowed.capability_fingerprint(), "fp_cap_sha256_acp");
    assert!(allowed.error().is_none());

    let denied = package.authorize_acp_permission(
        &profile,
        &capability,
        &authenticated,
        AcpPermissionRequest::new("leaven/lm.complete")
            .with_input_class("case.input")
            .with_input_class("external.secret")
            .with_model("gpt-test")
            .with_resource("run", json!("run_demo")),
    );
    assert!(!denied.allowed());
    assert_eq!(denied.error().unwrap()["code"], json!("capability_denied"));
    assert_eq!(
        denied.redactions()[0]["reason"],
        json!("data_class_forbidden")
    );

    let unknown = package.authorize_acp_permission(
        &profile,
        &capability,
        &authenticated,
        AcpPermissionRequest::new("leaven/unknown.extension"),
    );
    assert!(!unknown.allowed());
    assert_eq!(unknown.error().unwrap()["code"], json!("extension_error"));

    let other_capability = CapabilityDocument::from_value({
        let mut value = acp_capability();
        value["capability_fingerprint"] = json!("fp_cap_sha256_other");
        value["jti"] = json!("jti_acp_other");
        value["token_binding"]["token_id"] = json!("ltok_other");
        value
    })
    .unwrap();
    let bypass = package.authorize_acp_permission(
        &profile,
        &other_capability,
        &authenticated,
        AcpPermissionRequest::new("leaven/lm.complete")
            .with_input_class("case.input")
            .with_model("gpt-test")
            .with_resource("run", json!("run_demo")),
    );
    assert!(!bypass.allowed());
    assert_eq!(bypass.error().unwrap()["code"], json!("capability_denied"));
}

#[test]
fn acp_authenticate_rejects_unknown_expired_or_fingerprint_mismatched_tokens() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();
    let capability = CapabilityDocument::from_value(acp_capability()).unwrap();
    let mut registry = CapabilityRegistry::default();
    registry.insert(capability).unwrap();

    assert!(matches!(
        package
            .authenticate_acp_session(
                &profile,
                &registry,
                AcpAuthenticateRequest::opaque(
                    "ltok_missing",
                    "2026-05-23T00:10:00Z",
                    "fp_cap_sha256_acp"
                )
            )
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    assert!(matches!(
        package
            .authenticate_acp_session(
                &profile,
                &registry,
                AcpAuthenticateRequest::opaque(
                    "ltok_acp",
                    "2026-05-23T00:21:00Z",
                    "fp_cap_sha256_acp"
                )
            )
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    assert!(matches!(
        package
            .authenticate_acp_session(
                &profile,
                &registry,
                AcpAuthenticateRequest::opaque(
                    "ltok_acp",
                    "2026-05-23T00:10:00Z",
                    "fp_cap_sha256_other"
                )
            )
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));
}

#[test]
fn acp_permissions_deny_ungranted_models_workspace_ops_and_commands() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();
    let capability = CapabilityDocument::from_value(acp_capability()).unwrap();
    let mut registry = CapabilityRegistry::default();
    registry.insert(capability.clone()).unwrap();
    let authenticated = package
        .authenticate_acp_session(
            &profile,
            &registry,
            AcpAuthenticateRequest::opaque("ltok_acp", "2026-05-23T00:10:00Z", "fp_cap_sha256_acp"),
        )
        .unwrap();

    let workspace_allowed = package.authorize_acp_permission(
        &profile,
        &capability,
        &authenticated,
        AcpPermissionRequest::new("leaven/workspace.read_file")
            .with_resource("workspace_ids", json!("ws_acp"))
            .with_input_class("workspace.file")
            .with_workspace_op("read_file"),
    );
    assert!(workspace_allowed.allowed());

    let workspace_denied = package.authorize_acp_permission(
        &profile,
        &capability,
        &authenticated,
        AcpPermissionRequest::new("leaven/workspace.read_file")
            .with_resource("workspace_ids", json!("ws_acp"))
            .with_input_class("workspace.file")
            .with_workspace_op("git_diff"),
    );
    assert!(!workspace_denied.allowed());
    assert_eq!(
        workspace_denied.error().unwrap()["code"],
        json!("capability_denied")
    );

    let model_denied = package.authorize_acp_permission(
        &profile,
        &capability,
        &authenticated,
        AcpPermissionRequest::new("leaven/lm.complete")
            .with_input_class("case.input")
            .with_model("ungranted-model")
            .with_resource("run", json!("run_demo")),
    );
    assert!(!model_denied.allowed());
    assert_eq!(
        model_denied.error().unwrap()["code"],
        json!("capability_denied")
    );

    let sandbox_denied = package.authorize_acp_permission(
        &profile,
        &capability,
        &authenticated,
        AcpPermissionRequest::new("leaven/sandbox.exec")
            .with_resource("workspace_ids", json!("ws_acp"))
            .with_input_class("public")
            .with_workspace_op("exec")
            .with_command("python"),
    );
    assert!(!sandbox_denied.allowed());
    assert_eq!(
        sandbox_denied.error().unwrap()["code"],
        json!("capability_denied")
    );

    let case_target_allowed = package.authorize_acp_permission(
        &profile,
        &capability,
        &authenticated,
        AcpPermissionRequest::new("leaven/case.target")
            .with_resource("run", json!("run_demo"))
            .with_resource("evaluation_request_id", json!("evalreq_acp"))
            .with_case_field("target")
            .with_partition("validation")
            .with_input_class("case.target"),
    );
    assert!(case_target_allowed.allowed());

    let case_target_wrong_partition = package.authorize_acp_permission(
        &profile,
        &capability,
        &authenticated,
        AcpPermissionRequest::new("leaven/case.target")
            .with_resource("run", json!("run_demo"))
            .with_resource("evaluation_request_id", json!("evalreq_acp"))
            .with_case_field("target")
            .with_partition("train")
            .with_input_class("case.target"),
    );
    assert!(!case_target_wrong_partition.allowed());
    assert_eq!(
        case_target_wrong_partition.error().unwrap()["code"],
        json!("capability_denied")
    );
}

#[test]
fn acp_extension_results_require_receipts_capability_fingerprint_and_data_classes() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let result = package
        .validate_acp_extension_result_document(&extension_result())
        .unwrap();

    assert_eq!(result.method(), "leaven/lm.complete");
    assert_eq!(result.primary_kind(), "lm_response");
    assert_eq!(result.capability_fingerprint(), "fp_cap_sha256_acp");
    assert_eq!(result.receipt_count(), 1);
    assert_eq!(result.data_classes(), &["completion.raw".to_owned()]);

    let mut missing_receipts = extension_result();
    missing_receipts["receipts"] = json!([]);
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&missing_receipts)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
            | PublicSeamError::InvalidPlanResult { .. }
            | PublicSeamError::InvalidScope { .. }
    ));

    let mut missing_classes = extension_result();
    missing_classes["data_classes"] = json!([]);
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&missing_classes)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. } | PublicSeamError::InvalidScope { .. }
    ));

    let mut missing_capability = extension_result();
    missing_capability
        .as_object_mut()
        .unwrap()
        .remove("capability_fingerprint");
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&missing_capability)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let bare_payload = json!({
        "method": "leaven/lm.complete",
        "message": {
            "role": "assistant",
            "content": [{"kind": "text", "text": "ok"}]
        }
    });
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&bare_payload)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut missing_schema_required_value = extension_result();
    missing_schema_required_value["primary"]
        .as_object_mut()
        .unwrap()
        .remove("message");
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&missing_schema_required_value)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. } | PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn acp_extension_results_bind_worker_methods_to_primary_kinds_and_receipts() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    for (method, primary, receipt) in extension_result_cases() {
        let data_classes = primary
            .get("data_classes")
            .and_then(Value::as_array)
            .and_then(|items| items.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
            .unwrap_or_else(|| vec!["public"]);
        let result = package
            .validate_acp_extension_result_document(&extension_result_for(
                method,
                &primary,
                &receipt,
                &data_classes,
            ))
            .unwrap();

        assert_eq!(result.method(), method);
        assert_eq!(result.receipt_count(), 1);
    }
}

#[test]
fn acp_extension_results_reject_cross_method_payloads_unbound_receipts_and_data_class_gaps() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let wrong_primary = extension_result_for(
        "leaven/lm.complete",
        &agent_session_primary(),
        &call_receipt("lm_complete", "lmrec_acp"),
        &["public"],
    );
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&wrong_primary)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
            | PublicSeamError::InvalidPlanResult { .. }
            | PublicSeamError::InvalidScope { .. }
    ));

    let wrong_receipt = extension_result_for(
        "leaven/sandbox.exec",
        &sandbox_exec_primary(),
        &call_receipt("agent_run", "execrec_acp"),
        &["public"],
    );
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&wrong_receipt)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
            | PublicSeamError::InvalidPlanResult { .. }
            | PublicSeamError::InvalidScope { .. }
    ));

    let mut unbound_receipt = extension_result_for(
        "leaven/agent.run",
        &agent_session_primary(),
        &call_receipt("agent_run", "other_agentrec"),
        &["public"],
    );
    unbound_receipt["primary"]["receipt"] = json!("agentrec_acp");
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&unbound_receipt)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
            | PublicSeamError::InvalidPlanResult { .. }
            | PublicSeamError::InvalidScope { .. }
    ));

    let data_class_gap = extension_result_for(
        "leaven/workspace.read_file",
        &workspace_file_primary(),
        &query_receipt("qrec_workspace_file"),
        &["public"],
    );
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&data_class_gap)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut malformed_extension_primary = extension_result_for(
        "leaven/graph.query",
        &extension_primary("graph.query"),
        &query_receipt("qrec_graph"),
        &["public"],
    );
    malformed_extension_primary["primary"]
        .as_object_mut()
        .unwrap()
        .remove("namespace");
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&malformed_extension_primary)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn acp_extension_results_reject_forged_result_hashes_for_extension_and_receiptless_primaries() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut forged_same_kind_receipt = extension_result();
    forged_same_kind_receipt["receipts"][0]["result_hash"] =
        json!("fp_result_sha256_same_kind_unbound");
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&forged_same_kind_receipt)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. } | PublicSeamError::InvalidScope { .. }
    ));

    let mut forged_generic_extension_hash = extension_result_for(
        "leaven/graph.query",
        &extension_primary("graph.query"),
        &query_receipt("qrec_graph"),
        &["public"],
    );
    forged_generic_extension_hash["receipts"][0]["result_hash"] =
        json!("fp_result_sha256_same_kind_unbound");
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&forged_generic_extension_hash)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    for (method, primary, receipt) in [
        (
            "leaven/workspace.snapshot",
            workspace_snapshot_primary(),
            query_receipt("qrec_workspace_snapshot"),
        ),
        (
            "leaven/workspace.list",
            workspace_listing_primary(),
            query_receipt("qrec_workspace_list"),
        ),
        (
            "leaven/workspace.git_diff",
            workspace_diff_primary(),
            query_receipt("qrec_workspace_git_diff"),
        ),
    ] {
        let mut forged_receiptless_primary =
            extension_result_for(method, &primary, &receipt, &["workspace.file"]);
        forged_receiptless_primary["receipts"][0]["result_hash"] =
            json!("fp_result_sha256_same_kind_unbound");
        assert!(matches!(
            package
                .validate_acp_extension_result_document(&forged_receiptless_primary)
                .unwrap_err(),
            PublicSeamError::InvalidScope { .. }
        ));
    }
}

fn acp_profile() -> Value {
    json!({
        "schema_version": "leaven.acp_profile.v1",
        "base_protocol": "agent-client-protocol",
        "pinned_acp_version": "0.4.0",
        "transports": ["stdio_jsonrpc", "unix_socket_jsonrpc"],
        "auth": {
            "token_env": "LEAVEN_CAPABILITY_TOKEN",
            "endpoint_env": "LEAVEN_ENDPOINT",
            "fingerprint_env": "LEAVEN_CAPABILITY_FINGERPRINT",
            "http_header": "Authorization: Bearer <token>",
            "authenticate_maps_to": "leaven.capability.v1"
        },
        "permission_model": {
            "source": "ACP session/request_permission",
            "answer": "programmatic capability grant check",
            "denial": "PlanError + Redaction"
        },
        "extension_methods": locked_profile_methods(),
        "flow_control": {
            "bounded_channel_required": true,
            "default_max_inflight_updates": 32,
            "backpressure": "pause_worker",
            "heartbeat_ms": 1000
        }
    })
}

fn locked_profile_methods() -> Vec<Value> {
    vec![
        extension_method("leaven/graph.query", "graph.query"),
        extension_method("leaven/case.load", "case.read"),
        extension_method("leaven/case.input", "case.read"),
        extension_method("leaven/case.target", "case.read"),
        extension_method("leaven/case.metadata", "case.read"),
        extension_method("leaven/workspace.materialize", "workspace.materialize"),
        extension_method("leaven/workspace.snapshot", "workspace.read"),
        extension_method("leaven/workspace.list", "workspace.read"),
        extension_method("leaven/workspace.read_file", "workspace.read"),
        extension_method("leaven/workspace.stat", "workspace.read"),
        extension_method("leaven/workspace.digest", "workspace.read"),
        extension_method("leaven/workspace.git_log", "workspace.read"),
        extension_method("leaven/workspace.git_diff", "workspace.read"),
        extension_method("leaven/workspace.git_status", "workspace.read"),
        extension_method("leaven/workspace.capture_artifacts", "workspace.read"),
        extension_method("leaven/workspace.release", "workspace.release"),
        extension_method("leaven/lm.complete", "lm.complete"),
        extension_method("leaven/agent.run", "agent.run"),
        extension_method("leaven/sandbox.exec", "sandbox.exec"),
        extension_method("leaven/human.review", "human.review"),
        extension_method("leaven/proposal.submit_batch", "proposal.submit_batch"),
        extension_method("leaven/proposal.apply", "proposal.apply_batch"),
        extension_method("leaven/assessment.submit", "assessment.submit"),
        extension_method("leaven/evaluation.request", "evaluation.request"),
        extension_method("leaven/event.emit", "event.emit"),
    ]
}

fn extension_result_cases() -> Vec<(&'static str, Value, Value)> {
    let mut cases = Vec::new();
    cases.extend(query_extension_result_cases());
    cases.extend(workspace_extension_result_cases());
    cases.extend(effect_extension_result_cases());
    cases.extend(write_extension_result_cases());
    cases
}

fn query_extension_result_cases() -> Vec<(&'static str, Value, Value)> {
    vec![
        (
            "leaven/graph.query",
            extension_primary("graph.query"),
            query_receipt("qrec_graph"),
        ),
        (
            "leaven/case.load",
            extension_primary("case.load"),
            query_receipt("qrec_case_load"),
        ),
        (
            "leaven/case.input",
            extension_primary("case.input"),
            query_receipt("qrec_case_input"),
        ),
        (
            "leaven/case.target",
            extension_primary("case.target"),
            query_receipt("qrec_case_target"),
        ),
        (
            "leaven/case.metadata",
            extension_primary("case.metadata"),
            query_receipt("qrec_case_metadata"),
        ),
    ]
}

fn workspace_extension_result_cases() -> Vec<(&'static str, Value, Value)> {
    vec![
        (
            "leaven/workspace.materialize",
            workspace_handle_primary(),
            call_receipt("workspace_materialize", "wrec_materialize"),
        ),
        (
            "leaven/workspace.snapshot",
            workspace_snapshot_primary(),
            query_receipt("qrec_workspace_snapshot"),
        ),
        (
            "leaven/workspace.list",
            workspace_listing_primary(),
            query_receipt("qrec_workspace_list"),
        ),
        (
            "leaven/workspace.read_file",
            workspace_file_primary(),
            query_receipt("qrec_workspace_file"),
        ),
        (
            "leaven/workspace.stat",
            workspace_listing_primary(),
            query_receipt("qrec_workspace_stat"),
        ),
        (
            "leaven/workspace.digest",
            workspace_snapshot_primary(),
            query_receipt("qrec_workspace_digest"),
        ),
        (
            "leaven/workspace.git_log",
            workspace_diff_primary(),
            query_receipt("qrec_workspace_git_log"),
        ),
        (
            "leaven/workspace.git_diff",
            workspace_diff_primary(),
            query_receipt("qrec_workspace_git_diff"),
        ),
        (
            "leaven/workspace.git_status",
            workspace_diff_primary(),
            query_receipt("qrec_workspace_git_status"),
        ),
        (
            "leaven/workspace.capture_artifacts",
            workspace_listing_primary(),
            query_receipt("qrec_workspace_capture"),
        ),
        (
            "leaven/workspace.release",
            extension_primary("workspace.release"),
            call_receipt("workspace_release", "wrec_release"),
        ),
    ]
}

fn effect_extension_result_cases() -> Vec<(&'static str, Value, Value)> {
    vec![
        (
            "leaven/lm.complete",
            lm_response_primary(),
            call_receipt("lm_complete", "lmrec_acp"),
        ),
        (
            "leaven/agent.run",
            agent_session_primary(),
            call_receipt("agent_run", "agentrec_acp"),
        ),
        (
            "leaven/sandbox.exec",
            sandbox_exec_primary(),
            call_receipt("sandbox_exec", "execrec_acp"),
        ),
        (
            "leaven/human.review",
            extension_primary("human.review"),
            call_receipt("human_review", "humanrec_acp"),
        ),
    ]
}

fn write_extension_result_cases() -> Vec<(&'static str, Value, Value)> {
    vec![
        (
            "leaven/proposal.submit_batch",
            proposal_batch_primary(),
            write_receipt("submit_proposal_batch", "wrec_proposal_submit"),
        ),
        (
            "leaven/proposal.apply",
            apply_receipt_primary(),
            write_receipt("apply_proposal_batch", "wrec_proposal_apply"),
        ),
        (
            "leaven/assessment.submit",
            assessment_batch_primary(),
            write_receipt("submit_assessments", "wrec_assessment_submit"),
        ),
        (
            "leaven/evaluation.request",
            evaluation_request_primary(),
            write_receipt("request_evaluation", "wrec_evaluation_request"),
        ),
        (
            "leaven/event.emit",
            extension_primary("event.emit"),
            write_receipt("emit_run_event", "wrec_event_emit"),
        ),
    ]
}

fn extension_method(method: &str, action: &str) -> Value {
    json!({
        "method": method,
        "params_schema": "leaven.plan.v1.schema.json",
        "result_schema": "leaven.plan_result.v1.schema.json",
        "required_action": action,
        "produces_receipt": true
    })
}

fn acp_capability() -> Value {
    json!({
        "schema_version": "leaven.capability.v1",
        "jti": "jti_acp",
        "capability_fingerprint": "fp_cap_sha256_acp",
        "policy_fingerprint": "fp_policy_sha256_acp",
        "subject_fingerprint": "fp_subject_sha256_acp",
        "issuer": {
            "kind": "run_engine",
            "id": "engine_local"
        },
        "subject": {
            "kind": "stage_call",
            "run": "run_demo",
            "stage_call_id": "sc_acp",
            "role": "reflector"
        },
        "audience": ["leaven.acp.worker"],
        "issued_at": "2026-05-23T00:00:00Z",
        "expires_at": "2026-05-23T00:20:00Z",
        "expiry_behavior": "drain_inflight_no_new_ops",
        "token_binding": {
            "kind": "opaque_lookup",
            "token_id": "ltok_acp"
        },
        "revocation": {
            "mode": "issuer_epoch",
            "revocation_epoch": 7,
            "check": "on_every_request"
        },
        "renewal": {
            "mode": "renew_before_expiry",
            "max_extensions": 2,
            "max_total_lifetime_s": 3600
        },
        "budgets": {},
        "execution_policy": {
            "profile": "managed_sandbox",
            "network": "leaven_endpoint_only",
            "subprocess": "deny_except_sandbox_exec",
            "filesystem": "workspace_handles_only",
            "byo_effects": "forbidden"
        },
        "grants": [
            {
                "action": "lm.complete",
                "resource": {
                    "run": "run_demo"
                },
                "constraints": {
                    "allowed_input_classes": ["case.input"],
                    "forbidden_input_classes": ["external.secret"],
                    "models": ["gpt-test"]
                }
            },
            {
                "action": "case.read",
                "resource": {
                    "run": "run_demo",
                    "evaluation_request_id": "evalreq_acp"
                },
                "constraints": {
                    "case_fields": ["target"],
                    "partitions": ["validation"],
                    "allowed_input_classes": ["case.target"]
                }
            },
            {
                "action": "workspace.read",
                "resource": {
                    "workspace_ids": ["ws_acp"]
                },
                "constraints": {
                    "allowed_input_classes": ["workspace.file"],
                    "workspace_ops": ["read_file"]
                }
            },
            {
                "action": "sandbox.exec",
                "resource": {
                    "workspace_ids": ["ws_acp"]
                },
                "constraints": {
                    "allowed_input_classes": ["public"],
                    "workspace_ops": ["exec"],
                    "allowed_commands": ["cargo"]
                }
            }
        ],
        "delegation": {
            "may_delegate": false,
            "max_depth": 0,
            "must_attenuate": true,
            "allowed_actions": []
        }
    })
}

fn extension_result() -> Value {
    extension_result_for(
        "leaven/lm.complete",
        &lm_response_primary(),
        &call_receipt("lm_complete", "lmrec_acp"),
        &["completion.raw"],
    )
}

fn extension_result_for(
    method: &str,
    primary: &Value,
    receipt: &Value,
    data_classes: &[&str],
) -> Value {
    let mut result = json!({
        "method": method,
        "primary": primary,
        "receipts": [receipt],
        "redactions": [],
        "capability_fingerprint": "fp_cap_sha256_acp",
        "data_classes": data_classes
    });
    let schema_version = match result["receipts"][0]["kind"].as_str().unwrap() {
        "query" => "leaven.plan_query_result.v1",
        "call" => "leaven.plan_call_result.v1",
        "write" => "leaven.plan_write_result.v1",
        other => panic!("unexpected receipt kind {other}"),
    };
    let op_name = result["receipts"][0]["op_var"]
        .as_str()
        .unwrap_or("primary");
    result["receipts"][0]["result_hash"] = json!(prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": schema_version,
            "name": op_name,
            "value": result["primary"]
        }),
    ));
    result
}

fn extension_primary(op: &str) -> Value {
    json!({
        "kind": "extension",
        "namespace": "leaven",
        "op": op,
        "schema_fingerprint": "fp_schema_sha256_acpextension",
        "payload": {"status": "ok"}
    })
}

fn lm_response_primary() -> Value {
    json!({
        "kind": "lm_response",
        "message": {
            "role": "assistant",
            "content": [{"kind": "text", "text": "ok"}]
        },
        "graph_revision": "rev_acp",
        "data_classes": ["completion.raw"],
        "replayability": "fully_managed",
        "receipt": "lmrec_acp"
    })
}

fn workspace_handle_primary() -> Value {
    json!({
        "kind": "workspace_handle",
        "workspace": "ws_acp",
        "lifetime": "stage_call",
        "released": false,
        "graph_revision": "rev_acp",
        "data_classes": ["workspace.file"],
        "replayability": "fully_managed",
        "receipt": "wrec_materialize"
    })
}

fn workspace_snapshot_primary() -> Value {
    json!({
        "kind": "workspace_snapshot",
        "workspace": "ws_acp",
        "digest": "sha256:workspace",
        "graph_revision": "rev_acp",
        "data_classes": ["workspace.file"],
        "replayability": "pure_read"
    })
}

fn workspace_listing_primary() -> Value {
    json!({
        "kind": "workspace_listing",
        "entries": [{"path": "src/lib.rs", "kind": "file", "data_classes": ["workspace.file"]}],
        "graph_revision": "rev_acp",
        "data_classes": ["workspace.file"],
        "replayability": "pure_read"
    })
}

fn workspace_file_primary() -> Value {
    json!({
        "kind": "workspace_file",
        "path": "src/lib.rs",
        "content": "pub fn demo() {}",
        "graph_revision": "rev_acp",
        "data_classes": ["workspace.file"],
        "replayability": "pure_read",
        "receipt": "qrec_workspace_file"
    })
}

fn workspace_diff_primary() -> Value {
    json!({
        "kind": "workspace_diff",
        "text": " M src/lib.rs",
        "graph_revision": "rev_acp",
        "data_classes": ["workspace.file"],
        "replayability": "pure_read"
    })
}

fn agent_session_primary() -> Value {
    json!({
        "kind": "agent_session",
        "status": "completed",
        "graph_revision": "rev_acp",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "agentrec_acp"
    })
}

fn sandbox_exec_primary() -> Value {
    json!({
        "kind": "sandbox_exec",
        "status": "completed",
        "exit_code": 0,
        "graph_revision": "rev_acp",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "execrec_acp"
    })
}

fn proposal_batch_primary() -> Value {
    json!({
        "kind": "proposal_batch_receipt",
        "batch_id": "pb_acp",
        "proposal_ids": ["prop_acp"],
        "status": "committed",
        "graph_revision": "rev_acp",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "wrec_proposal_submit"
    })
}

fn apply_receipt_primary() -> Value {
    json!({
        "kind": "apply_receipt",
        "created_candidates": ["cand_acp_created"],
        "status": "committed",
        "graph_revision": "rev_acp",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "wrec_proposal_apply"
    })
}

fn assessment_batch_primary() -> Value {
    json!({
        "kind": "assessment_batch_receipt",
        "assessment_ids": ["assess_acp"],
        "evaluation_request_id": "evalreq_acp",
        "per_assessment": [
            {
                "assessment": "assess_acp",
                "replayability": "fully_managed"
            }
        ],
        "status": "committed",
        "graph_revision": "rev_acp",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "wrec_assessment_submit"
    })
}

fn evaluation_request_primary() -> Value {
    json!({
        "kind": "evaluation_request_receipt",
        "evaluation_request_id": "evalreq_acp",
        "status": "recorded",
        "graph_revision": "rev_acp",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "wrec_evaluation_request"
    })
}

fn call_receipt(call_kind: &str, receipt: &str) -> Value {
    json!({
        "kind": "call",
        "receipt": receipt,
        "op_var": "worker_call",
        "started_at": "2026-05-23T00:00:00Z",
        "completed_at": "2026-05-23T00:00:01Z",
        "call_kind": call_kind,
        "request_hash": "fp_request_sha256_acp",
        "result_hash": "fp_result_sha256_acp",
        "runtime_fingerprint": "fp_runtime_sha256_acp",
        "status": "succeeded"
    })
}

fn write_receipt(write_kind: &str, receipt: &str) -> Value {
    let mut value = json!({
        "kind": "write",
        "receipt": receipt,
        "op_var": "primary",
        "started_at": "2026-05-23T00:00:00Z",
        "completed_at": "2026-05-23T00:00:01Z",
        "write_kind": write_kind,
        "request_hash": "fp_request_sha256_acp",
        "result_hash": "fp_result_sha256_acp",
        "base_revision": "rev_acp",
        "committed_revision": "rev_acp",
        "status": "succeeded"
    });
    match write_kind {
        "submit_proposal_batch" => {
            value["proposal_batch_id"] = json!("pb_acp");
            value["proposal_ids"] = json!(["prop_acp"]);
        }
        "apply_proposal_batch" => {
            value["created_candidates"] = json!(["cand_acp_created"]);
        }
        "submit_assessments" => {
            value["evaluation_request_id"] = json!("evalreq_acp");
            value["assessment_ids"] = json!(["assess_acp"]);
            value["request_hash"] = json!(prefixed_jcs_hash(
                "fp_request_sha256_",
                &json!({
                    "schema_version": "leaven.submit_assessments_request.v1",
                    "evaluation_request_id": "evalreq_acp",
                    "assessment_ids": ["assess_acp"]
                }),
            ));
        }
        "request_evaluation" => {
            value["evaluation_request_id"] = json!("evalreq_acp");
        }
        "emit_run_event" => {
            value["event_id"] = json!("event_acp");
        }
        other => panic!("unexpected write kind {other}"),
    }
    value
}

fn query_receipt(receipt: &str) -> Value {
    json!({
        "kind": "query",
        "receipt": receipt,
        "op_var": "workspace_read",
        "started_at": "2026-05-23T00:00:00Z",
        "completed_at": "2026-05-23T00:00:01Z",
        "op_hash": "fp_query_sha256_acp",
        "result_hash": "fp_result_sha256_acp",
        "graph_revision": "rev_acp",
        "status": "succeeded",
        "read_scope_fingerprint": "fp_scope_sha256_acp",
        "projection_fingerprint": "fp_projection_sha256_acp"
    })
}

fn prefixed_jcs_hash(prefix: &str, value: &Value) -> String {
    format!(
        "{prefix}{}",
        jcs_canonicalize::sha256_jcs_hex(value).unwrap()
    )
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}
