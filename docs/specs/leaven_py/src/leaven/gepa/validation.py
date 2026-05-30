"""GEPA accepted-candidate validation policy — `lv.gepa.validation.*`.

Governing spec: `docs/specs/leaven_python.md` — Optimizers (GEPA policy).
"""

from __future__ import annotations

from dataclasses import dataclass

__all__ = ["ValidationPolicy", "full", "minibatch"]


@dataclass(frozen=True, slots=True)
class ValidationPolicy:
    """A validation policy; `kind` discriminates."""

    kind: str


def full(*, split: str) -> ValidationPolicy:
    """Validate against the full `split`."""
    raise NotImplementedError("see leaven_python.md — gepa.validation")


def minibatch(*, split: str, size: int) -> ValidationPolicy:
    """Validate against a minibatch of `size` cases from `split`."""
    raise NotImplementedError("see leaven_python.md — gepa.validation")
