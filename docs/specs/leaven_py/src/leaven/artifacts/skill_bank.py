"""`lv.artifacts.skill_bank(...)` — a skill-bank artifact.

Governing spec: `docs/specs/leaven_python.md` — Artifact adapters.
"""

from __future__ import annotations

from .base import Artifact

__all__ = ["skill_bank"]


def skill_bank(root: str, **kwargs: object) -> Artifact:
    """Build a skill-bank artifact (`lv.artifacts.skill_bank("./skills")`)."""
    raise NotImplementedError("see leaven_python.md — Artifact adapters / skill_bank")
