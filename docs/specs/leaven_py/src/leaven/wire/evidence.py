"""Wire records: evidence envelopes (`EvidenceEnvelope` / `Public` / `Private`).

Governing spec: `docs/specs/leaven_python.md` — What is preserved / Evidence
visibility. Schema owned by `docs/specs/public-seam-v1/schemas/`.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

from .visibility import Visibility

__all__ = ["EvidenceEnvelope", "EvidencePrivate", "EvidencePublic"]


class EvidencePublic(BaseModel):
    """Public-projection evidence payload."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)


class EvidencePrivate(BaseModel):
    """Private-projection evidence payload (not externally visible)."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)


class EvidenceEnvelope(BaseModel):
    """Visibility-tagged evidence envelope; target-derived flagged honestly."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    visibility: Visibility
    public: EvidencePublic | None = None
    private: EvidencePrivate | None = None
    target_derived: bool = False
