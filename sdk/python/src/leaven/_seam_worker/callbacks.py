"""Private callback receipt capture for Python command workers."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

EFFECT_CALLBACK_METHODS = frozenset({"leaven/lm.complete", "leaven/agent.run"})
PROPOSAL_CALLBACK_METHOD = "leaven/proposal.submit_batch"


@dataclass(frozen=True)
class CallbackReceipt:
    """One receipt reported by a nested callback result."""

    method: str
    receipt_id: str
    call_kind: str | None = None
    write_kind: str | None = None
    cost: dict[str, Any] | None = None
    proposal_ids: list[str] | None = None

    def to_json(self) -> dict[str, Any]:
        value: dict[str, Any] = {"method": self.method, "receipt": self.receipt_id}
        if self.call_kind is not None:
            value["call_kind"] = self.call_kind
        if self.write_kind is not None:
            value["write_kind"] = self.write_kind
        if self.cost is not None:
            value["cost"] = self.cost
        if self.proposal_ids is not None:
            value["proposal_ids"] = self.proposal_ids
        return value


class CallbackReceiptLog:
    """Append-only receipt log for one stage invocation."""

    def __init__(self) -> None:
        self._records: list[CallbackReceipt] = []

    def record_result(self, *, method: str, result: dict[str, Any]) -> None:
        """Capture callback receipts carried by one public-seam callback result."""
        self._records.extend(_receipts_from_result(method=method, result=result))

    def effect_receipts_json(self) -> list[dict[str, Any]]:
        """Return effect-call receipts safe to attach to a private stage result."""
        return [
            record.to_json()
            for record in self._records
            if record.method in EFFECT_CALLBACK_METHODS and _is_effect_receipt(record)
        ]

    def proposal_receipts_json(self) -> list[dict[str, Any]]:
        """Return proposal-write receipts observed during a proposer stage."""
        return [
            record.to_json()
            for record in self._records
            if record.method == PROPOSAL_CALLBACK_METHOD
        ]


def _receipts_from_result(
    *,
    method: str,
    result: dict[str, Any],
) -> list[CallbackReceipt]:
    records = []
    for value in result.get("receipts", []):
        if not isinstance(value, dict):
            continue
        receipt = value.get("receipt")
        if not isinstance(receipt, str) or not receipt:
            continue
        call_kind = value.get("call_kind")
        write_kind = value.get("write_kind")
        records.append(
            CallbackReceipt(
                method=method,
                receipt_id=receipt,
                call_kind=call_kind if isinstance(call_kind, str) else None,
                write_kind=write_kind if isinstance(write_kind, str) else None,
                cost=_matching_primary_cost(result, receipt),
                proposal_ids=_matching_proposal_ids(result, receipt),
            )
        )
    return records


def _matching_primary_cost(result: dict[str, Any], receipt: str) -> dict[str, Any] | None:
    primary = result.get("primary")
    if not isinstance(primary, dict) or primary.get("receipt") != receipt:
        return None
    cost = primary.get("cost")
    if not isinstance(cost, dict):
        return None
    return dict(cost)


def _matching_proposal_ids(result: dict[str, Any], receipt: str) -> list[str] | None:
    primary = result.get("primary")
    if not isinstance(primary, dict) or primary.get("receipt") != receipt:
        return None
    proposal_ids = primary.get("proposal_ids")
    if not isinstance(proposal_ids, list):
        return None
    return [proposal_id for proposal_id in proposal_ids if isinstance(proposal_id, str)]


def _is_effect_receipt(record: CallbackReceipt) -> bool:
    if record.method == "leaven/lm.complete":
        return record.receipt_id.startswith("lmrec_")
    if record.method == "leaven/agent.run":
        return record.receipt_id.startswith("agentrec_")
    return False


__all__ = ["CallbackReceiptLog"]
