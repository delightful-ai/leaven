"""GEPA train-minibatch sampling policy — `lv.gepa.sampling.*`.

Governing spec: `docs/specs/leaven_python.md` — Optimizers (GEPA policy).
"""

from __future__ import annotations

from dataclasses import dataclass

__all__ = ["SamplingPolicy", "full", "minibatch"]


@dataclass(frozen=True, slots=True)
class SamplingPolicy:
    """A train sampling policy; `kind` discriminates."""

    kind: str


def minibatch(*, split: str, size: int) -> SamplingPolicy:
    """Sample a minibatch of `size` cases from `split`."""
    raise NotImplementedError("see leaven_python.md — gepa.sampling")


def full(*, split: str) -> SamplingPolicy:
    """Use the full `split`."""
    raise NotImplementedError("see leaven_python.md — gepa.sampling")
