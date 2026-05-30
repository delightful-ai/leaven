"""Budget declaration — `lv.budget(usd=..., calls=...)`.

`calls=` counts metric/LM calls. Budget tracking for engine-mediated agent
sessions is a known unspecified gap in V1 (spec lines 843-847, 1443-1445).

Governing spec: `docs/specs/leaven_python.md` — Runtime / budget.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

__all__ = ["Budget", "budget"]


class Budget(BaseModel):
    """Immutable budget declaration.

    `usd=` caps spend; `calls=` caps metric/LM calls. Agent-session budget
    accounting is flagged unspecified in V1.
    """

    model_config = ConfigDict(frozen=True, extra="forbid")

    usd: float | None = None
    calls: int | None = None


def budget(*, usd: float | None = None, calls: int | None = None) -> Budget:
    """Construct a `Budget`.

    Spec: `lv.budget(usd=200, calls=2000)`.
    """
    raise NotImplementedError("see leaven_python.md — Runtime / budget")
