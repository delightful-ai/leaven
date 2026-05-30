"""GEPA target-component policy — `lv.gepa.component.*`.

Which artifact part GEPA targets for a proposal.

Governing spec: `docs/specs/leaven_python.md` — Optimizers (GEPA policy).
"""

from __future__ import annotations

from dataclasses import dataclass

__all__ = ["ComponentPolicy", "all", "named"]


@dataclass(frozen=True, slots=True)
class ComponentPolicy:
    """A target-component policy; `kind` discriminates."""

    kind: str


def all(**kwargs: object) -> ComponentPolicy:
    """Target all mutable components."""
    raise NotImplementedError("see leaven_python.md — gepa.component")


def named(*names: str) -> ComponentPolicy:
    """Target the named components."""
    raise NotImplementedError("see leaven_python.md — gepa.component")
