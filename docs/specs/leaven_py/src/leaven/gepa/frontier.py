"""GEPA frontier representation policy — `lv.gepa.frontier.*`.

Governing spec: `docs/specs/leaven_python.md` — Optimizers (GEPA policy).
"""

from __future__ import annotations

from dataclasses import dataclass

__all__ = ["FrontierPolicy", "pareto", "top_k"]


@dataclass(frozen=True, slots=True)
class FrontierPolicy:
    """A frontier representation policy; `kind` discriminates."""

    kind: str


def top_k(k: int) -> FrontierPolicy:
    """Keep the top-`k` candidates on the frontier."""
    raise NotImplementedError("see leaven_python.md — gepa.frontier")


def pareto(**kwargs: object) -> FrontierPolicy:
    """Keep a Pareto frontier."""
    raise NotImplementedError("see leaven_python.md — gepa.frontier")
