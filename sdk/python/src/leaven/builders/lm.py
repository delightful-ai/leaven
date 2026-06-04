"""`cx.lm.*` — LM completion through the seam's neutral request/response types."""

import asyncio
from collections.abc import Sequence
from typing import Literal, Protocol

from msgspec import UNSET, UnsetType
from pydantic import BaseModel, ConfigDict

from .._receipts import CallReceipt
from .._seam import LmCompleteRequest
from .._seam._wire import JsonObject
from .._seam._wire.json_value import json_object
from .._seam._wire.payloads import Cost
from .._seam._wire.results import LmCompleteResult
from ..json_value import JsonValue

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
    parsed: JsonValue | None = None
    """Parsed structured output when `response_format` was used."""
    finish_reason: str
    usage: dict[str, int]
    """{'prompt_tokens': N, 'completion_tokens': N, 'total_tokens': N}."""
    cost_usd: float | None = None
    model: str
    receipt: CallReceipt


class LmBuilder:
    """LM completion bound to a context. Calls are capability-gated + receipted."""

    def __init__(
        self,
        *,
        _client: "_SeamRequester | None" = None,
        _idempotency_prefix: str = "lm-builder",
        _plan_id: str = "planpythonlmbuilder001",
        _model: str = "gpt-4.1-mini",
    ) -> None:
        self._client = _client
        self._idempotency_prefix = _idempotency_prefix
        self._plan_id = _plan_id
        self._model = _model
        self._seq = 0

    @classmethod
    def _for_seam(
        cls,
        client: "_SeamRequester",
        *,
        idempotency_prefix: str = "lm-builder",
        plan_id: str = "planpythonlmbuilder001",
        model: str = "gpt-4.1-mini",
    ) -> "LmBuilder":
        """Bind this builder to the private public-seam process client."""
        return cls(
            _client=client,
            _idempotency_prefix=idempotency_prefix,
            _plan_id=plan_id,
            _model=model,
        )

    async def complete(
        self,
        *,
        prompt: str | None = None,
        messages: Sequence[LmMessage] | Sequence[JsonObject] | None = None,
        model: str | None = None,
        model_role: str | None = None,
        temperature: float | None = None,
        max_tokens: int | None = None,
        stop: Sequence[str] | None = None,
        response_format: object | None = None,
        tools: Sequence[JsonObject] | None = None,
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
        if self._client is None:
            raise NotImplementedError(
                "LmBuilder.complete needs an engine-bound public-seam client; "
                "use the cx.lm instance supplied to a running stage"
            )
        if forbidden_input_classes is not None:
            raise NotImplementedError(
                "LmBuilder.complete does not lower forbidden_input_classes yet"
            )
        if response_format is not None:
            raise NotImplementedError("LmBuilder.complete does not lower response_format yet")
        if tools is not None:
            raise NotImplementedError("LmBuilder.complete does not lower tools yet")

        selected_model = model or self._model
        request = LmCompleteRequest(
            request_id=f"{self._idempotency_prefix}-lm-{self._seq}",
            plan_id=self._plan_id,
            idempotency_key=f"{self._idempotency_prefix}-lm-{self._seq}",
            messages=_messages_to_wire(prompt=prompt, messages=messages),
            model=selected_model,
            model_role=model_role,
            temperature=temperature,
            max_tokens=max_tokens,
            stop=stop,
            input_classes=input_classes,
        )
        self._seq += 1
        result = await asyncio.to_thread(self._client.lm_complete, request)
        return _lm_response_from_result(result, model=selected_model)


class _SeamRequester(Protocol):
    """Small private protocol LmBuilder needs from the seam client."""

    def lm_complete(self, request: LmCompleteRequest) -> LmCompleteResult: ...


def _messages_to_wire(
    *,
    prompt: str | None,
    messages: Sequence[LmMessage] | Sequence[JsonObject] | None,
) -> list[JsonObject]:
    if (prompt is None) == (messages is None):
        raise ValueError("exactly one of prompt= or messages= is required")
    if prompt is not None:
        return [_message_to_wire({"role": "user", "content": prompt})]
    assert messages is not None
    return [_message_to_wire(message) for message in messages]


def _message_to_wire(message: LmMessage | JsonObject) -> JsonObject:
    value = message.model_dump() if isinstance(message, LmMessage) else dict(message)
    content = value.get("content")
    if isinstance(content, str):
        content = [{"kind": "text", "text": content}]
    elif not isinstance(content, list):
        raise NotImplementedError("LmBuilder.complete only lowers text message content yet")
    wire = {
        "role": value["role"],
        "content": content,
    }
    if value.get("tool_call_id") is not None:
        wire["tool_call_id"] = value["tool_call_id"]
    return json_object(wire)


def _lm_response_from_result(result: LmCompleteResult, *, model: str) -> LmResponse:
    primary = result.primary
    message = primary.message
    text = "".join(part.text for part in message.content)
    return LmResponse(
        text=text,
        parsed=None,
        finish_reason="stop",
        usage=_usage(primary.cost),
        cost_usd=_cost_usd(primary.cost),
        model=model,
        receipt=CallReceipt(receipt_id=primary.receipt),
    )


def _usage(cost: Cost | UnsetType) -> dict[str, int]:
    if cost is UNSET:
        return {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}
    prompt_tokens = 0 if cost.input_tokens is UNSET else cost.input_tokens
    completion_tokens = 0 if cost.output_tokens is UNSET else cost.output_tokens
    return {
        "prompt_tokens": prompt_tokens,
        "completion_tokens": completion_tokens,
        "total_tokens": prompt_tokens + completion_tokens,
    }


def _cost_usd(cost: Cost | UnsetType) -> float | None:
    if cost is UNSET:
        return None
    usd_micro = cost.usd_micro
    return None if usd_micro is UNSET else usd_micro / 1_000_000


__all__ = ["LmBuilder", "LmMessage", "LmMessageRole", "LmResponse"]
