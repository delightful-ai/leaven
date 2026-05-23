use leaven_public_seam::{
    AcpPermissionRequest, CapabilityDocument, PublicSeamError, PublicSeamPackage,
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
    assert_eq!(profile.extension_methods().len(), 5);
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
}

#[test]
fn acp_permissions_use_capability_grants_and_return_planerror_redactions() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();
    let capability = CapabilityDocument::from_value(acp_capability()).unwrap();

    let allowed = package.authorize_acp_permission(
        &profile,
        &capability,
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
        AcpPermissionRequest::new("leaven/unknown.extension"),
    );
    assert!(!unknown.allowed());
    assert_eq!(unknown.error().unwrap()["code"], json!("extension_error"));
}

#[test]
fn acp_permissions_deny_ungranted_models_workspace_ops_and_commands() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = package
        .validate_acp_profile_document(&acp_profile())
        .unwrap();
    let capability = CapabilityDocument::from_value(acp_capability()).unwrap();

    let workspace_allowed = package.authorize_acp_permission(
        &profile,
        &capability,
        AcpPermissionRequest::new("leaven/workspace.read_file")
            .with_resource("workspace_ids", json!("ws_acp"))
            .with_input_class("workspace.file")
            .with_workspace_op("read_file"),
    );
    assert!(workspace_allowed.allowed());

    let workspace_denied = package.authorize_acp_permission(
        &profile,
        &capability,
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
        AcpPermissionRequest::new("leaven/lm.complete")
            .with_input_class("case.input")
            .with_model("ungranted-model")
            .with_resource("run", json!("run_demo")),
    );
    assert!(!model_denied.allowed());

    let sandbox_denied = package.authorize_acp_permission(
        &profile,
        &capability,
        AcpPermissionRequest::new("leaven/sandbox.exec")
            .with_resource("workspace_ids", json!("ws_acp"))
            .with_input_class("public")
            .with_workspace_op("exec")
            .with_command("python"),
    );
    assert!(!sandbox_denied.allowed());
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
        PublicSeamError::InvalidScope { .. }
    ));

    let mut missing_classes = extension_result();
    missing_classes["data_classes"] = json!([]);
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&missing_classes)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
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
}

#[test]
fn acp_extension_results_bind_worker_methods_to_primary_kinds_and_receipts() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    for (method, primary, receipt) in [
        (
            "leaven/workspace.materialize",
            workspace_handle_primary(),
            call_receipt("workspace_materialize", "wscall_materialize"),
        ),
        (
            "leaven/workspace.read_file",
            workspace_file_primary(),
            query_receipt("qrec_workspace_file"),
        ),
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
            call_receipt("sandbox_exec", "sandboxrec_acp"),
        ),
    ] {
        let result = package
            .validate_acp_extension_result_document(&extension_result_for(
                method,
                &primary,
                &receipt,
                &["workspace.file", "completion.raw", "public"],
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
        PublicSeamError::InvalidScope { .. }
    ));

    let wrong_receipt = extension_result_for(
        "leaven/sandbox.exec",
        &sandbox_exec_primary(),
        &call_receipt("agent_run", "sandboxrec_acp"),
        &["public"],
    );
    assert!(matches!(
        package
            .validate_acp_extension_result_document(&wrong_receipt)
            .unwrap_err(),
        PublicSeamError::InvalidScope { .. }
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
        PublicSeamError::InvalidScope { .. }
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
        "extension_methods": [
            extension_method("leaven/lm.complete", "lm.complete"),
            extension_method("leaven/agent.run", "agent.run"),
            extension_method("leaven/sandbox.exec", "sandbox.exec"),
            extension_method("leaven/workspace.read_file", "workspace.read"),
            extension_method("leaven/proposal.apply", "proposal.apply_batch")
        ],
        "flow_control": {
            "bounded_channel_required": true,
            "default_max_inflight_updates": 32,
            "backpressure": "pause_worker",
            "heartbeat_ms": 1000
        }
    })
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
    json!({
        "method": method,
        "primary": primary,
        "receipts": [receipt],
        "redactions": [],
        "capability_fingerprint": "fp_cap_sha256_acp",
        "data_classes": data_classes
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
        "workspace": "workspace_acp",
        "lifetime": "stage_call",
        "released": false,
        "graph_revision": "rev_acp",
        "data_classes": ["workspace.file"],
        "replayability": "fully_managed",
        "receipt": "wscall_materialize"
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
        "receipt": "sandboxrec_acp"
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

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}
