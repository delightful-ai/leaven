"""MIPRO — reserved optimizer name (raises NotImplementedError in V1).

Governing spec: `docs/specs/leaven_python.md` — Optimizers (reserved names).
"""

from __future__ import annotations

from . import Optimizer

__all__ = ["mipro"]


def mipro(*args: object, **kwargs: object) -> Optimizer:
    """Reserved optimizer name; not behavior-bearing in V1."""
    raise NotImplementedError(
        "reserved optimizer name; GEPA is the only behavior-bearing optimizer in V1"
    )
