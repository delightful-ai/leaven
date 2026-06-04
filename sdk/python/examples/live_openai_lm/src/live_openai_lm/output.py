"""Output validation helpers for the live OpenAI LM proof."""

import json

from leaven.assessment import Assessment
from leaven.json_value import JsonObject, JsonValue

from live_openai_lm.config import EXPECTED_TEXT


def live_lm_output_from_assessment(assessment: Assessment) -> JsonObject:
    """Extract the runner's public LM proof output from an assessment."""
    public = assessment.evidence.public
    if public is None:
        raise ValueError(f"assessment {assessment.case.id!r} has no public evidence")
    if "output" not in public.payload:
        raise ValueError(f"assessment {assessment.case.id!r} public evidence has no output")
    raw = public.payload["output"]
    if not isinstance(raw, str):
        raise ValueError(f"assessment {assessment.case.id!r} public output is not inline text")
    return _json_object(json.loads(raw), context=f"assessment {assessment.case.id!r} public output")


def valid_live_lm_output(value: JsonObject) -> bool:
    """Return whether a runner output proves the configured LM callback path."""
    if "text" not in value or "receipt" not in value or "usage" not in value:
        return False
    usage = value["usage"]
    if not isinstance(usage, dict):
        return False
    if "total_tokens" not in usage:
        return False
    total_tokens = usage["total_tokens"]
    return (
        value["text"] == EXPECTED_TEXT
        and value["receipt"] == "lmrec_completion"
        and isinstance(total_tokens, int)
        and total_tokens > 0
    )


def _json_object(value: object, *, context: str) -> JsonObject:
    if not isinstance(value, dict):
        raise ValueError(f"{context} is not a JSON object")
    parsed: JsonObject = {}
    for key, item in value.items():
        if not isinstance(key, str):
            raise ValueError(f"{context} contains a non-string key")
        parsed[key] = _json_value(item, context=context)
    return parsed


def _json_value(value: object, *, context: str) -> JsonValue:
    if value is None or isinstance(value, str | int | float | bool):
        return value
    if isinstance(value, list):
        return [_json_value(item, context=context) for item in value]
    if isinstance(value, dict):
        return _json_object(value, context=context)
    raise ValueError(f"{context} contains non-JSON value {type(value).__name__}")


__all__ = ["live_lm_output_from_assessment", "valid_live_lm_output"]
