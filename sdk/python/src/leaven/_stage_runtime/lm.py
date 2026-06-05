"""Callback-backed LM builder for active Python stage contexts."""

from collections.abc import Sequence
from typing import overload

from pydantic import BaseModel

from .._errors import UnsupportedConfigurationError
from .._receipts import CallReceipt
from .._seam._wire import JsonObject
from ..builders.lm import LmBuilder, LmMessage, LmResponse, LmTool, _lm_response_from_result
from ..json_value import JsonValue
from ..output import JsonSchemaOutput, JsonSchemaValueOutput
from .protocols import LmCompleteCallback


class CallbackLmBuilder(LmBuilder):
    """A live `cx.lm` bound to the stage driver's `leaven/lm.complete` callback.

    The prompt path is wired: `complete(prompt=..., ...)` ships the prompt over
    the active stage seam and returns the host LM completion. Message lists,
    model/role selection, and tools are later slices.
    """

    def __init__(
        self,
        callback: LmCompleteCallback,
        stage_call_id: str,
        *,
        default_model: str,
    ) -> None:
        self._callback = callback
        self._stage_call_id = stage_call_id
        self._default_model = default_model
        self._seq = 0

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
        _ = forbidden_input_classes
        if tools is not None:
            raise UnsupportedConfigurationError(
                "callback-backed cx.lm.complete does not lower tools yet"
            )
        if prompt is None:
            raise UnsupportedConfigurationError(
                "callback-backed cx.lm.complete requires `prompt=` in this slice"
            )
        if messages is not None:
            raise UnsupportedConfigurationError(
                "callback-backed cx.lm.complete does not lower messages yet"
            )
        request_id = f"{self._stage_call_id}::lm::{self._seq}"
        self._seq += 1
        selected_model = model or self._default_model
        result = await self._callback.lm_complete(
            prompt,
            request_id=request_id,
            model=selected_model,
            model_role=model_role,
            temperature=temperature,
            max_tokens=max_tokens,
            stop=stop,
            input_classes=input_classes,
        )
        return _lm_response_from_result(result, model=selected_model, output=response_format)


def lm_response(text: str) -> LmResponse[JsonValue]:
    """Build the `LmResponse` returned by callback-backed `cx.lm.complete`.

    This slice carries completion text over the stage callback. Usage and cost
    projection remain later slices, so this response reports zero usage and no
    spend while preserving the call receipt shape.
    """
    return LmResponse(
        text=text,
        parsed=None,
        finish_reason="stop",
        usage={"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0},
        cost_usd=0.0,
        model="leaven-serve-mock",
        receipt=CallReceipt(receipt_id="lmrec_leaven_py_optimize"),
    )


__all__ = ["CallbackLmBuilder", "lm_response"]
