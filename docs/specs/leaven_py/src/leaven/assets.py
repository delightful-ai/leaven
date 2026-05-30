"""Asset references — `lv.assets.path("assets/...")`.

Asset references for `Case.files`. The engine materializes the referenced
asset into the case workspace.

Governing spec: `docs/specs/leaven_python.md` — Task and Case.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

__all__ = ["AssetRef", "path"]


class AssetRef(BaseModel):
    """An immutable reference to a case asset."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: str
    location: str


def path(location: str, **kwargs: object) -> AssetRef:
    """Reference an asset by path (`lv.assets.path("assets/challenge")`)."""
    raise NotImplementedError("see leaven_python.md — Task and Case / assets")
