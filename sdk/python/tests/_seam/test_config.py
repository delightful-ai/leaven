"""Tests for private typed seam service config and capability records."""

import json

from leaven._seam.capability import (
    AgentRunConstraints,
    AgentRunLimits,
    CandidateResource,
    CapabilityDocument,
    ProposalSubmitConstraints,
    WorkspaceResource,
    capability_to_json,
    effect_capability,
    proposer_stage_capability,
)
from leaven._seam.config import (
    CodexCliRuntimeConfig,
    LocalWorkspaceConfig,
    MockLmRuntimeConfig,
    SeamExecutionContext,
    SeamServiceConfig,
)


def test_service_config_encodes_typed_records_at_file_boundary() -> None:
    """Scenario: private service config stays typed until JSON bytes are written."""

    capability = effect_capability(
        capability_fingerprint="fp_cap_sha256_config",
        policy_fingerprint="fp_policy_sha256_config",
        candidate="cand_seed",
        workspace="ws_config",
        jti="cap_config",
        stage_call_id="sc_config",
    )
    config = SeamServiceConfig(
        context=SeamExecutionContext(
            capability_fingerprint="fp_cap_sha256_config",
            policy_fingerprint="fp_policy_sha256_config",
            base_revision="rev_config",
        ),
        capability=capability,
        workspace=LocalWorkspaceConfig(seed_files={"README.md": "hello"}),
        agent=CodexCliRuntimeConfig(codex_bin="/tmp/codex", timeout_s=17),
        lm=MockLmRuntimeConfig(text="ok", input_tokens=3, output_tokens=2),
    )

    wire = config.to_wire()
    assert isinstance(wire.capability, CapabilityDocument)
    assert wire.workspace.seed_files == {"README.md": "hello"}
    assert wire.agent.kind == "codex_cli"
    assert wire.lm.kind == "mock"

    encoded = config.to_json_bytes()
    decoded = json.loads(encoded)
    assert decoded["context"]["base_revision"] == "rev_config"
    assert decoded["capability"]["schema_version"] == "leaven.capability.v1"
    assert decoded["workspace"]["seed_files"] == {"README.md": "hello"}
    assert decoded["agent"]["codex_bin"] == "/tmp/codex"
    assert decoded["lm"]["responses"] == [
        {"text": "ok", "input_tokens": 3, "output_tokens": 2}
    ]
    assert decoded["stage"] == {"kind": "none"}


def test_effect_capability_uses_typed_grant_resources_constraints_and_limits() -> None:
    """Example: effect capability grants are not anonymous JSON bags."""

    capability = effect_capability(
        capability_fingerprint="fp_cap_sha256_effect",
        policy_fingerprint="fp_policy_sha256_effect",
        candidate="cand_effect",
        workspace="ws_effect",
        jti="cap_effect",
        stage_call_id="sc_effect",
    )

    assert isinstance(capability, CapabilityDocument)
    assert [grant.action for grant in capability.grants] == [
        "lm.complete",
        "workspace.materialize",
        "agent.run",
    ]
    assert isinstance(capability.grants[1].resource, CandidateResource)
    assert capability.grants[1].resource.candidate_ids == ["cand_effect"]
    assert isinstance(capability.grants[2].resource, WorkspaceResource)
    assert capability.grants[2].resource.workspace_ids == ["ws_effect"]
    assert isinstance(capability.grants[2].constraints, AgentRunConstraints)
    limits = capability.grants[2].limits
    assert isinstance(limits, AgentRunLimits)
    assert limits.timeout_s == 180


def test_proposer_capability_optionally_carries_agent_grants() -> None:
    """Boundary: submit-only proposer capability does not smuggle agent authority."""

    submit_only = proposer_stage_capability(
        capability_fingerprint="fp_cap_sha256_submit",
        policy_fingerprint="fp_policy_sha256_submit",
        surface_fingerprint="fp_surface",
        change_schema="fp_schema",
        candidate="cand_seed",
        workspace="ws_submit",
        jti="cap_submit",
        stage_call_id="sc_submit",
        allow_agent=False,
    )
    with_agent = proposer_stage_capability(
        capability_fingerprint="fp_cap_sha256_agent",
        policy_fingerprint="fp_policy_sha256_agent",
        surface_fingerprint="fp_surface",
        change_schema="fp_schema",
        candidate="cand_seed",
        workspace="ws_agent",
        jti="cap_agent",
        stage_call_id="sc_agent",
        allow_agent=True,
    )

    assert [grant.action for grant in submit_only.grants] == ["proposal.submit_batch"]
    assert isinstance(submit_only.grants[0].constraints, ProposalSubmitConstraints)
    assert [grant.action for grant in with_agent.grants] == [
        "proposal.submit_batch",
        "workspace.materialize",
        "agent.run",
    ]
    agent_resource = with_agent.grants[2].resource
    assert isinstance(agent_resource, WorkspaceResource)
    assert agent_resource.workspace_ids == ["ws_agent"]
    document = capability_to_json(with_agent)
    assert document["schema_version"] == "leaven.capability.v1"
