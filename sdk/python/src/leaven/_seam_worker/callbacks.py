"""Private callback receipt capture for Python command workers."""

from dataclasses import dataclass

from .._seam._wire import JsonObject, JsonValue
from .._seam._wire.json_value import json_object, json_value

EFFECT_CALLBACK_METHODS = frozenset({"leaven/lm.complete", "leaven/agent.run"})
PROPOSAL_CALLBACK_METHOD = "leaven/proposal.submit_batch"


@dataclass(frozen=True)
class CallbackReceipt:
    """One receipt reported by a nested callback result."""

    method: str
    receipt_id: str
    call_kind: str | None = None
    write_kind: str | None = None
    cost: JsonObject | None = None
    proposal_ids: list[str] | None = None
    blob_refs: list[JsonObject] | None = None

    def to_json(self) -> JsonObject:
        value: JsonObject = {"method": self.method, "receipt": self.receipt_id}
        if self.call_kind is not None:
            value["call_kind"] = self.call_kind
        if self.write_kind is not None:
            value["write_kind"] = self.write_kind
        if self.cost is not None:
            value["cost"] = self.cost
        if self.proposal_ids is not None:
            value["proposal_ids"] = json_value(self.proposal_ids)
        if self.blob_refs is not None:
            value["blob_refs"] = json_value(self.blob_refs)
        return value


class CallbackReceiptLog:
    """Append-only receipt log for one stage invocation."""

    def __init__(self) -> None:
        self._records: list[CallbackReceipt] = []

    def record_result(self, *, method: str, result: JsonObject) -> None:
        """Capture callback receipts carried by one public-seam callback result."""
        self._records.extend(_receipts_from_result(method=method, result=result))

    def effect_receipts_json(self) -> list[JsonObject]:
        """Return effect-call receipts safe to attach to a private stage result."""
        return [
            record.to_json()
            for record in self._records
            if record.method in EFFECT_CALLBACK_METHODS and _is_effect_receipt(record)
        ]

    def proposal_receipts_json(self) -> list[JsonObject]:
        """Return proposal-write receipts observed during a proposer stage."""
        return [
            record.to_json()
            for record in self._records
            if record.method == PROPOSAL_CALLBACK_METHOD
        ]


def _receipts_from_result(
    *,
    method: str,
    result: JsonObject,
) -> list[CallbackReceipt]:
    records = []
    for value in _json_array(result.get("receipts")):
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


def _matching_primary_cost(result: JsonObject, receipt: str) -> JsonObject | None:
    primary = result.get("primary")
    if not isinstance(primary, dict) or primary.get("receipt") != receipt:
        return None
    cost = primary.get("cost")
    if not isinstance(cost, dict):
        return None
    return json_object(cost)


def _matching_proposal_ids(result: JsonObject, receipt: str) -> list[str] | None:
    primary = result.get("primary")
    if not isinstance(primary, dict) or primary.get("receipt") != receipt:
        return None
    proposal_ids = primary.get("proposal_ids")
    if not isinstance(proposal_ids, list):
        return None
    return [proposal_id for proposal_id in proposal_ids if isinstance(proposal_id, str)]


def _matching_blob_refs(result: JsonObject, receipt: str) -> list[JsonObject] | None:
    primary = result.get("primary")
    if not isinstance(primary, dict) or primary.get("receipt") != receipt:
        return None
    refs = []
    transcript_ref = _blob_ref(primary.get("transcript_ref"))
    if transcript_ref is not None:
        refs.append(transcript_ref)
    for command in _json_array(primary.get("commands")):
        if not isinstance(command, dict):
            continue
        for key in ("stdout_ref", "stderr_ref"):
            ref = _blob_ref(command.get(key))
            if ref is not None:
                refs.append(ref)
    return refs or None


def _blob_ref(value: JsonValue | object) -> JsonObject | None:
    if not isinstance(value, dict):
        return None
    blob = json_object(value)
    blob_id = blob.get("id")
    if not isinstance(blob_id, str) or not blob_id:
        return None
    ref: JsonObject = {"kind": "blob_ref", "id": blob_id}
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


def _json_array(value: JsonValue | None) -> list[JsonValue]:
    if isinstance(value, list):
        return value
    return []


def _is_effect_receipt(record: CallbackReceipt) -> bool:
    if record.method == "leaven/lm.complete":
        return record.receipt_id.startswith("lmrec_")
    if record.method == "leaven/agent.run":
        return record.receipt_id.startswith("agentrec_")
    return False


__all__ = ["CallbackReceiptLog"]
