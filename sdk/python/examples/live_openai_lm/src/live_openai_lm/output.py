"""Output validation helpers for the live OpenAI LM proof."""

import json
from typing import Any

from leaven.assessment import Assessment

from live_openai_lm.config import EXPECTED_TEXT


def live_lm_output_from_assessment(assessment: Assessment) -> dict[str, Any]:
    """Extract the runner's public LM proof output from an assessment."""
    public = assessment.evidence.public
    if public is None:
        raise ValueError(f"assessment {assessment.case.id!r} has no public evidence")
    raw = public.payload.get("output")
    if not isinstance(raw, str):
        raise ValueError(f"assessment {assessment.case.id!r} public output is not inline text")
    value = json.loads(raw)
    if not isinstance(value, dict):
        raise ValueError(f"assessment {assessment.case.id!r} public output is not a JSON object")
    return value


def valid_live_lm_output(value: dict[str, Any]) -> bool:
    """Return whether a runner output proves the configured LM callback path."""
    usage = value.get("usage")
    return (
        value.get("text") == EXPECTED_TEXT
        and value.get("receipt") == "lmrec_completion"
        and isinstance(usage, dict)
        and int(usage.get("total_tokens", 0)) > 0
    )


__all__ = ["live_lm_output_from_assessment", "valid_live_lm_output"]
