"""Plan IR request construction for private public-seam clients."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class AgentRunRequest:
    """A single public-seam `leaven/agent.run` Plan request."""

    request_id: str
    plan_id: str
    candidate: str
    workspace: str
    instructions: dict[str, str]
    idempotency_prefix: str
    runtime: str = "codex-cli"
    timeout_s: int = 180
    max_turns: int = 1
    max_usd_micro: int = 5_000_000
    output: dict[str, Any] | None = None
    allowed_commands: Sequence[str] | None = None
    input_classes: Sequence[str] | None = None

    def to_json_rpc(self) -> dict[str, Any]:
        """Return a JSON-RPC request for `leaven/agent.run`."""
        return {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "leaven/agent.run",
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": self.plan_id,
                "consistency": {"kind": "latest_at_start"},
                "mode": {"kind": "execute"},
                "ops": [self._workspace_call(), self._agent_call()],
                "return": ["workspace", "completion"],
                "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"},
            },
        }

    def _workspace_call(self) -> dict[str, Any]:
        return {
            "kind": "call",
            "name": "workspace",
            "idempotency_key": f"{self.idempotency_prefix}-workspace",
            "call": {
                "kind": "workspace_materialize",
                "candidate": self.candidate,
                "surface": "program",
                "mode": "copy_on_write",
                "lifetime": "manual_release",
            },
        }

    def _agent_call(self) -> dict[str, Any]:
        tool_policy: dict[str, Any] = {"allow_shell": False}
        if self.allowed_commands is not None:
            tool_policy["allowed_commands"] = list(self.allowed_commands)
        return {
            "kind": "call",
            "name": "completion",
            "deps": ["workspace"],
            "idempotency_key": f"{self.idempotency_prefix}-agent",
            "call": {
                "kind": "agent_run",
                "runtime": self.runtime,
                "workspace": self.workspace,
                "instructions": self.instructions,
                "tool_policy": tool_policy,
                "output": self.output or {"kind": "final_message", "max_bytes": 512},
                "limits": {
                    "timeout_s": self.timeout_s,
                    "max_turns": self.max_turns,
                    "max_usd_micro": self.max_usd_micro,
                },
                "input_classes": list(self.input_classes or ["public"]),
            },
        }


@dataclass(frozen=True)
class LmCompleteRequest:
    """A single public-seam `leaven/lm.complete` Plan request."""

    request_id: str
    plan_id: str
    idempotency_key: str
    messages: Sequence[dict[str, Any]]
    model: str
    model_role: str | None = None
    temperature: float | None = None
    max_tokens: int | None = None
    stop: Sequence[str] | None = None
    output: dict[str, Any] | None = None
    input_classes: Sequence[str] | None = None

    def to_json_rpc(self) -> dict[str, Any]:
        """Return a JSON-RPC request for `leaven/lm.complete`."""
        return {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "leaven/lm.complete",
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": self.plan_id,
                "consistency": {"kind": "latest_at_start"},
                "mode": {"kind": "execute"},
                "ops": [self._lm_call()],
                "return": ["completion"],
                "commit": {"kind": "no_graph_writes"},
            },
        }

    def _lm_call(self) -> dict[str, Any]:
        call: dict[str, Any] = {
            "kind": "lm_complete",
            "purpose": "python.sdk",
            "model": self.model,
            "messages": list(self.messages),
            "output": self.output or {"kind": "final_message", "max_bytes": 512},
            "input_classes": list(self.input_classes or ["public"]),
        }
        if self.model_role is not None:
            call["model_role"] = self.model_role
        sampling: dict[str, Any] = {}
        if self.temperature is not None:
            sampling["temperature"] = self.temperature
        if self.max_tokens is not None:
            sampling["max_output_tokens"] = self.max_tokens
        if self.stop is not None:
            sampling["stop"] = list(self.stop)
        if sampling:
            call["sampling"] = sampling
        return {
            "kind": "call",
            "name": "completion",
            "idempotency_key": self.idempotency_key,
            "call": call,
        }


@dataclass(frozen=True)
class StageRunRequest:
    """A single public-seam `leaven/stage.run` runner dispatch request."""

    request_id: str
    run_id: str
    stage_call_id: str
    candidate: str
    case: str
    case_input: dict[str, Any]

    def to_json_rpc(self) -> dict[str, Any]:
        """Return a JSON-RPC request for `leaven/stage.run`."""
        return {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "leaven/stage.run",
            "params": {
                "schema_version": "leaven.stage_run.v1",
                "message": "stage_run_request",
                "stage": "runner",
                "payload": {
                    "schema_version": "leaven.stage_payloads.v1",
                    "role": "runner",
                    "run": self.run_id,
                    "stage_call_id": self.stage_call_id,
                    "candidate": self.candidate,
                    "case": self.case,
                    "case_input": self.case_input,
                    "target_forbidden": True,
                },
            },
        }


@dataclass(frozen=True)
class StageRunProposeRequest:
    """A single public-seam `leaven/stage.run` proposer dispatch request."""

    request_id: str
    run_id: str
    stage_call_id: str
    base_revision: str
    parent: str
    surface_fingerprint: str
    change_schema: str
    capability_fingerprint: str
    query_policy_fingerprint: str
    reflection_summary: str

    def to_json_rpc(self) -> dict[str, Any]:
        """Return a JSON-RPC request for `leaven/stage.run`."""
        return {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "leaven/stage.run",
            "params": {
                "schema_version": "leaven.stage_run.v1",
                "message": "stage_run_request",
                "stage": "proposer",
                "payload": {
                    "schema_version": "leaven.stage_payloads.v1",
                    "role": "proposer",
                    "run": self.run_id,
                    "stage_call_id": self.stage_call_id,
                    "base_revision": self.base_revision,
                    "parent": self.parent,
                    "surface_fingerprint": self.surface_fingerprint,
                    "reflection_result": self._reflection_result(),
                    "allowed_effects": ["change"],
                    "allowed_change_schemas": [self.change_schema],
                    "source_refs": [self.parent],
                    "query_policy_fingerprint": self.query_policy_fingerprint,
                    "capability_fingerprint": self.capability_fingerprint,
                },
            },
        }

    def _reflection_result(self) -> dict[str, Any]:
        return {
            "schema_version": "leaven.stage_payloads.v1",
            "role": "reflection_result",
            "summary": self.reflection_summary,
            "failure_modes": [
                {
                    "label": "seed_assessment_feedback",
                    "description": self.reflection_summary,
                    "source_refs": [self.parent],
                }
            ],
            "surface_suggestions": [],
            "negative_constraints": [],
            "positive_constraints": [],
            "source_refs": [self.parent],
            "read_receipts": ["qrec_python_seed_assessment"],
            "data_classes": ["optimizer.visible"],
            "confidence": 0.5,
        }


@dataclass(frozen=True)
class ProposalSubmitRequest:
    """A single public-seam `leaven/proposal.submit_batch` Plan request."""

    request_id: str
    plan_id: str
    idempotency_key: str
    proposals: Sequence[dict[str, Any]]

    def to_json_rpc(self) -> dict[str, Any]:
        """Return a JSON-RPC request for `leaven/proposal.submit_batch`."""
        return {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "leaven/proposal.submit_batch",
            "params": {
                "schema_version": "leaven.plan.v1",
                "plan_id": self.plan_id,
                "consistency": {"kind": "latest_at_start"},
                "mode": {"kind": "execute"},
                "ops": [self._submit_write()],
                "return": ["proposal_batch"],
                "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"},
            },
        }

    def _submit_write(self) -> dict[str, Any]:
        return {
            "kind": "write",
            "name": "proposal_batch",
            "idempotency_key": self.idempotency_key,
            "write": {
                "kind": "submit_proposal_batch",
                "semantics": "sequence",
                "proposals": list(self.proposals),
            },
        }


__all__ = [
    "AgentRunRequest",
    "LmCompleteRequest",
    "ProposalSubmitRequest",
    "StageRunProposeRequest",
    "StageRunRequest",
]
