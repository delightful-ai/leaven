"""Receipt handle types — opaque references to wire receipts.

Receipts are audit currency, not log decoration. The Rust engine mints them;
the Python side carries opaque handles and passes them back into evidence
envelopes / assessment writes to bind effects to their source operations.

Python users never construct receipts directly. High-level authoring surfaces
hide them; advanced evaluator/proposer code may receive opaque handles on
effect/query results and pass them into low-level evidence envelopes.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class _ReceiptBase(BaseModel):
    """Base for all opaque receipt handles. Equality is by content hash."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    receipt_id: str
    """Opaque receipt id; structure is engine-defined."""


class QueryReceipt(_ReceiptBase):
    """Receipt for a graph or case query operation."""


class CallReceipt(_ReceiptBase):
    """Receipt for a costful effect call (LM, agent, sandbox)."""


class WriteReceipt(_ReceiptBase):
    """Receipt for a graph mutation (proposal apply, assessment submit)."""


__all__ = ["CallReceipt", "QueryReceipt", "WriteReceipt"]
