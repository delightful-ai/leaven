"""Wire records: receipts — query / call / write audit currency.

Receipts bind request/result hashes, operation kind, timing, policy
fingerprint, and revision. Governing spec: `docs/specs/leaven_python.md` —
Receipts as audit currency. Schema owned by
`docs/specs/public-seam-v1/schemas/`.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

__all__ = ["CallReceipt", "QueryReceipt", "WriteReceipt"]


class QueryReceipt(BaseModel):
    """Receipt for a read/query operation."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    request_hash: str
    result_hash: str
    kind: str
    revision: str
    policy_fingerprint: str
    elapsed_ms: int = 0


class CallReceipt(BaseModel):
    """Receipt for a costful call (LM/agent/sandbox); failed costs still count."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    request_hash: str
    result_hash: str
    kind: str
    revision: str
    policy_fingerprint: str
    cost_usd: float = 0.0
    elapsed_ms: int = 0


class WriteReceipt(BaseModel):
    """Receipt for a graph-mutating write operation."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    request_hash: str
    result_hash: str
    kind: str
    revision: str
    policy_fingerprint: str
    elapsed_ms: int = 0
