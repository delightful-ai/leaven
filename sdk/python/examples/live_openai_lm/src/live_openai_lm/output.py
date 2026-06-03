"""Output validation helpers for the live OpenAI LM proof."""

from __future__ import annotations

from typing import Any

from live_openai_lm.config import EXPECTED_TEXT


def valid_live_lm_output(value: dict[str, Any]) -> bool:
    """Return whether a runner output proves the configured LM callback path."""
    usage = value.get("usage")
    return (
        value.get("text") == EXPECTED_TEXT
        and value.get("receipt") == "lmrec_completion"
        and isinstance(usage, dict)
        and int(usage.get("total_tokens", 0)) > 0
    )


__all__ = ["valid_live_lm_output"]
