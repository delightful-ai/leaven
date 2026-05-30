"""Wire record: `OutputRecord` — engine-side output projection.

Governing spec: `docs/specs/leaven_python.md` — Public API discipline (wire
ring). Schema owned by `docs/specs/public-seam-v1/schemas/`.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

from .visibility import Visibility

__all__ = ["OutputRecord"]


class OutputRecord(BaseModel):
    """Engine-side projection of a produced output. Floorboard creature."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: str
    path: str | None = None
    visibility: Visibility = Visibility.public
