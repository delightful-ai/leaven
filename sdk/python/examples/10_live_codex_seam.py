"""Example 10 — live-gated Python client for `leaven/agent.run`.

This is a proof of the public seam substrate, not the ergonomic `cx.agent.run`
SDK surface. It spawns `leaven seam serve --stdio --config`, sends one locked
Plan IR `leaven/agent.run` request, and checks that the Rust child materializes
a workspace, runs the configured Codex CLI adapter, and returns an
`agent_session` with receipts and transcript refs.

Run only when live Codex spend is intended:

    LEAVEN_LIVE_CODEX=1 uv run python examples/10_live_codex_seam.py

Set `LEAVEN_BIN` or `LEAVEN_CODEX_BIN` to override binary discovery.
"""

from __future__ import annotations

import os

from leaven._seam import (
    AgentRunRequest,
    CodexCliRuntimeConfig,
    LocalWorkspaceConfig,
    SeamClient,
    SeamExecutionContext,
    SeamServiceConfig,
    effect_capability,
    resolve_codex_binary,
)


def main() -> None:
    if os.environ.get("LEAVEN_LIVE_CODEX") != "1":
        print("skipped: set LEAVEN_LIVE_CODEX=1 to run the live Codex seam proof")
        return

    codex = resolve_codex_binary()
    client = SeamClient(config=_service_config(codex))
    result = client.request(_agent_run_request().to_json_rpc())

    primary = result["primary"]
    receipts = result["receipts"]
    assert primary["kind"] == "agent_session"
    assert primary["status"] == "completed"
    assert [receipt["call_kind"] for receipt in receipts] == ["workspace_materialize", "agent_run"]
    assert primary["transcript_ref"]["bytes"] > 100

    codex_command = primary["commands"][1]["argv"]
    print("agent status:     ", primary["status"])
    print("codex model:      ", codex_command[codex_command.index("--model") + 1])
    print("transcript bytes: ", primary["transcript_ref"]["bytes"])
    print("receipts:         ", ", ".join(receipt["receipt"] for receipt in receipts))


def _service_config(codex_bin: str) -> SeamServiceConfig:
    capability_fingerprint = "fp_cap_sha256_py_live_codex"
    policy_fingerprint = "fp_policy_sha256_py_live_codex"
    candidate = "cand_pylivecodex"
    workspace = "ws_pylivecodex_materialized"
    return SeamServiceConfig(
        context=SeamExecutionContext(
            capability_fingerprint=capability_fingerprint,
            policy_fingerprint=policy_fingerprint,
            base_revision="rev_py_live_codex_base",
        ),
        capability=effect_capability(
            capability_fingerprint=capability_fingerprint,
            policy_fingerprint=policy_fingerprint,
            candidate=candidate,
            workspace=workspace,
            jti="jti_py_live_codex",
            stage_call_id="sc_py_live_codex",
        ),
        workspace=LocalWorkspaceConfig(
            seed_files={"README.md": "Live Codex proof workspace for Leaven Python public seam.\n"}
        ),
        agent=CodexCliRuntimeConfig(codex_bin=codex_bin),
    )


def _agent_run_request() -> AgentRunRequest:
    return AgentRunRequest(
        request_id="py-live-codex-agent-1",
        plan_id="planpylivecodex001",
        candidate="cand_pylivecodex",
        workspace="ws_pylivecodex_materialized",
        idempotency_prefix="py-live-codex",
        instructions={
            "system": (
                "You are running inside a temporary Leaven proof workspace. "
                "Do not edit files or run tools unless absolutely necessary."
            ),
            "task": (
                "Return exactly this sentence as the final answer: "
                "Leaven Python live Codex seam proof succeeded."
            ),
        },
    )


if __name__ == "__main__":
    main()
