"""GEPA child-acceptance gate policy — `lv.gepa.gate.*`.

Governing spec: `docs/specs/leaven_python.md` — Optimizers (GEPA policy).
"""

from __future__ import annotations

from dataclasses import dataclass

__all__ = ["GatePolicy", "improvement"]


@dataclass(frozen=True, slots=True)
class GatePolicy:
    """A child-acceptance gate policy; `kind` discriminates."""

    kind: str


def improvement(*, min_delta: float = 0.0) -> GatePolicy:
    """Accept a child only if it improves by at least `min_delta`."""
    raise NotImplementedError("see leaven_python.md — gepa.gate")
