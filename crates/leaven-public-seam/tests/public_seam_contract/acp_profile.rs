use crate::support::package;
use crate::support::prefixed_jcs_hash;
use leaven_public_seam::{
    AcpAuthenticateRequest, AcpBackpressure, AcpPermissionRequest, AcpProgressDisposition,
    AcpProgressPriority, AcpSessionLifecycle, AcpSessionState, AcpStdioWorkerLaunch,
    AcpWorkerSession, CapabilityDocument, CapabilityRegistry, PublicSeamError, PublicSeamPackage,
};
use serde_json::{Value, json};

#[test]
fn acp_profile_validates_pinned_stdio_leaven_methods_and_bounded_updates() {
    let package = package();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();

    assert_eq!(profile.pinned_acp_version(), "0.4.0");
    assert_eq!(profile.token_env(), "LEAVEN_CAPABILITY_TOKEN");
    assert_eq!(profile.endpoint_env(), "LEAVEN_ENDPOINT");
    assert_eq!(profile.fingerprint_env(), "LEAVEN_CAPABILITY_FINGERPRINT");
    assert_eq!(
        profile.transports(),
        &["stdio_jsonrpc".to_owned(), "unix_socket_jsonrpc".to_owned()]
    );
    assert_eq!(profile.default_max_inflight_updates(), 32);
    assert_eq!(profile.backpressure(), AcpBackpressure::PauseWorker);
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
fn acp_stdio_worker_launch_uses_profile_env_and_redacts_bearer_artifacts() {
    let package = package();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();
    let session = AcpWorkerSession::start(&profile).unwrap();

    let launch = AcpStdioWorkerLaunch::new(
        &profile,
        &session,
        "secret-token",
        "stdio://worker/session",
        "fp_cap_sha256_launch",
    )
    .unwrap();

    assert_eq!(launch.transport(), "stdio_jsonrpc");
    assert_eq!(launch.engine_role(), "engine_client");
    assert_eq!(launch.worker_role(), "worker_agent");
    assert_eq!(
        launch.worker_env().get("LEAVEN_CAPABILITY_TOKEN"),
        Some(&"secret-token".to_owned())
    );
    assert_eq!(
        launch.worker_env().get("LEAVEN_ENDPOINT"),
        Some(&"stdio://worker/session".to_owned())
    );
    assert_eq!(
        launch.worker_env().get("LEAVEN_CAPABILITY_FINGERPRINT"),
        Some(&"fp_cap_sha256_launch".to_owned())
    );

    let artifact_env = launch.artifact_env();
    assert!(!artifact_env.contains_key("LEAVEN_CAPABILITY_TOKEN"));
    assert_eq!(
        artifact_env.get("LEAVEN_ENDPOINT"),
        Some(&"stdio://worker/session".to_owned())
    );
    launch.validate_artifact_env(&artifact_env).unwrap();

    assert!(matches!(
        launch
            .validate_artifact_env(launch.worker_env())
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut renamed_token_leak = artifact_env.clone();
    renamed_token_leak.insert("OTHER_TOKEN".to_owned(), "secret-token".to_owned());
    assert!(matches!(
        launch
            .validate_artifact_env(&renamed_token_leak)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut bearer_header_leak = artifact_env;
    bearer_header_leak.insert("Authorization".to_owned(), "Bearer secret-token".to_owned());
    assert!(matches!(
        launch
            .validate_artifact_env(&bearer_header_leak)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut composite_value_leak = launch.artifact_env();
    composite_value_leak.insert(
        "COMMAND".to_owned(),
        "run --header 'Authorization: Bearer secret-token'".to_owned(),
    );
    assert!(matches!(
        launch
            .validate_artifact_env(&composite_value_leak)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let debug = format!("{launch:?}");
    assert!(!debug.contains("secret-token"));
    assert!(debug.contains("<redacted>"));
}

#[test]
fn acp_stdio_worker_launch_rejects_missing_required_launch_facts() {
    let package = package();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();
    let session = AcpWorkerSession::start(&profile).unwrap();

    for (token, endpoint, fingerprint) in [
        ("", "stdio://worker/session", "fp_cap_sha256_launch"),
        ("secret-token", "", "fp_cap_sha256_launch"),
        ("secret-token", "stdio://worker/session", ""),
    ] {
        assert!(matches!(
            AcpStdioWorkerLaunch::new(&profile, &session, token, endpoint, fingerprint)
                .unwrap_err(),
            PublicSeamError::InvalidScope { .. }
        ));
    }
}

#[test]
fn acp_worker_session_uses_engine_client_worker_agent_inversion_and_bounded_updates() {
    let package = package();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();
    let mut session = AcpWorkerSession::start(&profile).unwrap();

    assert_eq!(session.pinned_acp_version(), "0.4.0");
    assert_eq!(session.transport(), "stdio_jsonrpc");
    assert_eq!(session.engine_role(), "engine_client");
    assert_eq!(session.worker_role(), "worker_agent");
    assert_eq!(session.lifecycle().max_inflight_updates(), 32);
    assert_eq!(
        session.lifecycle().backpressure(),
        AcpBackpressure::PauseWorker
    );
    assert_eq!(session.lifecycle().state(), AcpSessionState::Running);

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

    let cancellation = session
        .lifecycle_mut()
        .cancel_with_error(
            "operator cancelled",
            "valrec_operator_cancel",
            plan_error("valrec_operator_cancel", "cancelled", "operator cancelled"),
        )
        .unwrap();
    assert_eq!(cancellation.reason(), "operator cancelled");
    assert_eq!(cancellation.receipt(), "valrec_operator_cancel");
    assert_eq!(cancellation.error()["code"], json!("cancelled"));
    assert_eq!(
        cancellation.error()["receipt"],
        json!("valrec_operator_cancel")
    );
    assert_eq!(session.lifecycle().state(), AcpSessionState::Cancelled);
    assert!(session.lifecycle().is_cancelled());
    assert_eq!(
        session
            .lifecycle_mut()
            .cancel_with_error(
                "ignored duplicate",
                "valrec_duplicate_cancel",
                plan_error("valrec_duplicate_cancel", "cancelled", "ignored duplicate"),
            )
            .unwrap()
            .reason(),
        "operator cancelled"
    );
    assert!(matches!(
        session
            .lifecycle_mut()
            .enqueue_progress("late progress")
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));
}

#[test]
fn acp_lifecycle_cancellation_requires_receipts_and_closed_plan_errors() {
    let package = package();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();
    let mut lifecycle = AcpSessionLifecycle::from_profile(&profile).unwrap();

    for (receipt, error, expected) in [
        (
            "",
            plan_error("valrec_cancel", "cancelled", "operator cancelled"),
            "ACP cancellation receipt must be non-empty",
        ),
        (
            "valrec_cancel",
            json!({"code": "cancelled"}),
            "ACP cancellation error must carry message",
        ),
        (
            "valrec_cancel",
            plan_error("valrec_other", "cancelled", "operator cancelled"),
            "ACP cancellation error receipt must match cancellation receipt",
        ),
        (
            "valrec_cancel",
            plan_error("valrec_cancel", "not_a_closed_code", "operator cancelled"),
            "ACP cancellation error code must be a closed PlanError code",
        ),
        (
            "acprec_cancel",
            plan_error("acprec_cancel", "cancelled", "operator cancelled"),
            "ReceiptId grammar",
        ),
    ] {
        let error = lifecycle
            .cancel_with_error("operator cancelled", receipt, error)
            .unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected `{expected}` in {error:?}"
        );
        assert_eq!(lifecycle.state(), AcpSessionState::Running);
    }

    let cancellation = lifecycle
        .cancel_with_error(
            "operator cancelled",
            "valrec_cancel",
            plan_error("valrec_cancel", "cancelled", "operator cancelled"),
        )
        .unwrap();
    assert_eq!(cancellation.receipt(), "valrec_cancel");
    assert_eq!(cancellation.error()["code"], json!("cancelled"));
    assert_eq!(cancellation.error()["message"], json!("operator cancelled"));

    let mut object_ref_lifecycle = AcpSessionLifecycle::from_profile(&profile).unwrap();
    let cancellation = object_ref_lifecycle
        .cancel_with_error(
            "operator cancelled",
            "valrec_object_cancel",
            json!({
                "code": "cancelled",
                "message": "operator cancelled",
                "receipt": {
                    "kind": "receipt",
                    "id": "valrec_object_cancel",
                    "fingerprint": "fp_receipt_sha256_cancel"
                }
            }),
        )
        .unwrap();
    assert_eq!(cancellation.receipt(), "valrec_object_cancel");
    assert_eq!(
        cancellation.error()["receipt"]["id"],
        json!("valrec_object_cancel")
    );
}

#[test]
fn acp_lifecycle_rejects_unbounded_or_overproducing_progress_queues() {
    let package = package();
    let mut unbounded_profile = acp_profile();
    unbounded_profile["flow_control"]["default_max_inflight_updates"] = json!(0);
    assert!(matches!(
        package
            .validate_acp_profile_document(&unbounded_profile)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. } | PublicSeamError::InvalidScope { .. }
    ));

    let mut one_slot_profile = acp_profile();
    one_slot_profile["flow_control"]["default_max_inflight_updates"] = json!(1);
    let one_slot_profile = package
        .validate_acp_profile_document(&one_slot_profile)
        .unwrap();
    let mut lifecycle = AcpSessionLifecycle::from_profile(&one_slot_profile).unwrap();
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
fn acp_lifecycle_applies_profile_backpressure_strategies() {
    let package = package();

    let mut drop_profile = acp_profile();
    drop_profile["flow_control"]["default_max_inflight_updates"] = json!(1);
    drop_profile["flow_control"]["backpressure"] = json!("drop_noncritical_updates");
    let drop_profile = package
        .validate_acp_profile_document(&drop_profile)
        .unwrap();
    assert_eq!(
        drop_profile.backpressure(),
        AcpBackpressure::DropNoncriticalUpdates
    );
    let mut drop_lifecycle = AcpSessionLifecycle::from_profile(&drop_profile).unwrap();
    drop_lifecycle.enqueue_progress("first").unwrap();
    assert!(matches!(
        drop_lifecycle
            .offer_progress("noncritical", AcpProgressPriority::Noncritical)
            .unwrap(),
        AcpProgressDisposition::DroppedNoncritical
    ));
    assert_eq!(drop_lifecycle.inflight_updates(), 1);
    assert!(matches!(
        drop_lifecycle
            .offer_progress("critical", AcpProgressPriority::Critical)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
    ));

    let mut disconnect_profile = acp_profile();
    disconnect_profile["flow_control"]["default_max_inflight_updates"] = json!(1);
    disconnect_profile["flow_control"]["backpressure"] = json!("disconnect");
    let disconnect_profile = package
        .validate_acp_profile_document(&disconnect_profile)
        .unwrap();
    assert_eq!(
        disconnect_profile.backpressure(),
        AcpBackpressure::Disconnect
    );
    let mut disconnect_lifecycle = AcpSessionLifecycle::from_profile(&disconnect_profile).unwrap();
    disconnect_lifecycle.enqueue_progress("first").unwrap();
    assert!(matches!(
        disconnect_lifecycle
            .offer_progress("overflow", AcpProgressPriority::Critical)
            .unwrap(),
        AcpProgressDisposition::Disconnected(reason)
            if reason == "ACP session disconnected after update overflow"
    ));
    assert_eq!(disconnect_lifecycle.state(), AcpSessionState::Cancelled);
    let cancellation = disconnect_lifecycle.cancellation().unwrap();
    assert_eq!(cancellation.receipt(), "valrec_acp_disconnect_1");
    assert_eq!(cancellation.error()["code"], json!("cancelled"));
    assert_eq!(
        cancellation.error()["receipt"],
        json!("valrec_acp_disconnect_1")
    );

    let mut enqueue_disconnect = AcpSessionLifecycle::from_profile(&disconnect_profile).unwrap();
    let first = enqueue_disconnect.enqueue_progress("first").unwrap();
    assert_eq!(first.message(), "first");
    let error = enqueue_disconnect.enqueue_progress("second").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("ACP session disconnected after update overflow"),
        "unexpected enqueue disconnect error: {error:?}"
    );
    assert_eq!(enqueue_disconnect.state(), AcpSessionState::Cancelled);
    assert_eq!(enqueue_disconnect.inflight_updates(), 1);
    assert_eq!(
        enqueue_disconnect
            .acknowledge_oldest_update()
            .unwrap()
            .message(),
        "first"
    );

    let mut unknown = acp_profile();
    unknown["flow_control"]["backpressure"] = json!("spin_forever");
    assert!(matches!(
        package.validate_acp_profile_document(&unknown).unwrap_err(),
        PublicSeamError::ExampleValidation { .. } | PublicSeamError::InvalidScope { .. }
    ));
}

#[test]
fn acp_profile_rejects_mcp_latest_nonstdio_human_granting_and_unbounded_updates() {
    let package = package();

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
fn acp_jsonrpc_requests_and_responses_bind_plan_ir_and_extension_results() {
    let package = package();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();

    let request_value = json!({
        "jsonrpc": "2.0",
        "id": "req-lm-001",
        "method": "leaven/lm.complete",
        "params": acp_plan_params()
    });
    let request = package
        .validate_acp_jsonrpc_request_document(&profile, &request_value)
        .unwrap();
    assert_eq!(request.id(), "req-lm-001");
    assert_eq!(request.method(), "leaven/lm.complete");

    let response_value = json!({
        "jsonrpc": "2.0",
        "id": "req-lm-001",
        "result": extension_result()
    });
    let response = package
        .validate_acp_jsonrpc_response_document(&request, &response_value)
        .unwrap();
    assert_eq!(response.id(), "req-lm-001");
    assert_eq!(response.method(), "leaven/lm.complete");
    assert_eq!(response.primary_kind(), "lm_response");
}

#[test]
fn acp_jsonrpc_rejects_in_process_or_cross_method_fakes() {
    let package = package();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();

    let mut private_method = json!({
        "jsonrpc": "2.0",
        "id": "req-private-001",
        "method": "private/run_lm",
        "params": acp_plan_params()
    });
    assert!(matches!(
        package.validate_acp_jsonrpc_request_document(&profile, &private_method),
        Err(PublicSeamError::InvalidScope { .. })
    ));

    private_method["method"] = json!("leaven/mcp.bridge");
    assert!(matches!(
        package.validate_acp_jsonrpc_request_document(&profile, &private_method),
        Err(PublicSeamError::InvalidScope { .. })
    ));

    let bare_in_process_payload = json!({
        "jsonrpc": "2.0",
        "id": "req-bare-001",
        "method": "leaven/lm.complete",
        "params": {
            "message": {
                "role": "assistant",
                "content": [{"kind": "text", "text": "ok"}]
            }
        }
    });
    assert!(matches!(
        package.validate_acp_jsonrpc_request_document(&profile, &bare_in_process_payload),
        Err(PublicSeamError::ExampleValidation { .. } | PublicSeamError::InvalidPlan { .. })
    ));

    let smuggled_in_process_payload = json!({
        "jsonrpc": "2.0",
        "id": "req-smuggled-001",
        "method": "leaven/lm.complete",
        "params": acp_plan_params(),
        "private_process_payload": {
            "message": {
                "role": "assistant",
                "content": [{"kind": "text", "text": "ok"}]
            }
        }
    });
    assert!(matches!(
        package.validate_acp_jsonrpc_request_document(&profile, &smuggled_in_process_payload),
        Err(PublicSeamError::InvalidScope { .. })
    ));

    let agent_request_value = json!({
        "jsonrpc": "2.0",
        "id": "req-agent-001",
        "method": "leaven/agent.run",
        "params": acp_plan_params()
    });
    let agent_request = package
        .validate_acp_jsonrpc_request_document(&profile, &agent_request_value)
        .unwrap();
    let lm_response = json!({
        "jsonrpc": "2.0",
        "id": "req-agent-001",
        "result": extension_result()
    });
    assert!(matches!(
        package.validate_acp_jsonrpc_response_document(&agent_request, &lm_response),
        Err(PublicSeamError::InvalidScope { .. })
    ));

    let smuggled_response = json!({
        "jsonrpc": "2.0",
        "id": "req-agent-001",
        "result": extension_result_for(
            "leaven/agent.run",
            &agent_session_primary(),
            &call_receipt("agent_run", "agentrec_acp"),
            &["public", "transcript.raw"]
        ),
        "private_process_payload": {
            "stdout": "proposal patch without ACP receipt binding"
        }
    });
    assert!(matches!(
        package.validate_acp_jsonrpc_response_document(&agent_request, &smuggled_response),
        Err(PublicSeamError::InvalidScope { .. })
    ));

    let mismatched_id = json!({
        "jsonrpc": "2.0",
        "id": "other-request",
        "result": extension_result_for(
            "leaven/agent.run",
            &agent_session_primary(),
            &call_receipt("agent_run", "agentrec_acp"),
            &["public", "transcript.raw"]
        )
    });
    assert!(matches!(
        package.validate_acp_jsonrpc_response_document(&agent_request, &mismatched_id),
        Err(PublicSeamError::InvalidScope { .. })
    ));
}

#[test]
fn acp_permissions_use_capability_grants_and_return_planerror_redactions() {
    let package = package();
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
    let package = package();
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
    let package = package();
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
    let package = package();
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
    let package = package();

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
fn acp_extension_results_preserve_agent_and_sandbox_blob_ref_data_classes() {
    let package = package();

    let mut agent = agent_session_primary();
    agent["transcript_ref"] = acp_blob_ref("blob_agent_transcript", &["transcript.raw"]);
    agent["data_classes"] = json!(["public", "transcript.raw"]);
    let agent_result = package
        .validate_acp_extension_result_document(&extension_result_for(
            "leaven/agent.run",
            &agent,
            &call_receipt("agent_run", "agentrec_acp"),
            &["public", "transcript.raw"],
        ))
        .unwrap();
    assert_eq!(
        agent_result.data_classes(),
        &["public".to_owned(), "transcript.raw".to_owned()]
    );

    let mut sandbox = sandbox_exec_primary();
    sandbox["stdout_ref"] = acp_blob_ref("blob_stdout", &["transcript.raw"]);
    sandbox["stderr_ref"] = acp_blob_ref("blob_stderr", &["transcript.raw"]);
    sandbox["files"] = json!({
        "out.txt": acp_blob_ref("blob_out", &["workspace.file"])
    });
    sandbox["data_classes"] = json!(["public", "transcript.raw", "workspace.file"]);
    let sandbox_result = package
        .validate_acp_extension_result_document(&extension_result_for(
            "leaven/sandbox.exec",
            &sandbox,
            &call_receipt("sandbox_exec", "execrec_acp"),
            &["public", "transcript.raw", "workspace.file"],
        ))
        .unwrap();
    assert_eq!(
        sandbox_result.data_classes(),
        &[
            "public".to_owned(),
            "transcript.raw".to_owned(),
            "workspace.file".to_owned()
        ]
    );
}

#[test]
fn acp_extension_results_reject_agent_and_sandbox_blob_ref_data_class_gaps() {
    let package = package();

    let mut agent_transcript_gap = agent_session_primary();
    agent_transcript_gap["transcript_ref"] =
        acp_blob_ref("blob_agent_transcript", &["transcript.raw"]);
    let agent_transcript_gap = extension_result_for(
        "leaven/agent.run",
        &agent_transcript_gap,
        &call_receipt("agent_run", "agentrec_acp"),
        &["public"],
    );
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&agent_transcript_gap)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. } | PublicSeamError::InvalidScope { .. }
    ));

    let mut sandbox_stream_gap = sandbox_exec_primary();
    sandbox_stream_gap["stdout_ref"] = acp_blob_ref("blob_stdout", &["transcript.raw"]);
    sandbox_stream_gap["stderr_ref"] = acp_blob_ref("blob_stderr", &["transcript.raw"]);
    sandbox_stream_gap["files"] = json!({
        "out.txt": acp_blob_ref("blob_out", &["workspace.file"])
    });
    let sandbox_stream_gap = extension_result_for(
        "leaven/sandbox.exec",
        &sandbox_stream_gap,
        &call_receipt("sandbox_exec", "execrec_acp"),
        &["public"],
    );
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&sandbox_stream_gap)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. } | PublicSeamError::InvalidScope { .. }
    ));
}

#[test]
fn acp_extension_results_reject_lm_cost_audit_gaps() {
    let package = package();

    let mut missing_primary_cost = lm_response_primary();
    missing_primary_cost.as_object_mut().unwrap().remove("cost");
    let error = package
        .validate_acp_extension_result_document(&extension_result_for(
            "leaven/lm.complete",
            &missing_primary_cost,
            &call_receipt("lm_complete", "lmrec_acp"),
            &["completion.raw"],
        ))
        .unwrap_err();
    assert!(error.to_string().contains("cost"));

    let mut mismatched_receipt_cost = extension_result_for(
        "leaven/lm.complete",
        &lm_response_primary(),
        &call_receipt("lm_complete", "lmrec_acp"),
        &["completion.raw"],
    );
    mismatched_receipt_cost["receipts"][0]["cost"] = json!({"usd_micro": 1});
    let error = package
        .validate_acp_extension_result_document(&mismatched_receipt_cost)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("lm_complete primary cost must match call receipt cost")
    );

    let mut mismatched_primary_receipt = lm_response_primary();
    mismatched_primary_receipt["receipt"] = json!("lmrec_other");
    let mut mismatched_result = extension_result_for(
        "leaven/lm.complete",
        &mismatched_primary_receipt,
        &call_receipt("lm_complete", "lmrec_acp"),
        &["completion.raw"],
    );
    push_receipt_bound_to_primary(
        &mut mismatched_result,
        call_receipt("lm_complete", "lmrec_other"),
    );
    let error = package
        .validate_acp_extension_result_document(&mismatched_result)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("does not match expected receipt")
    );
}

#[test]
fn acp_extension_results_reject_agent_audit_gaps() {
    let package = package();

    for (primary, expected) in [
        (
            agent_session_primary_without("transcript_ref"),
            "agent_run result value must carry transcript_ref",
        ),
        (
            agent_session_primary_without("commands"),
            "agent_run result value must carry commands",
        ),
        (
            agent_session_primary_with_command(json!({
                "status": "completed",
                "receipt": "agentrec_acp"
            })),
            "agent_run command record must carry argv",
        ),
        (
            agent_session_primary_with_command(json!({
                "argv": [],
                "status": "completed",
                "receipt": "agentrec_acp"
            })),
            "agent_run command record argv must not be empty",
        ),
        (
            agent_session_primary_with_command(json!({
                "argv": ["codex", 42],
                "status": "completed",
                "receipt": "agentrec_acp"
            })),
            "agent_run command argv",
        ),
        (
            agent_session_primary_with_command(json!({
                "argv": ["codex"],
                "receipt": "agentrec_acp"
            })),
            "agent_run command status",
        ),
        (
            agent_session_primary_with_command(json!({
                "argv": ["codex"],
                "status": "completed",
                "receipt": "agentrec_other",
                "stdout_ref": acp_blob_ref("blob_agent_stdout", &["transcript.raw"]),
                "stderr_ref": acp_blob_ref("blob_agent_stderr", &["transcript.raw"])
            })),
            "agent_run command record receipt",
        ),
        (
            agent_session_primary_without("cost"),
            "agent_run result value must carry cost",
        ),
    ] {
        let error = package
            .validate_acp_extension_result_document(&extension_result_for(
                "leaven/agent.run",
                &primary,
                &call_receipt("agent_run", "agentrec_acp"),
                &["public", "transcript.raw"],
            ))
            .unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "unexpected ACP agent audit error for {primary:?}: {error:?}"
        );
    }

    assert_acp_agent_extension_rejects_missing_primary_receipt(&package);
    let mut mismatched_primary_receipt = agent_session_primary();
    mismatched_primary_receipt["receipt"] = json!("agentrec_other");
    let mut mismatched_result = extension_result_for(
        "leaven/agent.run",
        &mismatched_primary_receipt,
        &call_receipt("agent_run", "agentrec_acp"),
        &["public", "transcript.raw"],
    );
    push_receipt_bound_to_primary(
        &mut mismatched_result,
        call_receipt("agent_run", "agentrec_other"),
    );
    let error = package
        .validate_acp_extension_result_document(&mismatched_result)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("does not match expected receipt")
    );

    assert_acp_agent_extension_rejects_missing_receipt_cost(&package);
}

fn assert_acp_agent_extension_rejects_missing_primary_receipt(package: &PublicSeamPackage) {
    let mut missing_primary_receipt = agent_session_primary();
    missing_primary_receipt
        .as_object_mut()
        .unwrap()
        .remove("receipt");
    let error = package
        .validate_acp_extension_result_document(&extension_result_for(
            "leaven/agent.run",
            &missing_primary_receipt,
            &call_receipt("agent_run", "agentrec_acp"),
            &["public", "transcript.raw"],
        ))
        .unwrap_err();
    assert!(error.to_string().contains("primary.receipt"));
}

fn assert_acp_agent_extension_rejects_missing_receipt_cost(package: &PublicSeamPackage) {
    let mut missing_receipt_cost = extension_result_for(
        "leaven/agent.run",
        &agent_session_primary(),
        &call_receipt("agent_run", "agentrec_acp"),
        &["public", "transcript.raw"],
    );
    missing_receipt_cost["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("cost");
    let error = package
        .validate_acp_extension_result_document(&missing_receipt_cost)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("agent_run primary cost must match call receipt cost")
    );
}

#[test]
fn acp_extension_results_reject_sandbox_audit_gaps() {
    let package = package();

    for (primary, expected) in [
        (
            sandbox_exec_primary_without("cost"),
            "sandbox_exec result value must carry cost",
        ),
        (
            sandbox_exec_primary_without("exit_code"),
            "completed sandbox_exec result value must carry exit_code",
        ),
        (
            sandbox_exec_primary_without("stdout_ref"),
            "completed sandbox_exec result value must carry stdout_ref and stderr_ref",
        ),
        (
            sandbox_exec_primary_without("stderr_ref"),
            "completed sandbox_exec result value must carry stdout_ref and stderr_ref",
        ),
        (
            sandbox_exec_primary_with_file("/tmp/out.txt"),
            "sandbox_exec result file path must be relative workspace path",
        ),
        (
            sandbox_exec_primary_with_file("../out.txt"),
            "sandbox_exec result file path must be relative workspace path",
        ),
        (
            sandbox_exec_primary_with_file(""),
            "sandbox_exec result file path must be relative workspace path",
        ),
        (
            sandbox_exec_primary_with_file("out//log.txt"),
            "sandbox_exec result file path must be relative workspace path",
        ),
    ] {
        let error = package
            .validate_acp_extension_result_document(&extension_result_for(
                "leaven/sandbox.exec",
                &primary,
                &call_receipt("sandbox_exec", "execrec_acp"),
                &["public", "workspace.file"],
            ))
            .unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "unexpected ACP sandbox audit error for {primary:?}: {error:?}"
        );
    }

    let mut mismatched_primary_receipt = sandbox_exec_primary();
    mismatched_primary_receipt["receipt"] = json!("execrec_other");
    let mut mismatched_result = extension_result_for(
        "leaven/sandbox.exec",
        &mismatched_primary_receipt,
        &call_receipt("sandbox_exec", "execrec_acp"),
        &["public"],
    );
    push_receipt_bound_to_primary(
        &mut mismatched_result,
        call_receipt("sandbox_exec", "execrec_other"),
    );
    let error = package
        .validate_acp_extension_result_document(&mismatched_result)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("does not match expected receipt")
    );

    let mut mismatched_receipt_cost = extension_result_for(
        "leaven/sandbox.exec",
        &sandbox_exec_primary(),
        &call_receipt("sandbox_exec", "execrec_acp"),
        &["public"],
    );
    mismatched_receipt_cost["receipts"][0]["cost"] = json!({"usd_micro": 1});
    let error = package
        .validate_acp_extension_result_document(&mismatched_receipt_cost)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("sandbox_exec primary cost must match call receipt cost")
    );
}

#[test]
fn acp_extension_results_reject_cross_method_payloads_unbound_receipts_and_data_class_gaps() {
    let package = package();

    let wrong_primary = extension_result_for(
        "leaven/lm.complete",
        &agent_session_primary(),
        &call_receipt("lm_complete", "lmrec_acp"),
        &["public", "transcript.raw"],
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

    let wrong_workspace_primary = extension_result_for(
        "leaven/workspace.read_file",
        &workspace_diff_primary(),
        &query_receipt("qrec_workspace_file"),
        &["workspace.file"],
    );
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&wrong_workspace_primary)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
            | PublicSeamError::InvalidPlanResult { .. }
            | PublicSeamError::InvalidScope { .. }
    ));

    assert_workspace_release_extension_result_negatives(&package);

    let mut unbound_primary = agent_session_primary();
    unbound_primary["receipt"] = json!("agentrec_acp");
    let unbound_receipt = extension_result_for(
        "leaven/agent.run",
        &unbound_primary,
        &call_receipt("agent_run", "other_agentrec"),
        &["public", "transcript.raw"],
    );
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

fn assert_workspace_release_extension_result_negatives(package: &PublicSeamPackage) {
    let wrong_release_primary = extension_result_for(
        "leaven/workspace.release",
        &extension_primary("workspace.release"),
        &call_receipt("workspace_release", "wrec_release"),
        &["workspace.file"],
    );
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&wrong_release_primary)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
            | PublicSeamError::InvalidPlanResult { .. }
            | PublicSeamError::InvalidScope { .. }
    ));

    let mut unreleased_primary = workspace_handle_primary();
    unreleased_primary["receipt"] = json!("wrec_release");
    let unreleased_release_primary = extension_result_for(
        "leaven/workspace.release",
        &unreleased_primary,
        &call_receipt("workspace_release", "wrec_release"),
        &["workspace.file"],
    );
    let error = package
        .validate_acp_extension_result_document(&unreleased_release_primary)
        .unwrap_err();
    assert!(
        error.to_string().contains("released workspace_handle"),
        "unexpected release primary error: {error:?}"
    );
}

#[test]
fn acp_extension_results_reject_forged_result_hashes_for_extension_and_receiptless_primaries() {
    let package = package();

    let wrong_extension_op = extension_result_for(
        "leaven/graph.query",
        &extension_primary("case.target"),
        &query_receipt("qrec_graph"),
        &["public"],
    );
    let error = package
        .validate_acp_extension_result_document(&wrong_extension_op)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("must return extension op `graph.query`"),
        "unexpected wrong extension op error: {error:?}"
    );

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

fn acp_plan_params() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_acp_jsonrpc",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "let",
                "name": "input",
                "expr": {
                    "kind": "literal",
                    "value": "hello",
                    "data_classes": ["public"]
                }
            }
        ],
        "return": ["input"],
        "commit": {
            "kind": "no_graph_writes"
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
            released_workspace_handle_primary(),
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

fn plan_error(receipt: &str, code: &str, message: &str) -> Value {
    json!({
        "code": code,
        "message": message,
        "receipt": receipt
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
            "role": "proposer"
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

fn push_receipt_bound_to_primary(result: &mut Value, mut receipt: Value) {
    let schema_version = match receipt["kind"].as_str().unwrap() {
        "query" => "leaven.plan_query_result.v1",
        "call" => "leaven.plan_call_result.v1",
        "write" => "leaven.plan_write_result.v1",
        other => panic!("unexpected receipt kind {other}"),
    };
    let op_name = receipt["op_var"].as_str().unwrap_or("primary");
    receipt["result_hash"] = json!(prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": schema_version,
            "name": op_name,
            "value": result["primary"]
        }),
    ));
    result["receipts"].as_array_mut().unwrap().push(receipt);
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
        "cost": {"usd_micro": 42, "lm_calls": 1},
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

fn released_workspace_handle_primary() -> Value {
    let mut primary = workspace_handle_primary();
    primary["released"] = json!(true);
    primary["receipt"] = json!("wrec_release");
    primary
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
        "transcript_ref": acp_blob_ref("blob_agent_transcript", &["transcript.raw"]),
        "commands": [{
            "argv": ["codex"],
            "status": "completed",
            "receipt": "agentrec_acp",
            "stdout_ref": acp_blob_ref("blob_agent_stdout", &["transcript.raw"]),
            "stderr_ref": acp_blob_ref("blob_agent_stderr", &["transcript.raw"])
        }],
        "cost": {"usd_micro": 1000, "agent_calls": 1},
        "graph_revision": "rev_acp",
        "data_classes": ["public", "transcript.raw"],
        "replayability": "fully_managed",
        "receipt": "agentrec_acp"
    })
}

fn agent_session_primary_without(field: &str) -> Value {
    let mut primary = agent_session_primary();
    primary.as_object_mut().unwrap().remove(field);
    primary
}

fn agent_session_primary_with_command(command: Value) -> Value {
    let mut primary = agent_session_primary();
    primary["commands"] = Value::Array(vec![command]);
    primary
}

fn sandbox_exec_primary() -> Value {
    json!({
        "kind": "sandbox_exec",
        "status": "completed",
        "exit_code": 0,
        "cost": {"usd_micro": 10, "sandbox_calls": 1},
        "stdout_ref": acp_blob_ref("blob_sandbox_stdout", &["public"]),
        "stderr_ref": acp_blob_ref("blob_sandbox_stderr", &["public"]),
        "graph_revision": "rev_acp",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "execrec_acp"
    })
}

fn sandbox_exec_primary_without(field: &str) -> Value {
    let mut primary = sandbox_exec_primary();
    primary.as_object_mut().unwrap().remove(field);
    primary
}

fn sandbox_exec_primary_with_file(path: &str) -> Value {
    let mut primary = sandbox_exec_primary();
    primary["data_classes"] = json!(["public", "workspace.file"]);
    primary["files"] = json!({
        path: acp_blob_ref("blob_sandbox_file", &["workspace.file"])
    });
    primary
}

fn acp_blob_ref(id: &str, data_classes: &[&str]) -> Value {
    json!({
        "kind": "blob_ref",
        "id": id,
        "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "bytes": 32,
        "data_classes": data_classes
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
    let mut value = json!({
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
    });
    match call_kind {
        "lm_complete" => value["cost"] = json!({"usd_micro": 42, "lm_calls": 1}),
        "agent_run" => value["cost"] = json!({"usd_micro": 1000, "agent_calls": 1}),
        "sandbox_exec" => value["cost"] = json!({"usd_micro": 10, "sandbox_calls": 1}),
        _ => {}
    }
    value
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
