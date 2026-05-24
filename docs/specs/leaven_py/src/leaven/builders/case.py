"""`cx.case.*` — case loading and queries."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Literal

from ..case import Case

CaseField = Literal["input", "target", "metadata", "target_ref"]


class CaseBuilder:
    """Case loader bound to a context. Reads are receipted."""

    async def load(
        self,
        case_id: str,
        *,
        include: Sequence[CaseField] = ("input", "metadata"),
    ) -> Case:
        """Load a case by id.

        `include` controls projection. By default `target` is NOT included
        (target-safe read for runners/reflectors). Evaluators/scorers/judges
        include `target` explicitly when needed; the seam enforces that the
        capability authorizes it.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def load_batch(
        self,
        case_ids: Sequence[str],
        *,
        include: Sequence[CaseField] = ("input", "metadata"),
    ) -> list[Case]:
        """Load multiple cases in one round-trip."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["CaseBuilder", "CaseField"]
