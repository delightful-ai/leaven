"""`lv.artifacts.repo(...)` — a repo artifact.

Governing spec: `docs/specs/leaven_python.md` — Artifact adapters.
"""

from __future__ import annotations

from collections.abc import Sequence

from .base import Artifact

__all__ = ["repo"]


def repo(root: str, *, mutable: Sequence[str] | None = None, **kwargs: object) -> Artifact:
    """Build a repo artifact (`lv.artifacts.repo("./repo")`)."""
    raise NotImplementedError("see leaven_python.md — Artifact adapters / repo")
