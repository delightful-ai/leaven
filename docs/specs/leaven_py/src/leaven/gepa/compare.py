"""GEPA multi-score comparison policy — `lv.gepa.compare.*`.

`lv.gepa.compare.weighted({correctness: 0.8, trajectory_quality: 0.2})` keys by
the SCORER OBJECT (spec line 1143).

Governing spec: `docs/specs/leaven_python.md` — Optimizers (GEPA policy).
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..score import Scorer

__all__ = ["CompareConfig", "lexicographic", "weighted"]


@dataclass(frozen=True, slots=True)
class CompareConfig:
    """A multi-score comparison policy; `kind` discriminates."""

    kind: str


def weighted(weights: Mapping[Scorer, float]) -> CompareConfig:
    """Weighted comparison keyed by scorer object."""
    raise NotImplementedError("see leaven_python.md — gepa.compare")


def lexicographic(order: Sequence[Scorer]) -> CompareConfig:
    """Lexicographic comparison in scorer-object order."""
    raise NotImplementedError("see leaven_python.md — gepa.compare")
