"""Output validation helpers for the live OpenAI LM proof."""

import json

from leaven.assessment import Assessment
from leaven.json_value import JsonObject, JsonValue
from pydantic import BaseModel, ConfigDict

from live_openai_lm.config import EXPECTED_TEXT


class LiveLmUsage(BaseModel):
    """Usage facts projected by the live LM callback."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    total_tokens: int


class LiveLmOutput(BaseModel):
    """Typed runner output proving the live LM callback path."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    text: str
    receipt: str
    usage: LiveLmUsage
    cost_usd: float | None
    model: str


def live_lm_output_from_assessment(assessment: Assessment) -> LiveLmOutput:
    """Extract the runner's public LM proof output from an assessment."""
    public = assessment.evidence.public
    if public is None:
        raise ValueError(f"assessment {assessment.case.id!r} has no public evidence")
    if "output" not in public.payload:
        raise ValueError(f"assessment {assessment.case.id!r} public evidence has no output")
    raw = public.payload["output"]
    if not isinstance(raw, str):
        raise ValueError(f"assessment {assessment.case.id!r} public output is not inline text")
    return LiveLmOutput.model_validate(
        _json_object(json.loads(raw), context=f"assessment {assessment.case.id!r} public output")
    )


def valid_live_lm_output(value: LiveLmOutput) -> bool:
    """Return whether a runner output proves the configured LM callback path."""
    return (
        value.text == EXPECTED_TEXT
        and value.receipt == "lmrec_completion"
        and value.usage.total_tokens > 0
    )


def _json_object(raw_json: object, *, context: str) -> JsonObject:
    if not isinstance(raw_json, dict):
        raise ValueError(f"{context} is not a JSON object")
    parsed: JsonObject = {}
    for key, item in raw_json.items():
        if not isinstance(key, str):
            raise ValueError(f"{context} contains a non-string key")
        parsed[key] = _json_value(item, context=context)
    return parsed


def _json_value(raw_json: object, *, context: str) -> JsonValue:
    if raw_json is None or isinstance(raw_json, str | int | float | bool):
        return raw_json
    if isinstance(raw_json, list):
        return [_json_value(item, context=context) for item in raw_json]
    if isinstance(raw_json, dict):
        return _json_object(raw_json, context=context)
    raise ValueError(f"{context} contains non-JSON value {type(raw_json).__name__}")


__all__ = ["LiveLmOutput", "LiveLmUsage", "live_lm_output_from_assessment", "valid_live_lm_output"]
