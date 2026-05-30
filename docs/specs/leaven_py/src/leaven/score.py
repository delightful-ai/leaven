"""Score and the Scorer type alias — `lv.Score`, `lv.Scorer`.

`Score` is the tiny scorer return value: a number and why. NO `metrics` dict,
NO `output` field, NO evidence blob at the product layer. Multiple objectives
are multiple named scorers, not a heavier `Score`.

`Score.feedback` is OPTIONAL actionable side-info that flows to reflection. When
feedback is empty, the engine lowers the `Score` to a stringified score for the
reflective batch — so an unannotated number is still legible to the reflector.
This shape is borrowed from GEPA / dspy `ScoreWithFeedback`
(`{score, feedback}`), where empty feedback degrades to the stringified score.

LOWERING CONTRACT: the wire `Score` record additionally requires an `output`
field; the ENGINE fills it from the `RolloutResult` at lowering time. The
product `Score` deliberately omits `output` — a scorer never repeats what the
rollout already produced.

`Scorer` is a TYPE ALIAS for the callable shape, exported for annotations ONLY.
There is NO `Scorer` constructor/class — a scorer is a plain async function;
agentic scoring is `cx.agent.run(...)` inside it. Premade scorers live in
`lv.scorers.*`.

Governing spec: `docs/specs/leaven_python.md` — Scorer and Score / the `score`
slot.
"""

from __future__ import annotations

from collections.abc import Awaitable, Callable
from typing import TYPE_CHECKING, Any

from pydantic import BaseModel, ConfigDict

if TYPE_CHECKING:
    from .case import Case
    from .context import Context
    from .rollout import RolloutResult

__all__ = ["Score", "Scorer"]


class Score(BaseModel):
    """A judgment: a number and why. Frozen."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    value: float
    feedback: str = ""


type Scorer = Callable[[RolloutResult[Any], Case, Context], Awaitable[Score]]
"""Annotation-only alias for the scorer callable shape. NOT a constructor."""
