"""Callback-backed LM builder for active Python stage contexts."""

from collections.abc import Sequence

from .._receipts import CallReceipt
from ..builders.lm import LmBuilder, LmMessage, LmResponse, _lm_response_from_result
from .protocols import LmCompleteCallback


class CallbackLmBuilder(LmBuilder):
    """A live `cx.lm` bound to the stage driver's `leaven/lm.complete` callback.

    The prompt path is wired: `complete(prompt=..., ...)` ships the prompt over
    the active stage seam and returns the host LM completion. Message lists,
    model/role selection, tools, and structured output are later slices.
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

    async def complete(  # type: ignore[override]
        self,
        *,
        prompt: str | None = None,
        messages: Sequence[LmMessage] | Sequence[dict[str, object]] | None = None,
        model: str | None = None,
        model_role: str | None = None,
        temperature: float | None = None,
        max_tokens: int | None = None,
        stop: Sequence[str] | None = None,
        response_format: object | None = None,
        tools: Sequence[dict[str, object]] | None = None,
        input_classes: Sequence[str] | None = None,
        forbidden_input_classes: Sequence[str] | None = None,
    ) -> LmResponse:
        _ = (messages, response_format, tools, forbidden_input_classes)
        if prompt is None:
            raise NotImplementedError("cx.lm.complete requires `prompt=` in this slice")
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
        return _lm_response_from_result(result, model=selected_model)


def lm_response(text: str) -> LmResponse:
    """Build the `LmResponse` returned by callback-backed `cx.lm.complete`.

    This slice carries completion text over the stage callback. Usage and cost
    projection remain later slices, so this response reports zero usage and no
    spend while preserving the call receipt shape.
    """
    return LmResponse(
        text=text,
        finish_reason="stop",
        usage={"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0},
        cost_usd=0.0,
        model="leaven-serve-mock",
        receipt=CallReceipt(receipt_id="lmrec_leaven_py_optimize"),
    )


__all__ = ["CallbackLmBuilder", "lm_response"]
