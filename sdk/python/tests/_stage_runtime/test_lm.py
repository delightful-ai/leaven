"""Tests for callback-backed stage-runtime LM builder."""

from collections.abc import Sequence

import msgspec
from pydantic import BaseModel

import leaven as lv
from leaven._seam._wire.payloads import Cost
from leaven._seam._wire.refs import WireJsonExtensionPayload
from leaven._seam._wire.results import (
    LmCompleteResult,
    LmContentPart,
    LmMessageRecord,
    LmResponsePrimary,
)
from leaven._stage_runtime.lm import CallbackLmBuilder


class StructuredAnswer(BaseModel):
    answer: str


async def test_callback_lm_builder_passes_response_format_to_result_projection() -> None:
    """Example: stage `cx.lm.complete` preserves its typed response contract."""

    lm = CallbackLmBuilder(
        FakeLmCallback(),
        "stage_call_001",
        default_model="mock-lm",
    )

    response = await lm.complete(
        prompt="Answer as JSON.",
        response_format=lv.output.json_schema(StructuredAnswer),
    )

    assert response.parsed == StructuredAnswer(answer="ok")
    assert response.parsed.answer == "ok"


class FakeLmCallback:
    async def lm_complete(
        self,
        prompt: str,
        *,
        request_id: str,
        model: str,
        model_role: str | None = None,
        temperature: float | None = None,
        max_tokens: int | None = None,
        stop: Sequence[str] | None = None,
        input_classes: Sequence[str] | None = None,
    ) -> LmCompleteResult:
        _ = (
            prompt,
            request_id,
            model,
            model_role,
            temperature,
            max_tokens,
            stop,
            input_classes,
        )
        return LmCompleteResult(
            method="leaven/lm.complete",
            primary=LmResponsePrimary(
                kind="lm_response",
                message=LmMessageRecord(
                    role="assistant",
                    content=[LmContentPart(kind="text", text='{"answer":"ok"}')],
                ),
                receipt="lmrec_callback",
                graph_revision="rev_callback_lm",
                data_classes=["public"],
                replayability="boundary_managed",
                cost=Cost(usd_micro=42, input_tokens=3, output_tokens=2),
                parsed=_wire_json({"answer": "ok"}),
            ),
            receipts=[],
            redactions=[],
            capability_fingerprint="fp_cap_callback",
            policy_fingerprint="fp_policy_callback",
            data_classes=["public"],
        )


def _wire_json(value: dict[str, str]) -> WireJsonExtensionPayload:
    return msgspec.convert(value, type=WireJsonExtensionPayload)
