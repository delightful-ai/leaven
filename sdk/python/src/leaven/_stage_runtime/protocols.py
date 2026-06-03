"""Private protocols implemented by Python stage runtime drivers."""

from __future__ import annotations

from typing import Protocol


class AgentRunCallback(Protocol):
    """Driver capability required by callback-backed `cx.agent.run`."""

    def request(self, request: dict) -> dict:
        """Send one JSON-RPC request through the active stage seam."""
        ...


class ProposalSubmitCallback(Protocol):
    """Driver capability required by callback-backed `cx.proposals.submit`."""

    def request(self, request: dict) -> dict:
        """Send one JSON-RPC request through the active stage seam."""
        ...


class LmCompleteCallback(Protocol):
    """Driver capability required by callback-backed `cx.lm.complete`."""

    async def lm_complete(self, prompt: str, *, request_id: str) -> str:
        """Complete one prompt through the stage driver's active seam."""
        ...


__all__ = ["AgentRunCallback", "LmCompleteCallback", "ProposalSubmitCallback"]
