use crate::support::workspace_root;
use leaven_public_seam::{
    CallAuthorityDenialKind, CallAuthorityError, CapabilityDocument, PublicSeamPackage,
};
use serde_json::{Value, json};

#[test]
fn call_authority_accepts_lm_agent_and_sandbox_input_classes() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let capability = CapabilityDocument::from_value(call_capability()).unwrap();
    let report = package
        .validate_call_authority_document(&call_authority_plan(), &capability)
        .unwrap();

    assert_eq!(report.lm_calls(), 1);
    assert_eq!(report.agent_calls(), 1);
    assert_eq!(report.sandbox_calls(), 1);
    assert_eq!(
        report.checked_input_classes(),
        vec!["case.input", "public", "workspace.file"]
    );
}

#[test]
fn call_authority_rejects_forbidden_input_classes_before_execution() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let capability = CapabilityDocument::from_value(call_capability()).unwrap();
    let mut plan = call_authority_plan();
    plan["ops"][1]["call"]["input_classes"] = json!(["workspace.file", "external.secret"]);

    let denial = call_authority_denial(
        package
            .validate_call_authority_document(&plan, &capability)
            .unwrap_err(),
    );
    assert_eq!(denial.kind(), CallAuthorityDenialKind::DataClass);
    assert_eq!(denial.redactions(), &["external.secret"]);
}

#[test]
fn call_authority_rejects_declared_forbidden_intersections_even_when_grant_allows() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let capability = CapabilityDocument::from_value(call_capability_allowing_target()).unwrap();
    let mut plan = call_authority_plan();
    plan["ops"][0]["call"]["input_classes"] = json!(["case.input", "case.target"]);
    plan["ops"][0]["call"]["forbidden_input_classes"] = json!(["case.target"]);

    let denial = call_authority_denial(
        package
            .validate_call_authority_document(&plan, &capability)
            .unwrap_err(),
    );
    assert_eq!(denial.kind(), CallAuthorityDenialKind::DataClass);
    assert_eq!(denial.redactions(), &["case.target"]);
}

#[test]
fn call_authority_rejects_reflector_lm_call_input_classes_include_case_target() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let capability = CapabilityDocument::from_value(call_capability()).unwrap();
    let mut plan = call_authority_plan();
    plan["ops"][0]["call"]["input_classes"] = json!(["case.input", "case.target"]);
    plan["ops"][0]["call"]["forbidden_input_classes"] = json!([]);

    let error = package
        .validate_call_authority_document(&plan, &capability)
        .unwrap_err();
    let denial = call_authority_denial(error);
    assert_eq!(denial.kind(), CallAuthorityDenialKind::DataClass);
    assert_eq!(denial.redactions(), &["case.target"]);
    assert!(
        denial.message().contains("reflector lm_complete calls"),
        "unexpected denial: {denial:?}"
    );
}

#[test]
fn call_authority_rejects_reflector_model_role_case_target_with_non_reflector_subject() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let capability = CapabilityDocument::from_value(call_capability_allowing_target()).unwrap();
    let mut plan = call_authority_plan();
    plan["ops"][0]["call"]["input_classes"] = json!(["case.input", "case.target"]);
    plan["ops"][0]["call"]["forbidden_input_classes"] = json!([]);

    let denial = call_authority_denial(
        package
            .validate_call_authority_document(&plan, &capability)
            .unwrap_err(),
    );
    assert_eq!(denial.kind(), CallAuthorityDenialKind::DataClass);
    assert_eq!(denial.redactions(), &["case.target"]);
}

#[test]
fn call_authority_rejects_agent_shell_or_network_beyond_execution_policy() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let capability = CapabilityDocument::from_value(call_capability()).unwrap();

    let mut shell_plan = call_authority_plan();
    shell_plan["ops"][1]["call"]["tool_policy"] = json!({
        "allow_shell": true,
        "network": "leaven_endpoint_only"
    });
    let error = package
        .validate_call_authority_document(&shell_plan, &capability)
        .unwrap_err();
    assert!(
        error.to_string().contains("agent_run allow_shell denied"),
        "unexpected error: {error:?}"
    );

    let mut network_plan = call_authority_plan();
    network_plan["ops"][1]["call"]["tool_policy"] = json!({
        "allow_shell": false,
        "network": "unrestricted"
    });
    let error = package
        .validate_call_authority_document(&network_plan, &capability)
        .unwrap_err();
    assert!(
        error.to_string().contains("agent_run network policy"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn call_authority_rejects_sandbox_exec_outside_workspace_execution_policy() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut subprocess_denied = call_capability();
    subprocess_denied["execution_policy"]["subprocess"] = json!("deny");
    let capability = CapabilityDocument::from_value(subprocess_denied).unwrap();
    let error = package
        .validate_call_authority_document(&call_authority_plan(), &capability)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("sandbox_exec denied by execution_policy.subprocess"),
        "unexpected error: {error:?}"
    );

    let mut unrestricted_filesystem = call_capability();
    unrestricted_filesystem["execution_policy"]["filesystem"] = json!("unrestricted");
    let capability = CapabilityDocument::from_value(unrestricted_filesystem).unwrap();
    let error = package
        .validate_call_authority_document(&call_authority_plan(), &capability)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("sandbox_exec requires workspace_handles_only"),
        "unexpected error: {error:?}"
    );

    let mut network_denied = call_capability();
    network_denied["execution_policy"]["network"] = json!("deny");
    let capability = CapabilityDocument::from_value(network_denied).unwrap();
    let error = package
        .validate_call_authority_document(&call_authority_plan(), &capability)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("sandbox_exec denied by execution_policy.network"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn call_authority_rejects_agent_allowed_commands_outside_grants() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut allowed_plan = call_authority_plan();
    allowed_plan["ops"][1]["call"]["tool_policy"] = json!({
        "allow_shell": false,
        "allowed_tools": ["read_file"],
        "allowed_commands": ["python"],
        "network": "deny"
    });
    let mut allowed_capability = call_capability();
    allowed_capability["grants"][1]["constraints"]["allowed_commands"] = json!(["python"]);
    let capability = CapabilityDocument::from_value(allowed_capability).unwrap();
    package
        .validate_call_authority_document(&allowed_plan, &capability)
        .unwrap();

    let mut denied_plan = allowed_plan;
    denied_plan["ops"][1]["call"]["tool_policy"]["allowed_commands"] = json!(["python", "bash"]);
    let error = package
        .validate_call_authority_document(&denied_plan, &capability)
        .unwrap_err();
    assert!(
        error.to_string().contains("allowed_commands"),
        "unexpected error: {error:?}"
    );
}

fn call_authority_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "callauth001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "call",
                "name": "lm",
                "idempotency_key": "call-auth-0001",
                "call": {
                    "kind": "lm_complete",
                    "purpose": "reflection",
                    "model_role": "reflector",
                    "messages": [
                        {
                            "role": "user",
                            "content": [{"kind": "text", "text": "reflect"}]
                        }
                    ],
                    "output": {"kind": "final_message"},
                    "input_classes": ["case.input", "public"]
                }
            },
            {
                "kind": "call",
                "name": "agent",
                "idempotency_key": "call-auth-0002",
                "call": {
                    "kind": "agent_run",
                    "runtime": "codex",
                    "instructions": {"task": "edit target-free workspace files"},
                    "output": {"kind": "final_message"},
                    "input_classes": ["workspace.file", "public"]
                }
            },
            {
                "kind": "call",
                "name": "sandbox",
                "idempotency_key": "call-auth-0003",
                "call": {
                    "kind": "sandbox_exec",
                    "workspace": "ws_call_authority",
                    "argv": ["cargo", "test"],
                    "timeout_s": 30,
                    "output": {"kind": "final_message"},
                    "input_classes": ["workspace.file", "public"]
                }
            }
        ],
        "return": ["lm", "agent", "sandbox"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn call_capability() -> Value {
    let mut value = base_capability();
    value["grants"] = json!([
        {
            "action": "lm.complete",
            "resource": {},
            "constraints": {
                "allowed_input_classes": ["case.input", "public"],
                "forbidden_input_classes": ["case.target"],
                "purposes": ["reflection"],
                "model_roles": ["reflector"]
            }
        },
        call_grant(
            "agent.run",
            &["workspace.file", "public"],
            &["case.target", "external.secret"]
        ),
        sandbox_call_grant(
            &["workspace.file", "public"],
            &["case.target", "external.secret"]
        )
    ]);
    value
}

fn call_capability_allowing_target() -> Value {
    let mut value = base_capability();
    value["subject"]["role"] = json!("proposer");
    value["grants"] = json!([
        {
            "action": "lm.complete",
            "resource": {},
            "constraints": {
                "allowed_input_classes": ["case.input", "case.target", "public"],
                "forbidden_input_classes": [],
                "purposes": ["reflection"],
                "model_roles": ["reflector"]
            }
        },
        call_grant("agent.run", &["workspace.file", "public"], &[]),
        sandbox_call_grant(&["workspace.file", "public"], &[])
    ]);
    value
}

fn call_grant(action: &str, allowed: &[&str], forbidden: &[&str]) -> Value {
    json!({
        "action": action,
        "resource": {},
        "constraints": {
            "allowed_input_classes": allowed,
            "forbidden_input_classes": forbidden
        }
    })
}

fn sandbox_call_grant(allowed: &[&str], forbidden: &[&str]) -> Value {
    json!({
        "action": "sandbox.exec",
        "resource": {
            "workspace_ids": ["ws_call_authority"]
        },
        "constraints": {
            "allowed_input_classes": allowed,
            "forbidden_input_classes": forbidden,
            "workspace_ops": ["exec"],
            "allowed_commands": ["cargo"]
        },
        "limits": {
            "timeout_s": 30
        }
    })
}

fn base_capability() -> Value {
    json!({
        "schema_version": "leaven.capability.v1",
        "jti": "jti_call_authority",
        "capability_fingerprint": "fp_cap_sha256_callauthority",
        "policy_fingerprint": "fp_policy_sha256_callauthority",
        "subject_fingerprint": "fp_subject_sha256_callauthority",
        "issuer": {
            "kind": "run_engine",
            "id": "engine_local"
        },
        "subject": {
            "kind": "stage_call",
            "run": "run_demo",
            "stage_call_id": "sc_call_authority",
            "role": "reflector"
        },
        "audience": ["leaven.acp.worker"],
        "issued_at": "2026-05-23T00:00:00Z",
        "expires_at": "2026-05-23T00:20:00Z",
        "expiry_behavior": "drain_inflight_no_new_ops",
        "token_binding": {
            "kind": "opaque_lookup",
            "token_id": "ltok_call_authority"
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
        "grants": [],
        "delegation": {
            "may_delegate": false,
            "max_depth": 0,
            "must_attenuate": true,
            "allowed_actions": []
        }
    })
}

fn call_authority_denial(error: CallAuthorityError) -> leaven_public_seam::CallAuthorityDenial {
    match error {
        CallAuthorityError::Denied(denial) => denial,
        other @ CallAuthorityError::InvalidPlan(_) => {
            panic!("expected call authority denial, got {other:?}")
        }
    }
}
