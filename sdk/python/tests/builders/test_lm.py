"""Tests for `leaven.builders.lm`."""

import json

import msgspec

import leaven as lv
from leaven._seam import LmCompleteRequest
from leaven._seam._wire.payloads import Cost
from leaven._seam._wire.results import (
    LmCompleteResult,
    LmContentPart,
    LmMessageRecord,
    LmResponsePrimary,
)
from leaven.builders.lm import LmBuilder
from leaven.json_value import JsonObject, JsonValue


async def test_lm_builder_complete_lowers_json_schema_response_format() -> None:
    """Example: bound `lm.complete` carries structured output schema authority."""

    client = FakeLmSeamClient()
    lm = LmBuilder._for_seam(
        client,
        idempotency_prefix="lm-builder-json-schema",
        plan_id="planlmbuilderjson001",
        model="gpt-4.1-mini",
    )
    response_format = lv.output.json_schema(
        {
            "type": "object",
            "properties": {"answer": {"type": "string"}},
            "required": ["answer"],
            "additionalProperties": False,
        }
    )

    response = await lm.complete(prompt="Answer as JSON.", response_format=response_format)

    params = _params_object(client.request_value.to_params())
    ops = _json_array(params["ops"])
    call = _json_object(_json_object(ops[0])["call"])
    output_wire = _json_object(call["output"])
    assert output_wire == {
        "kind": "json_schema",
        "schema_fingerprint": (
            "fp_schema_sha256_"
            "d7f69ea25824f613d0b60198abe050adc66a3bf45d9f2045d1997214a55498e5"
        ),
        "schema": response_format.schema_,
    }
    assert response.parsed == {"answer": "ok"}


class FakeLmSeamClient:
    def __init__(self) -> None:
        self.request_value = LmCompleteRequest(
            request_id="unset",
            plan_id="unset",
            idempotency_key="unset",
            messages=[{"role": "user", "content": [{"kind": "text", "text": "unset"}]}],
            model="unset",
        )

    def lm_complete(self, request: LmCompleteRequest) -> LmCompleteResult:
        self.request_value = request
        return LmCompleteResult(
            method="leaven/lm.complete",
            primary=LmResponsePrimary(
                kind="lm_response",
                message=LmMessageRecord(
                    role="assistant",
                    content=[LmContentPart(kind="text", text='{"answer":"ok"}')],
                ),
                receipt="lmrec_completion",
                graph_revision="rev_lm_builder",
                data_classes=["public"],
                replayability="boundary_managed",
                cost=Cost(usd_micro=42, input_tokens=3, output_tokens=2),
                parsed=msgspec.Raw(msgspec.json.encode({"answer": "ok"})),
            ),
            receipts=[],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["public"],
        )


def _params_object(params: object) -> JsonObject:
    value = json.loads(msgspec.json.encode(params))
    if not isinstance(value, dict):
        raise TypeError("expected JSON object")
    return value


def _json_array(value: JsonValue) -> list[JsonValue]:
    if not isinstance(value, list):
        raise TypeError("expected JSON array")
    return value


def _json_object(value: JsonValue) -> JsonObject:
    if not isinstance(value, dict):
        raise TypeError("expected JSON object")
    return value
