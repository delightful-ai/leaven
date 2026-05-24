"""Score type — the scalar(+output+metrics) output of a scorer.

Scorers return a Score; the Score's value drives optimizer selection.
The output field provides the visibility-labeled projection the optimizer
reads as feedback.
"""

from __future__ import annotations

from typing import Self

from pydantic import BaseModel, ConfigDict, Field

from .output_record import OutputRecord


class Score(BaseModel):
    """Score for one (candidate, case) pair.

    `value` is the optimizer-visible scalar (typically [0,1]).
    `output` is the visibility-labeled feedback the optimizer reads.
    `metrics` are arbitrary side-band measurements (always optimizer-visible).
    """

    model_config = ConfigDict(frozen=True, extra="forbid")

    value: float
    output: OutputRecord
    metrics: dict[str, float] = Field(default_factory=dict)

    @classmethod
    def exact_match(cls, output: str, target: str) -> Self:
        """Convenience: 1.0 if `output == target`, else 0.0."""
        raise NotImplementedError("scaffold; see leaven.scoring.exact_match")


__all__ = ["Score"]
