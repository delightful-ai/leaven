"""`cx.lm.*` — LM completion through the seam's neutral request/response types."""

import asyncio
from collections.abc import Sequence
from typing import Literal, Protocol, overload

import msgspec
from msgspec import UNSET, UnsetType
from pydantic import BaseModel, ConfigDict

from .._errors import UnboundBuilderError
from .._receipts import CallReceipt
from .._seam import LmCompleteRequest
from .._seam._wire import JsonObject
from .._seam._wire.calls import LmTool as WireLmTool
from .._seam._wire.json_value import json_object, json_value
from .._seam._wire.payloads import Cost
from .._seam._wire.refs import WireJsonSchemaObject
from .._seam._wire.results import LmCompleteResult
from ..json_value import JsonSchema, JsonValue
from ..output import JsonSchemaOutput, JsonSchemaValueOutput
from ._output_contract import json_schema_output_to_wire

LmMessageRole = Literal["system", "developer", "user", "assistant", "tool"]


class LmMessage(BaseModel):
    """One message in an LM conversation."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    role: LmMessageRole
    content: str
    tool_call_id: str | None = None


class LmTool(BaseModel):
    """One tool declaration available to an LM completion."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    name: str
    input_schema: JsonSchema
    description: str | None = None
    requires_capability_action: str | None = None


class LmResponse[ParsedOutputT](BaseModel):
    """Result of `cx.lm.complete(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    text: str
    """Assistant-authored final response text."""
    parsed: ParsedOutputT
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

    @overload
    async def complete[ParsedOutputT: BaseModel](
        self,
        *,
        prompt: str | None = None,
        messages: Sequence[LmMessage] | Sequence[JsonObject] | None = None,
        model: str | None = None,
        model_role: str | None = None,
        temperature: float | None = None,
        max_tokens: int | None = None,
        stop: Sequence[str] | None = None,
        response_format: JsonSchemaOutput[ParsedOutputT],
        tools: Sequence[LmTool] | None = None,
        input_classes: Sequence[str] | None = None,
        forbidden_input_classes: Sequence[str] | None = None,
    ) -> LmResponse[ParsedOutputT]: ...

    @overload
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
        response_format: JsonSchemaValueOutput | None = None,
        tools: Sequence[LmTool] | None = None,
        input_classes: Sequence[str] | None = None,
        forbidden_input_classes: Sequence[str] | None = None,
    ) -> LmResponse[JsonValue]: ...

    async def complete[ParsedOutputT: BaseModel](
        self,
        *,
        prompt: str | None = None,
        messages: Sequence[LmMessage] | Sequence[JsonObject] | None = None,
        model: str | None = None,
        model_role: str | None = None,
        temperature: float | None = None,
        max_tokens: int | None = None,
        stop: Sequence[str] | None = None,
        response_format: JsonSchemaOutput[ParsedOutputT] | JsonSchemaValueOutput | None = None,
        tools: Sequence[LmTool] | None = None,
        input_classes: Sequence[str] | None = None,
        forbidden_input_classes: Sequence[str] | None = None,
    ) -> LmResponse[ParsedOutputT] | LmResponse[JsonValue]:
        """Complete a prompt or message list.

        Either `prompt` or `messages` is required (not both). `model` selects
        a specific configured LM; `model_role` selects by configured role
        (`"reflector"`, `"grader"`, etc.). `input_classes` and
        `forbidden_input_classes` declare what data the call may carry —
        the seam enforces.
        """
        if self._client is None:
            raise UnboundBuilderError(
                "LmBuilder.complete needs an engine-bound public-seam client; "
                "use the cx.lm instance supplied to a running stage"
            )

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
            output=None if response_format is None else json_schema_output_to_wire(response_format),
            tools=None if tools is None else [_tool_to_wire(tool) for tool in tools],
            input_classes=input_classes,
            forbidden_input_classes=forbidden_input_classes,
        )
        self._seq += 1
        result = await asyncio.to_thread(self._client.lm_complete, request)
        return _lm_response_from_result(result, model=selected_model, output=response_format)


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
    content = value["content"]
    if isinstance(content, str):
        content = [{"kind": "text", "text": content}]
    elif not isinstance(content, list):
        raise TypeError("LmBuilder.complete requires text message content")
    wire = {
        "role": value["role"],
        "content": content,
    }
    if "tool_call_id" in value and value["tool_call_id"] is not None:
        wire["tool_call_id"] = value["tool_call_id"]
    return json_object(wire)


def _tool_to_wire(tool: LmTool) -> WireLmTool:
    description = tool.description if tool.description is not None else UNSET
    requires_capability_action = (
        tool.requires_capability_action
        if tool.requires_capability_action is not None
        else UNSET
    )
    return WireLmTool(
        name=tool.name,
        input_schema=msgspec.convert(tool.input_schema, type=WireJsonSchemaObject),
        description=description,
        requires_capability_action=requires_capability_action,
    )


def _lm_response_from_result[ParsedOutputT: BaseModel](
    result: LmCompleteResult,
    *,
    model: str,
    output: JsonSchemaOutput[ParsedOutputT] | JsonSchemaValueOutput | None,
) -> LmResponse[ParsedOutputT] | LmResponse[JsonValue]:
    primary = result.primary
    message = primary.message
    text = "".join(part.text for part in message.content)
    return LmResponse(
        text=text,
        parsed=_parsed_json(primary.parsed, output),
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


def _parsed_json[ParsedOutputT: BaseModel](
    value: msgspec.Raw | UnsetType,
    output: JsonSchemaOutput[ParsedOutputT] | JsonSchemaValueOutput | None,
) -> ParsedOutputT | JsonValue:
    if value is UNSET:
        if isinstance(output, JsonSchemaOutput):
            raise TypeError("model-backed LM response is missing parsed payload")
        return None
    if isinstance(output, JsonSchemaOutput):
        model = output.parse_to
        return model.model_validate_json(bytes(value))
    return json_value(msgspec.json.decode(value))


__all__ = ["LmBuilder", "LmMessage", "LmMessageRole", "LmResponse", "LmTool"]
