"""`cx.lm.*` — LM completion through the seam's neutral request/response types."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any, Literal

from pydantic import BaseModel, ConfigDict

from .._receipts import CallReceipt

LmMessageRole = Literal["system", "developer", "user", "assistant", "tool"]


class LmMessage(BaseModel):
    """One message in an LM conversation."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    role: LmMessageRole
    content: str
    tool_call_id: str | None = None


class LmResponse(BaseModel):
    """Result of `cx.lm.complete(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    text: str
    """Assistant-authored final response text."""
    parsed: Any | None = None
    """Parsed structured output when `response_format` was used."""
    finish_reason: str
    usage: dict[str, int]
    """{'prompt_tokens': N, 'completion_tokens': N, 'total_tokens': N}."""
    cost_usd: float | None = None
    model: str
    receipt: CallReceipt


class LmBuilder:
    """LM completion bound to a context. Calls are capability-gated + receipted."""

    async def complete(
        self,
        *,
        prompt: str | None = None,
        messages: Sequence[LmMessage] | Sequence[dict[str, Any]] | None = None,
        model: str | None = None,
        model_role: str | None = None,
        temperature: float | None = None,
        max_tokens: int | None = None,
        stop: Sequence[str] | None = None,
        response_format: Any | None = None,
        tools: Sequence[dict[str, Any]] | None = None,
        input_classes: Sequence[str] | None = None,
        forbidden_input_classes: Sequence[str] | None = None,
    ) -> LmResponse:
        """Complete a prompt or message list.

        Either `prompt` or `messages` is required (not both). `model` selects
        a specific configured LM; `model_role` selects by configured role
        (`"reflector"`, `"grader"`, etc.). `input_classes` and
        `forbidden_input_classes` declare what data the call may carry —
        the seam enforces.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["LmBuilder", "LmMessage", "LmMessageRole", "LmResponse"]
