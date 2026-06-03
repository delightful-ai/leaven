"""Deterministic aggregate score projection for the current prompt slice."""

from __future__ import annotations


def mean_score(scores: list[float]) -> float:
    """Return an average score with empty input defined as zero."""
    if not scores:
        return 0.0
    return sum(scores) / len(scores)


__all__ = ["mean_score"]
