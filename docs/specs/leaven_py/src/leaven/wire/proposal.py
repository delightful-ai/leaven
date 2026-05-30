"""Wire records: `ProposalEffect` / `ProposalBatch`.

`ProposalEffect` is the create/change effect carried by `lv.Proposal`.
Governing spec: `docs/specs/leaven_python.md`. Schema owned by
`docs/specs/public-seam-v1/schemas/`.
"""

from __future__ import annotations

from collections.abc import Sequence
from enum import StrEnum

from pydantic import BaseModel, ConfigDict

__all__ = ["ProposalBatch", "ProposalEffect"]


class ProposalEffect(StrEnum):
    """Proposal lineage effect: fresh authored vs change of existing candidate."""

    create = "create"
    change = "change"


class ProposalBatch(BaseModel):
    """A batch of proposals with a shared effect."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    proposals: Sequence[object]
    effect: ProposalEffect
