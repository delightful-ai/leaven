"""Deterministic local score projection for the current prompt slice."""

from __future__ import annotations

from typing import Any


def exact_answer_score(output: Any, target: dict[str, Any] | None) -> float:
    """Return exact-match score against the common `target["answer"]` shape."""
    if target is None or "answer" not in target:
        return 0.0
    return 1.0 if str(output).strip() == str(target["answer"]).strip() else 0.0


def mean_score(scores: list[float]) -> float:
    """Return an average score with empty input defined as zero."""
    if not scores:
        return 0.0
    return sum(scores) / len(scores)


__all__ = ["exact_answer_score", "mean_score"]
