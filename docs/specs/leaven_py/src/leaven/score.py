"""Score type — scalar selection signal plus feedback.

Scorers return a Score; the Score's value drives optimizer selection.
Feedback explains the scalar. The engine binds the rollout output and evidence
behind the scenes.
"""

from __future__ import annotations

from typing import Self

from pydantic import BaseModel, ConfigDict


class Score(BaseModel):
    """Score for one (candidate, case) pair.

    `value` is the optimizer-visible scalar (typically [0,1]).
    `feedback` is natural-language scorer feedback.
    """

    model_config = ConfigDict(frozen=True, extra="forbid")

    value: float
    feedback: str = ""

    @classmethod
    def exact_match(cls, output: str, target: str) -> Self:
        """Convenience: 1.0 if `output == target`, else 0.0."""
        raise NotImplementedError("scaffold; see leaven.scoring.exact_match")


__all__ = ["Score"]
