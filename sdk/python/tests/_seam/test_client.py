"""Tests for `leaven._seam.client` typed request execution."""

import subprocess
from pathlib import Path

import msgspec
from _pytest.monkeypatch import MonkeyPatch

from leaven._seam import (
    AgentRunRequest,
    SeamClient,
    SeamExecutionContext,
    SeamServiceConfig,
    StageRunProposeRequest,
    StageRunRequest,
)
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


def test_client_lowers_typed_runner_stage_request(monkeypatch: MonkeyPatch) -> None:
    """Runner stage dispatch stays typed until the JSON-RPC transport envelope."""

    captured = CapturedRun(
        stdout=(
            '{"jsonrpc":"2.0","id":"req_stage","result":'
            '{"schema_version":"leaven.stage_run.v1","message":"stage_run_result",'
            '"stage":"runner","stage_call_id":"sc_stage",'
            '"output":{"kind":"text","visibility":"public","data_classes":["public"],'
            '"summary":"ok","value":"ok"}}}'
        )
    )
    monkeypatch.setattr(subprocess, "run", captured.run)
    client = _client()

    result = client.stage_run(
        StageRunRequest(
            request_id="req_stage",
            run_id="run_stage",
            stage_call_id="sc_stage",
            candidate="cand_seed",
            case="case_1",
            case_input={"prompt": "hi"},
        )
    )

    envelope = msgspec.json.decode(captured.input_text.encode(), type=JsonRpcRequestEnvelope)
    assert envelope.method == "leaven/stage.run"
    assert envelope.id == "req_stage"
    assert result.stage == "runner"
    assert result.output.summary == "ok"


def test_client_lowers_typed_proposer_stage_request(monkeypatch: MonkeyPatch) -> None:
    """Proposer stage dispatch does not use caller-assembled JSON-RPC dicts."""

    captured = CapturedRun(
        stdout=(
            '{"jsonrpc":"2.0","id":"req_propose","result":'
            '{"schema_version":"leaven.stage_run.v1","message":"stage_run_result",'
            '"stage":"proposer","stage_call_id":"sc_proposer",'
            '"output":{"kind":"text","visibility":"public","data_classes":["public"],'
            '"summary":"proposed"},'
            '"proposal_receipts":[{"method":"leaven/proposal.submit_batch",'
            '"receipt":"wrec_propose","proposal_ids":["prop_1"]}]}}'
        )
    )
    monkeypatch.setattr(subprocess, "run", captured.run)
    client = _client()

    result = client.stage_propose(
        StageRunProposeRequest(
            request_id="req_propose",
            run_id="run_stage",
            stage_call_id="sc_proposer",
            base_revision="rev_1",
            parent="cand_seed",
            surface_fingerprint="fp_surface",
            change_schema="fp_schema",
            capability_fingerprint="fp_cap",
            query_policy_fingerprint="fp_policy",
            reflection_summary="try a clearer instruction",
        )
    )

    envelope = msgspec.json.decode(captured.input_text.encode(), type=JsonRpcRequestEnvelope)
    assert envelope.method == "leaven/stage.run"
    assert envelope.id == "req_propose"
    assert result.stage == "proposer"
    assert result.proposal_receipts is not msgspec.UNSET
    assert result.proposal_receipts[0].proposal_ids == ["prop_1"]


def _client() -> SeamClient:
    return SeamClient(
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


class CapturedRun:
    def __init__(
        self,
        stdout: str = (
            '{"jsonrpc":"2.0","id":"req_agent","result":'
            '{"method":"leaven/agent.run","primary":'
            '{"kind":"agent_session","status":"completed","receipt":"agentrec_test",'
            '"commands":[{"argv":["codex"],"status":"completed"}]},'
            '"receipts":[],"redactions":[],"capability_fingerprint":"fp_cap_test",'
            '"policy_fingerprint":"fp_policy_test","data_classes":["public"]}}'
        ),
    ) -> None:
        self.input_text = ""
        self.stdout = stdout

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
            stdout=self.stdout,
            stderr="",
        )


__all__ = []
