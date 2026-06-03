"""Role-scoped context bindings for the command-runner worker."""

from __future__ import annotations

from .._stage_runtime import CallbackRolloutContext


class StaticLmCallback:
    """Deterministic LM callback for the checked-in worker dispatch slice."""

    def __init__(self, text: str) -> None:
        self._text = text

    async def lm_complete(self, prompt: str, *, request_id: str) -> str:
        """Return configured text while preserving the callback shape."""
        _ = (prompt, request_id)
        return self._text


def rollout_context(
    *,
    candidate_id: str,
    stage_call_id: str,
    lm_text: str,
) -> CallbackRolloutContext:
    """Build the context passed to a registered runner stage."""
    return CallbackRolloutContext(
        StaticLmCallback(lm_text),
        candidate_id=candidate_id,
        stage_call_id=stage_call_id,
    )


__all__ = ["StaticLmCallback", "rollout_context"]
