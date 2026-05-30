"""`Artifact` base marker for all artifact adapters.

Governing spec: `docs/specs/leaven_python.md` — Artifact adapters.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

__all__ = ["Artifact"]


class Artifact(BaseModel):
    """Base marker for all artifact adapters; `kind` discriminates."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    kind: str
