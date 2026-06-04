"""Score type — scalar selection signal plus feedback.

Scorers return a Score; the Score's value drives optimizer selection.
Feedback explains the scalar. The engine binds the rollout output and evidence
behind the scenes.
"""

from typing import Self

from pydantic import BaseModel, ConfigDict

from . import scoring


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
        value = scoring.exact_match(output, target)
        feedback = "exact match" if value == 1.0 else f"expected {target!r}, got {output!r}"
        return cls(value=value, feedback=feedback)


__all__ = ["Score"]
