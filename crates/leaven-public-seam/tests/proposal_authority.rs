use leaven_public_seam::{CapabilityDocument, PublicSeamError, PublicSeamPackage};
use serde_json::{Value, json};

#[test]
fn proposal_authority_accepts_effects_surfaces_schemas_and_apply_grants() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let capability = CapabilityDocument::from_value(proposal_capability()).unwrap();
    let report = package
        .validate_proposal_authority_document(&proposal_authority_plan(), &capability)
        .unwrap();

    assert_eq!(report.submit_batches(), 1);
    assert_eq!(report.apply_writes(), 1);
    assert_eq!(report.change_effects(), 1);
    assert_eq!(report.workspace_diff_effects(), 1);
    assert_eq!(report.agent_session_effects(), 1);
}

#[test]
fn proposal_authority_rejects_submit_only_apply_and_ungranted_surface_or_effect() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let submit_only = CapabilityDocument::from_value(submit_only_capability()).unwrap();
    assert!(matches!(
        package
            .validate_proposal_authority_document(&proposal_authority_plan(), &submit_only)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));

    let capability = CapabilityDocument::from_value(proposal_capability()).unwrap();
    let mut wrong_surface = proposal_authority_plan();
    wrong_surface["ops"][0]["write"]["proposals"][0]["effect"]["surface_fingerprint"] =
        json!("fp_surface_sha256_other");
    assert!(matches!(
        package
            .validate_proposal_authority_document(&wrong_surface, &capability)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));

    let mut wrong_effect = proposal_authority_plan();
    wrong_effect["ops"][0]["write"]["proposals"]
        .as_array_mut()
        .unwrap()[0]["effect"] = json!({
        "kind": "create",
        "artifact_type": "answer",
        "artifact_schema": "fp_schema_sha256_change",
        "artifact": {
            "kind": "literal",
            "value": "ok"
        }
    });
    assert!(matches!(
        package
            .validate_proposal_authority_document(&wrong_effect, &capability)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));
}

#[test]
fn proposal_authority_rejects_schema_valid_change_without_granted_schema() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let capability = CapabilityDocument::from_value(proposal_capability()).unwrap();
    let mut wrong_schema = proposal_authority_plan();
    wrong_schema["ops"][0]["write"]["proposals"][1]["effect"]["change_schema"] =
        json!("fp_schema_sha256_other");

    assert!(matches!(
        package
            .validate_proposal_authority_document(&wrong_schema, &capability)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));
}

fn proposal_authority_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "proposalauth001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "write",
                "name": "proposal_batch",
                "idempotency_key": "proposal-auth-0001",
                "write": {
                    "kind": "submit_proposal_batch",
                    "semantics": "sequence",
                    "proposals": [
                        change_effect("cand_proposal_parent", "change"),
                        workspace_diff_effect("cand_proposal_parent"),
                        agent_session_effect("cand_proposal_parent")
                    ]
                }
            },
            {
                "kind": "write",
                "name": "applied",
                "idempotency_key": "proposal-auth-0002",
                "write": {
                    "kind": "apply_proposal_batch",
                    "proposal_batch": "pb_proposal_authority",
                    "policy": "apply_first_valid"
                }
            }
        ],
        "return": ["applied"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn change_effect(target: &str, effect_kind: &str) -> Value {
    json!({
        "effect": {
            "kind": effect_kind,
            "target": target,
            "surface_fingerprint": "fp_surface_sha256_allowed",
            "change_schema": "fp_schema_sha256_change",
            "change": {
                "kind": "literal",
                "value": {"patch": "change"}
            }
        },
        "causal": {
            "inputs": ["cand_proposal_parent"]
        },
        "informed_by": {
            "kind": "literal",
            "value": ["qrec_proposal_lineage"]
        },
        "read_receipts": ["qrec_proposal_lineage"]
    })
}

fn workspace_diff_effect(target: &str) -> Value {
    json!({
        "effect": {
            "kind": "change_from_workspace_diff",
            "target": target,
            "workspace": "ws_proposal_authority",
            "roots": ["src"],
            "parser": "leaven.diff.git.v1",
            "surface_fingerprint": "fp_surface_sha256_allowed",
            "change_schema": "fp_schema_sha256_change"
        },
        "causal": {
            "inputs": ["cand_proposal_parent"]
        },
        "informed_by": {
            "kind": "literal",
            "value": ["wsread_proposal_diff"]
        },
        "read_receipts": ["wsread_proposal_diff"]
    })
}

fn agent_session_effect(target: &str) -> Value {
    json!({
        "effect": {
            "kind": "change_from_agent_session",
            "target": target,
            "agent_receipt": "agentrec_proposal_session",
            "parser": "leaven.agent.patch.v1",
            "surface_fingerprint": "fp_surface_sha256_allowed",
            "change_schema": "fp_schema_sha256_change"
        },
        "causal": {
            "inputs": ["cand_proposal_parent"]
        },
        "informed_by": {
            "kind": "literal",
            "value": ["agentrec_proposal_session"]
        },
        "read_receipts": ["agentrec_proposal_session"]
    })
}

fn proposal_capability() -> Value {
    let mut value = base_capability();
    value["grants"] = json!([
        {
            "action": "proposal.submit_batch",
            "resource": {},
            "constraints": {
                "effects": [
                    "change",
                    "change_from_workspace_diff",
                    "change_from_agent_session"
                ],
                "allowed_surfaces": ["fp_surface_sha256_allowed"],
                "change_schemas": ["fp_schema_sha256_change"]
            }
        },
        {
            "action": "proposal.apply_batch",
            "resource": {},
            "constraints": {
                "may_apply": true
            }
        }
    ]);
    value
}

fn submit_only_capability() -> Value {
    let mut value = proposal_capability();
    value["grants"].as_array_mut().unwrap().pop();
    value
}

fn base_capability() -> Value {
    json!({
        "schema_version": "leaven.capability.v1",
        "jti": "jti_proposal_authority",
        "capability_fingerprint": "fp_cap_sha256_proposalauthority",
        "policy_fingerprint": "fp_policy_sha256_proposalauthority",
        "subject_fingerprint": "fp_subject_sha256_proposalauthority",
        "issuer": {
            "kind": "run_engine",
            "id": "engine_local"
        },
        "subject": {
            "kind": "stage_call",
            "run": "run_demo",
            "stage_call_id": "sc_proposal_authority",
            "role": "proposer"
        },
        "audience": ["leaven.acp.worker"],
        "issued_at": "2026-05-23T00:00:00Z",
        "expires_at": "2026-05-23T00:20:00Z",
        "expiry_behavior": "drain_inflight_no_new_ops",
        "token_binding": {
            "kind": "opaque_lookup",
            "token_id": "ltok_proposal_authority"
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

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}
