"""Capability document helpers for private public-seam proofs."""

from leaven._seam._wire import JsonObject
from leaven._seam._wire.json_value import json_object


def effect_capability(
    *,
    capability_fingerprint: str,
    policy_fingerprint: str,
    candidate: str,
    workspace: str,
    jti: str,
    stage_call_id: str,
) -> JsonObject:
    """Build the current effect capability document for a workspace+agent proof."""
    return json_object({
        "schema_version": "leaven.capability.v1",
        "jti": jti,
        "capability_fingerprint": capability_fingerprint,
        "policy_fingerprint": policy_fingerprint,
        "subject_fingerprint": f"fp_subject_sha256_{stage_call_id}",
        "issuer": {"kind": "run_engine", "id": "engine_local"},
        "subject": {
            "kind": "stage_call",
            "run": f"run_{stage_call_id}",
            "stage_call_id": stage_call_id,
            "role": "scorer",
        },
        "audience": ["leaven.acp.worker"],
        "issued_at": "2026-06-02T00:00:00Z",
        "expires_at": "2026-06-02T00:20:00Z",
        "expiry_behavior": "drain_inflight_no_new_ops",
        "token_binding": {"kind": "opaque_lookup", "token_id": f"ltok_{stage_call_id}"},
        "revocation": {"mode": "issuer_epoch", "revocation_epoch": 9, "check": "on_every_request"},
        "renewal": {
            "mode": "renew_before_expiry",
            "max_extensions": 0,
            "max_total_lifetime_s": 1200,
        },
        "grants": [
            {
                "action": "lm.complete",
                "resource": {},
                "constraints": {
                    "allowed_input_classes": ["public"],
                    "purposes": ["python.sdk"],
                    "models": ["gpt-4.1-mini", "gpt-5.4-mini"],
                    "model_roles": ["reflector", "grader"],
                },
            },
            {
                "action": "workspace.materialize",
                "resource": {"candidate_ids": [candidate]},
                "constraints": {"workspace_ops": ["materialize"]},
            },
            {
                "action": "agent.run",
                "resource": {"workspace_ids": [workspace]},
                "constraints": {"allowed_input_classes": ["public"]},
                "limits": {"timeout_s": 180, "max_usd_micro": 5_000_000},
            },
        ],
        "budgets": {},
        "execution_policy": {
            "profile": "managed_sandbox",
            "network": "leaven_endpoint_only",
            "subprocess": "deny_except_sandbox_exec",
            "filesystem": "workspace_handles_only",
            "byo_effects": "forbidden",
        },
        "delegation": {
            "may_delegate": False,
            "max_depth": 0,
            "must_attenuate": True,
            "allowed_actions": [],
        },
    })


def proposer_stage_capability(
    *,
    capability_fingerprint: str,
    policy_fingerprint: str,
    surface_fingerprint: str,
    change_schema: str,
    candidate: str,
    workspace: str,
    jti: str,
    stage_call_id: str,
    allow_agent: bool,
) -> JsonObject:
    """Build a proposer-stage capability document for proposal submission."""
    grants: list[JsonObject] = [
        {
            "action": "proposal.submit_batch",
            "resource": {},
            "constraints": {
                "effects": ["change", "change_from_agent_session"],
                "allowed_surfaces": [surface_fingerprint],
                "change_schemas": [change_schema],
            },
        }
    ]
    if allow_agent:
        grants.extend(
            [
                {
                    "action": "workspace.materialize",
                    "resource": {"candidate_ids": [candidate]},
                    "constraints": {"workspace_ops": ["materialize"]},
                },
                {
                    "action": "agent.run",
                    "resource": {"workspace_ids": [workspace]},
                    "constraints": {"allowed_input_classes": ["public"]},
                    "limits": {"timeout_s": 180, "max_usd_micro": 5_000_000},
                },
            ]
        )
    return json_object({
        "schema_version": "leaven.capability.v1",
        "jti": jti,
        "capability_fingerprint": capability_fingerprint,
        "policy_fingerprint": policy_fingerprint,
        "subject_fingerprint": f"fp_subject_sha256_{stage_call_id}",
        "issuer": {"kind": "run_engine", "id": "engine_local"},
        "subject": {
            "kind": "stage_call",
            "run": f"run_{stage_call_id}",
            "stage_call_id": stage_call_id,
            "role": "proposer",
        },
        "audience": ["leaven.acp.worker"],
        "issued_at": "2026-06-03T00:00:00Z",
        "expires_at": "2026-06-03T00:20:00Z",
        "expiry_behavior": "drain_inflight_no_new_ops",
        "token_binding": {"kind": "opaque_lookup", "token_id": f"ltok_{stage_call_id}"},
        "revocation": {"mode": "issuer_epoch", "revocation_epoch": 9, "check": "on_every_request"},
        "renewal": {
            "mode": "renew_before_expiry",
            "max_extensions": 0,
            "max_total_lifetime_s": 1200,
        },
        "grants": grants,
        "budgets": {},
        "execution_policy": {
            "profile": "managed_sandbox",
            "network": "leaven_endpoint_only",
            "subprocess": "deny_except_sandbox_exec",
            "filesystem": "workspace_handles_only",
            "byo_effects": "forbidden",
        },
        "delegation": {
            "may_delegate": False,
            "max_depth": 0,
            "must_attenuate": True,
            "allowed_actions": [],
        },
    })


__all__ = ["effect_capability", "proposer_stage_capability"]
