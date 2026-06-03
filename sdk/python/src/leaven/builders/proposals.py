"""`cx.proposals.*` — submit + apply proposal batches from proposer stages."""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

from .._receipts import WriteReceipt
from ..proposal import ProposalBatch


class ProposalSubmission(BaseModel):
    """Result of a proposal submission. Apply is a separate call."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    receipt: WriteReceipt
    proposal_ids: list[str]


class ProposalsBuilder:
    """Proposal submission bound to a context."""

    async def submit(self, batch: ProposalBatch) -> ProposalSubmission:
        """Submit a proposal batch. Engine validates against capability + surface."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def apply(self, submission: ProposalSubmission) -> WriteReceipt:
        """Ask the engine to apply a previously-submitted proposal batch."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def submit_and_apply(self, batch: ProposalBatch) -> WriteReceipt:
        """Convenience: submit + apply in one round-trip."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["ProposalSubmission", "ProposalsBuilder"]
