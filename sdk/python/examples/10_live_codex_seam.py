"""Example 10 — live-gated Python client for `leaven/agent.run`.

This is a proof of the public seam substrate through `AgentBuilder.run`, but
not yet through an engine-supplied `cx.agent` inside `lv.optimize(...).run()`.
It spawns `leaven seam serve --stdio --config`, lowers one locked
`leaven/agent.run` Plan IR request, and checks that the Rust child materializes
a workspace, runs the configured Codex CLI adapter, and returns an
`AgentSession` with receipts and transcript refs.

Run only when live Codex spend is intended:

    LEAVEN_LIVE_CODEX=1 uv run python examples/10_live_codex_seam.py

Set `LEAVEN_BIN` or `LEAVEN_CODEX_BIN` to override binary discovery.
"""

import asyncio
import os

from leaven._handles import WorkspaceHandle
from leaven._receipts import CallReceipt
from leaven._seam import (
    CodexCliRuntimeConfig,
    LocalWorkspaceConfig,
    SeamClient,
    SeamExecutionContext,
    SeamServiceConfig,
    effect_capability,
    resolve_codex_binary,
)
from leaven.agent_instructions import AgentInstructions
from leaven.builders.agent import AgentBuilder, AgentSession


def main() -> None:
    if os.environ.get("LEAVEN_LIVE_CODEX") != "1":
        print("skipped: set LEAVEN_LIVE_CODEX=1 to run the live Codex seam proof")
        return

    codex = resolve_codex_binary()
    client = SeamClient(config=_service_config(codex))
    agent = AgentBuilder._for_seam(
        client,
        candidate_id="cand_pylivecodex",
        idempotency_prefix="py-live-codex",
        plan_id="planpylivecodex001",
    )
    session = _run(agent)

    assert session.transcript_ref == "blob_completion_transcript"
    assert session.receipt.receipt_id == "agentrec_completion"
    assert session.commands[1]["status"] == "completed"

    codex_command = session.commands[1]["argv"]
    print("agent receipt:    ", session.receipt.receipt_id)
    print("codex model:      ", codex_command[codex_command.index("--model") + 1])
    print("transcript ref:   ", session.transcript_ref)
    print("commands:         ", len(session.commands))


def _run(agent: AgentBuilder) -> AgentSession:
    return asyncio.run(
        agent.run(
            workspace=WorkspaceHandle(
                workspace_id="ws_pylivecodex_materialized",
                candidate_id="cand_pylivecodex",
                receipt=CallReceipt(receipt_id="wrec_workspace"),
                lifetime="manual",
            ),
            instructions=AgentInstructions(
                system=(
                    "You are running inside a temporary Leaven proof workspace. "
                    "Do not edit files or run tools unless absolutely necessary."
                ),
                task=(
                    "Return exactly this sentence as the final answer: "
                    "Leaven Python live Codex seam proof succeeded."
                ),
            ),
        )
    )


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


if __name__ == "__main__":
    main()
