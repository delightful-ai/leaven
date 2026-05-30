"""Wire record: evidence visibility projection class.

Governing spec: `docs/specs/leaven_python.md` — What is preserved / Evidence
visibility. Schema owned by `docs/specs/public-seam-v1/schemas/`.
"""

from __future__ import annotations

from enum import StrEnum

__all__ = ["Visibility"]


class Visibility(StrEnum):
    """Public / private / trace projection data classes."""

    public = "public"
    private = "private"
    trace = "trace"
