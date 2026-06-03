"""Private callback receipt capture for Python command workers."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, cast

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
    blob_refs: list[dict[str, Any]] | None = None

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
        if self.blob_refs is not None:
            value["blob_refs"] = self.blob_refs
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
                blob_refs=_matching_blob_refs(result, receipt),
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


def _matching_blob_refs(result: dict[str, Any], receipt: str) -> list[dict[str, Any]] | None:
    primary = result.get("primary")
    if not isinstance(primary, dict) or primary.get("receipt") != receipt:
        return None
    refs = []
    transcript_ref = _blob_ref(primary.get("transcript_ref"))
    if transcript_ref is not None:
        refs.append(transcript_ref)
    for command in primary.get("commands", []):
        if not isinstance(command, dict):
            continue
        for key in ("stdout_ref", "stderr_ref"):
            ref = _blob_ref(command.get(key))
            if ref is not None:
                refs.append(ref)
    return refs or None


def _blob_ref(value: object) -> dict[str, Any] | None:
    if not isinstance(value, dict):
        return None
    blob = cast("dict[str, Any]", value)
    blob_id = blob.get("id")
    if not isinstance(blob_id, str) or not blob_id:
        return None
    ref: dict[str, Any] = {"kind": "blob_ref", "id": blob_id}
    sha256 = blob.get("sha256")
    if isinstance(sha256, str):
        ref["sha256"] = sha256
    byte_count = blob.get("bytes")
    if isinstance(byte_count, int):
        ref["bytes"] = byte_count
    data_classes = blob.get("data_classes")
    if isinstance(data_classes, list):
        ref["data_classes"] = [item for item in data_classes if isinstance(item, str)]
    return ref


def _is_effect_receipt(record: CallbackReceipt) -> bool:
    if record.method == "leaven/lm.complete":
        return record.receipt_id.startswith("lmrec_")
    if record.method == "leaven/agent.run":
        return record.receipt_id.startswith("agentrec_")
    return False


__all__ = ["CallbackReceiptLog"]
