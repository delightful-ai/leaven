"""LM Plan IR request construction for private public-seam clients."""

from collections.abc import Sequence
from dataclasses import dataclass

from msgspec import UNSET, convert

from leaven._seam._wire.calls import (
    LmCompleteCall,
    LmMessage,
    LmOutputContract,
    LmOutputFinalMessage,
    LmSampling,
    LmTool,
)
from leaven._seam._wire.payloads import CommitPolicyNoGraphWrites, PlanDocument, PlanOp

from .plans import SeamRequestMethod, _plan_document

# Default response byte budget when the runner sets no `max_tokens`. Reasoning
# runners (e.g. AIME solving) set `max_tokens`, which sizes the byte budget below
# so the final message is not refused for exceeding a tiny fixed cap.
_DEFAULT_FINAL_MESSAGE_MAX_BYTES = 512
# A generous upper bound on bytes per output token (multi-byte UTF-8 plus
# whitespace), used to size the final-message byte budget from `max_tokens`.
_BYTES_PER_TOKEN = 8


@dataclass(frozen=True)
class LmCompleteRequest:
    """A single public-seam `leaven/lm.complete` Plan request."""

    request_id: str
    plan_id: str
    idempotency_key: str
    messages: Sequence[LmMessage]
    model: str
    model_role: str | None = None
    temperature: float | None = None
    max_tokens: int | None = None
    stop: Sequence[str] | None = None
    output: LmOutputContract | None = None
    tools: Sequence[LmTool] | None = None
    input_classes: Sequence[str] | None = None
    forbidden_input_classes: Sequence[str] | None = None

    @property
    def method(self) -> SeamRequestMethod:
        """Locked LM method."""
        return "leaven/lm.complete"

    def to_params(self) -> PlanDocument:
        """Return the locked LM Plan params."""
        return _plan_document(
            plan_id=self.plan_id,
            ops=[self._lm_call()],
            return_names=["completion"],
            commit=CommitPolicyNoGraphWrites(),
        )

    def _final_message_max_bytes(self) -> int:
        """Size the default final-message byte cap to the requested token budget.

        The host refuses a final message that exceeds this cap, so a small fixed
        default would truncate reasoning runners. When `max_tokens` is set, size
        the byte budget to that many output tokens; otherwise keep the small
        default for short completions.
        """
        if self.max_tokens is None:
            return _DEFAULT_FINAL_MESSAGE_MAX_BYTES
        return max(_DEFAULT_FINAL_MESSAGE_MAX_BYTES, self.max_tokens * _BYTES_PER_TOKEN)

    def _lm_call(self) -> PlanOp:
        sampling = {}
        if self.temperature is not None:
            sampling["temperature"] = self.temperature
        if self.max_tokens is not None:
            sampling["max_output_tokens"] = self.max_tokens
        if self.stop is not None:
            sampling["stop"] = list(self.stop)
        return PlanOp(
            kind="call",
            name="completion",
            idempotency_key=self.idempotency_key,
            call=LmCompleteCall(
                purpose="python.sdk",
                model=self.model,
                model_role=self.model_role if self.model_role is not None else UNSET,
                messages=list(self.messages),
                output=self.output or LmOutputFinalMessage(max_bytes=self._final_message_max_bytes()),
                tools=list(self.tools) if self.tools is not None else UNSET,
                sampling=convert(sampling, type=LmSampling) if sampling else UNSET,
                input_classes=list(self.input_classes or ["public"]),
                forbidden_input_classes=(
                    list(self.forbidden_input_classes)
                    if self.forbidden_input_classes is not None
                    else UNSET
                ),
            ),
        )


__all__ = ["LmCompleteRequest"]
