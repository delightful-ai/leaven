"""Plan IR request construction for private public-seam clients."""

from __future__ import annotations

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
    max_bytes: int = 512

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
                "tool_policy": {"allow_shell": False},
                "output": {"kind": "final_message", "max_bytes": self.max_bytes},
                "limits": {
                    "timeout_s": self.timeout_s,
                    "max_turns": self.max_turns,
                    "max_usd_micro": self.max_usd_micro,
                },
                "input_classes": ["public"],
            },
        }


__all__ = ["AgentRunRequest"]
