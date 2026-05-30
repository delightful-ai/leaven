"""`lv.artifacts.directory(...)` — a directory artifact with mutable globs.

Governing spec: `docs/specs/leaven_python.md` — Artifact adapters.
"""

from __future__ import annotations

from collections.abc import Sequence

from .base import Artifact

__all__ = ["directory"]


def directory(root: str, *, mutable: Sequence[str], **kwargs: object) -> Artifact:
    """Build a directory artifact (`lv.artifacts.directory("./harness", mutable=[...])`)."""
    raise NotImplementedError("see leaven_python.md — Artifact adapters / directory")
