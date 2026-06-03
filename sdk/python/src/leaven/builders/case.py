"""`cx.case.*` — case loading and queries."""

from collections.abc import Sequence
from typing import Literal

from ..case import Case

CaseField = Literal["input", "target", "metadata", "files", "setup", "sandbox", "split"]


class CaseBuilder:
    """Case loader bound to a context.

    Returned `Case` values are ordinary user-facing records. The engine still
    records read receipts internally for audit/replay.
    """

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
