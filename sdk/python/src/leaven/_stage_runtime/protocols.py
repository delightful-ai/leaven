"""Private protocols implemented by Python stage runtime drivers."""

from __future__ import annotations

from typing import Protocol


class LmCompleteCallback(Protocol):
    """Driver capability required by callback-backed `cx.lm.complete`."""

    async def lm_complete(self, prompt: str, *, request_id: str) -> str:
        """Complete one prompt through the stage driver's active seam."""
        ...


__all__ = ["LmCompleteCallback"]
