"""`lv.gepa.*` — GEPA-namespaced optimizer policy.

GEPA-specific policy that orchestrates the four stages: sampling, validation,
frontier, gate, component, compare, plus the `reflective_dataset` hook. These
are deliberately GEPA-namespaced, not a generic optimizer-agnostic interface.

Governing spec: `docs/specs/leaven_python.md` — Optimizers.
"""

from __future__ import annotations

from . import (
    compare,
    component,
    frontier,
    gate,
    reflective_dataset,
    sampling,
    validation,
)

__all__ = [
    "compare",
    "component",
    "frontier",
    "gate",
    "reflective_dataset",
    "sampling",
    "validation",
]
