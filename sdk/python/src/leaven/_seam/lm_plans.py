"""LM Plan IR request construction for private public-seam clients."""

from collections.abc import Sequence
from dataclasses import dataclass

from msgspec import UNSET, convert

from leaven._seam._wire.calls import (
    LmCompleteCall,
    LmMessage,
    LmOutputContract,
    LmOutputFinalMessage,
    LmOutputJsonSchema,
    LmSampling,
    LmTool,
)
from leaven._seam._wire.json_value import JsonObject
from leaven._seam._wire.payloads import CommitPolicyNoGraphWrites, PlanDocument, PlanOp
from leaven._seam._wire.refs import WireJsonSchemaObject

from .plans import SeamRequestMethod, _optional_int_field, _plan_document, _string_field


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
    output: JsonObject | None = None
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
                output=_wire_lm_output_contract(
                    self.output or {"kind": "final_message", "max_bytes": 512}
                ),
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


def _wire_lm_output_contract(value: JsonObject) -> LmOutputContract:
    kind = _string_field(value, "kind")
    if kind == "final_message":
        return LmOutputFinalMessage(max_bytes=_optional_int_field(value, "max_bytes"))
    if kind == "json_schema":
        schema = UNSET
        if "schema" in value:
            schema = convert(value["schema"], type=WireJsonSchemaObject)
        return LmOutputJsonSchema(
            schema_fingerprint=_string_field(value, "schema_fingerprint"),
            schema=schema,
        )
    raise ValueError(f"unsupported LM output contract kind: {kind}")


__all__ = ["LmCompleteRequest"]
