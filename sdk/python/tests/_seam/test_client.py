"""Tests for `leaven._seam.client` typed request execution."""

import subprocess
from pathlib import Path

import msgspec
from _pytest.monkeypatch import MonkeyPatch

from leaven._seam import AgentRunRequest, SeamClient, SeamExecutionContext, SeamServiceConfig
from leaven._seam._wire.jsonrpc import JsonRpcRequestEnvelope


def test_client_lowers_typed_request_to_locked_json_rpc(monkeypatch: MonkeyPatch) -> None:
    """SeamClient accepts request records, not caller-assembled JSON-RPC dicts."""

    captured = CapturedRun()
    monkeypatch.setattr(subprocess, "run", captured.run)
    client = SeamClient(
        config=SeamServiceConfig(
            context=SeamExecutionContext(
                capability_fingerprint="fp_cap_sha256_test",
                policy_fingerprint="fp_policy_sha256_test",
                base_revision="run_base",
            )
        ),
        leaven_bin=Path("/does/not/exist/leaven"),
        repo_root=Path("/does/not/exist/repo"),
    )

    result = client.agent_run(
        AgentRunRequest(
            request_id="req_agent",
            plan_id="plan_agent",
            candidate="cand_agent",
            workspace="ws_agent",
            instructions={"task": "answer"},
            idempotency_prefix="agent-test",
        )
    )

    envelope = msgspec.json.decode(captured.input_text.encode(), type=JsonRpcRequestEnvelope)
    assert envelope.method == "leaven/agent.run"
    assert envelope.id == "req_agent"
    assert result.primary.receipt == "agentrec_test"


class CapturedRun:
    def __init__(self) -> None:
        self.input_text = ""

    def run(
        self,
        args: list[str],
        *,
        input: str,
        text: bool,
        capture_output: bool,
        timeout: int,
        check: bool,
    ) -> subprocess.CompletedProcess[str]:
        self.input_text = input.rstrip("\n")
        _ = (args, text, capture_output, timeout, check)
        return subprocess.CompletedProcess(
            args,
            0,
            stdout=(
                '{"jsonrpc":"2.0","id":"req_agent","result":'
                '{"method":"leaven/agent.run","primary":'
                '{"kind":"agent_session","status":"completed","receipt":"agentrec_test",'
                '"commands":[{"argv":["codex"],"status":"completed"}]},'
                '"receipts":[],"redactions":[],"capability_fingerprint":"fp_cap_test",'
                '"policy_fingerprint":"fp_policy_test","data_classes":["public"]}}'
            ),
            stderr="",
        )


__all__ = []
