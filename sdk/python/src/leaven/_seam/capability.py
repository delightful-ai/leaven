"""Capability document helpers for private public-seam proofs."""

from typing import Literal

import msgspec
from msgspec import UNSET, Struct, UnsetType

from ._wire import JsonObject
from ._wire.json_value import json_object


class CapabilityIssuer(Struct, frozen=True, forbid_unknown_fields=True):
    """Capability issuer identity."""

    kind: Literal["run_engine"]
    id: str


class CapabilitySubject(Struct, frozen=True, forbid_unknown_fields=True):
    """Stage-call subject bound to the capability."""

    kind: Literal["stage_call"]
    run: str
    stage_call_id: str
    role: Literal["scorer", "proposer"]


class OpaqueTokenBinding(Struct, frozen=True, forbid_unknown_fields=True):
    """Opaque token binding used by the service capability registry."""

    kind: Literal["opaque_lookup"]
    token_id: str


class IssuerEpochRevocation(Struct, frozen=True, forbid_unknown_fields=True):
    """Issuer-epoch revocation policy."""

    mode: Literal["issuer_epoch"]
    revocation_epoch: int
    check: Literal["on_every_request"]


class RenewBeforeExpiry(Struct, frozen=True, forbid_unknown_fields=True):
    """Non-renewing proof capability renewal policy."""

    mode: Literal["renew_before_expiry"]
    max_extensions: int
    max_total_lifetime_s: int


class EmptyResource(Struct, frozen=True, forbid_unknown_fields=True):
    """Capability grant with no resource selector."""


class CandidateResource(Struct, frozen=True, forbid_unknown_fields=True):
    """Candidate-scoped capability grant resource."""

    candidate_ids: list[str]


class WorkspaceResource(Struct, frozen=True, forbid_unknown_fields=True):
    """Workspace-scoped capability grant resource."""

    workspace_ids: list[str]


type CapabilityResource = EmptyResource | CandidateResource | WorkspaceResource


class LmCompleteConstraints(Struct, frozen=True, forbid_unknown_fields=True):
    """Constraints for `lm.complete` proof grants."""

    allowed_input_classes: list[str]
    purposes: list[str]
    models: list[str]
    model_roles: list[str]


class WorkspaceMaterializeConstraints(Struct, frozen=True, forbid_unknown_fields=True):
    """Constraints for `workspace.materialize` proof grants."""

    workspace_ops: list[Literal["materialize"]]


class AgentRunConstraints(Struct, frozen=True, forbid_unknown_fields=True):
    """Constraints for `agent.run` proof grants."""

    allowed_input_classes: list[str]


class ProposalSubmitConstraints(Struct, frozen=True, forbid_unknown_fields=True):
    """Constraints for `proposal.submit_batch` proof grants."""

    effects: list[Literal["change", "change_from_agent_session"]]
    allowed_surfaces: list[str]
    change_schemas: list[str]


type CapabilityConstraints = (
    LmCompleteConstraints
    | WorkspaceMaterializeConstraints
    | AgentRunConstraints
    | ProposalSubmitConstraints
)


class AgentRunLimits(Struct, frozen=True, forbid_unknown_fields=True):
    """Agent-run grant limits used by live proof configs."""

    timeout_s: int
    max_usd_micro: int


class CapabilityGrant(Struct, frozen=True, forbid_unknown_fields=True):
    """One typed capability grant."""

    action: Literal["lm.complete", "workspace.materialize", "agent.run", "proposal.submit_batch"]
    resource: CapabilityResource
    constraints: CapabilityConstraints
    limits: AgentRunLimits | UnsetType = UNSET


class ExecutionPolicy(Struct, frozen=True, forbid_unknown_fields=True):
    """Execution policy lattice carried by proof capabilities."""

    profile: Literal["managed_sandbox"]
    network: Literal["leaven_endpoint_only"]
    subprocess: Literal["deny_except_sandbox_exec"]
    filesystem: Literal["workspace_handles_only"]
    byo_effects: Literal["forbidden"]


class DelegationPolicy(Struct, frozen=True, forbid_unknown_fields=True):
    """Delegation policy for non-delegable proof capabilities."""

    may_delegate: bool
    max_depth: int
    must_attenuate: bool
    allowed_actions: list[str]


class CapabilityDocument(Struct, frozen=True, forbid_unknown_fields=True):
    """Typed capability document emitted by the Python seam SDK."""

    schema_version: Literal["leaven.capability.v1"]
    jti: str
    capability_fingerprint: str
    policy_fingerprint: str
    subject_fingerprint: str
    issuer: CapabilityIssuer
    subject: CapabilitySubject
    audience: list[Literal["leaven.acp.worker"]]
    issued_at: str
    expires_at: str
    expiry_behavior: Literal["drain_inflight_no_new_ops"]
    token_binding: OpaqueTokenBinding
    revocation: IssuerEpochRevocation
    renewal: RenewBeforeExpiry
    grants: list[CapabilityGrant]
    budgets: EmptyResource
    execution_policy: ExecutionPolicy
    delegation: DelegationPolicy


def capability_to_json(capability: CapabilityDocument) -> JsonObject:
    """Project a typed capability document to JSON-compatible builtins."""
    return json_object(msgspec.to_builtins(capability))


def effect_capability(
    *,
    capability_fingerprint: str,
    policy_fingerprint: str,
    candidate: str,
    workspace: str,
    jti: str,
    stage_call_id: str,
) -> CapabilityDocument:
    """Build the current effect capability document for a workspace+agent proof."""
    return CapabilityDocument(
        schema_version="leaven.capability.v1",
        jti=jti,
        capability_fingerprint=capability_fingerprint,
        policy_fingerprint=policy_fingerprint,
        subject_fingerprint=f"fp_subject_sha256_{stage_call_id}",
        issuer=CapabilityIssuer(kind="run_engine", id="engine_local"),
        subject=CapabilitySubject(
            kind="stage_call",
            run=f"run_{stage_call_id}",
            stage_call_id=stage_call_id,
            role="scorer",
        ),
        audience=["leaven.acp.worker"],
        issued_at="2026-06-02T00:00:00Z",
        expires_at="2026-06-02T00:20:00Z",
        expiry_behavior="drain_inflight_no_new_ops",
        token_binding=OpaqueTokenBinding(kind="opaque_lookup", token_id=f"ltok_{stage_call_id}"),
        revocation=IssuerEpochRevocation(
            mode="issuer_epoch",
            revocation_epoch=9,
            check="on_every_request",
        ),
        renewal=RenewBeforeExpiry(
            mode="renew_before_expiry",
            max_extensions=0,
            max_total_lifetime_s=1200,
        ),
        grants=[
            CapabilityGrant(
                action="lm.complete",
                resource=EmptyResource(),
                constraints=LmCompleteConstraints(
                    allowed_input_classes=["public"],
                    purposes=["python.sdk"],
                    models=["gpt-4.1-mini", "gpt-5.4-mini"],
                    model_roles=["reflector", "grader"],
                ),
            ),
            CapabilityGrant(
                action="workspace.materialize",
                resource=CandidateResource(candidate_ids=[candidate]),
                constraints=WorkspaceMaterializeConstraints(workspace_ops=["materialize"]),
            ),
            CapabilityGrant(
                action="agent.run",
                resource=WorkspaceResource(workspace_ids=[workspace]),
                constraints=AgentRunConstraints(allowed_input_classes=["public"]),
                limits=AgentRunLimits(timeout_s=180, max_usd_micro=5_000_000),
            ),
        ],
        budgets=EmptyResource(),
        execution_policy=_execution_policy(),
        delegation=_delegation_policy(),
    )


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
) -> CapabilityDocument:
    """Build a proposer-stage capability document for proposal submission."""
    grants = [
        CapabilityGrant(
            action="proposal.submit_batch",
            resource=EmptyResource(),
            constraints=ProposalSubmitConstraints(
                effects=["change", "change_from_agent_session"],
                allowed_surfaces=[surface_fingerprint],
                change_schemas=[change_schema],
            ),
        )
    ]
    if allow_agent:
        grants.extend(
            [
                CapabilityGrant(
                    action="workspace.materialize",
                    resource=CandidateResource(candidate_ids=[candidate]),
                    constraints=WorkspaceMaterializeConstraints(workspace_ops=["materialize"]),
                ),
                CapabilityGrant(
                    action="agent.run",
                    resource=WorkspaceResource(workspace_ids=[workspace]),
                    constraints=AgentRunConstraints(allowed_input_classes=["public"]),
                    limits=AgentRunLimits(timeout_s=180, max_usd_micro=5_000_000),
                ),
            ]
        )
    return CapabilityDocument(
        schema_version="leaven.capability.v1",
        jti=jti,
        capability_fingerprint=capability_fingerprint,
        policy_fingerprint=policy_fingerprint,
        subject_fingerprint=f"fp_subject_sha256_{stage_call_id}",
        issuer=CapabilityIssuer(kind="run_engine", id="engine_local"),
        subject=CapabilitySubject(
            kind="stage_call",
            run=f"run_{stage_call_id}",
            stage_call_id=stage_call_id,
            role="proposer",
        ),
        audience=["leaven.acp.worker"],
        issued_at="2026-06-03T00:00:00Z",
        expires_at="2026-06-03T00:20:00Z",
        expiry_behavior="drain_inflight_no_new_ops",
        token_binding=OpaqueTokenBinding(kind="opaque_lookup", token_id=f"ltok_{stage_call_id}"),
        revocation=IssuerEpochRevocation(
            mode="issuer_epoch",
            revocation_epoch=9,
            check="on_every_request",
        ),
        renewal=RenewBeforeExpiry(
            mode="renew_before_expiry",
            max_extensions=0,
            max_total_lifetime_s=1200,
        ),
        grants=grants,
        budgets=EmptyResource(),
        execution_policy=_execution_policy(),
        delegation=_delegation_policy(),
    )


def _execution_policy() -> ExecutionPolicy:
    return ExecutionPolicy(
        profile="managed_sandbox",
        network="leaven_endpoint_only",
        subprocess="deny_except_sandbox_exec",
        filesystem="workspace_handles_only",
        byo_effects="forbidden",
    )


def _delegation_policy() -> DelegationPolicy:
    return DelegationPolicy(
        may_delegate=False,
        max_depth=0,
        must_attenuate=True,
        allowed_actions=[],
    )


__all__ = [
    "CapabilityDocument",
    "capability_to_json",
    "effect_capability",
    "proposer_stage_capability",
]
