"""Private callback receipt capture for Python command workers."""

from dataclasses import dataclass

from msgspec import UNSET

from .._seam._wire import JsonObject
from .._seam._wire.json_value import json_value
from .._seam._wire.payloads import (
    BlobRef,
    Cost,
    ReceiptRef,
    ReceiptRefRecord,
    StageEffectBlobContent,
)
from .._seam._wire.results import AgentRunResult, LmCompleteResult, ProposalSubmitResult

EFFECT_CALLBACK_METHODS = frozenset({"leaven/lm.complete", "leaven/agent.run"})
PROPOSAL_CALLBACK_METHOD = "leaven/proposal.submit_batch"

type CallbackResult = LmCompleteResult | AgentRunResult | ProposalSubmitResult


@dataclass(frozen=True)
class CallbackReceipt:
    """One receipt reported by a nested callback result."""

    method: str
    receipt_id: str
    call_kind: str | None = None
    write_kind: str | None = None
    cost: Cost | None = None
    proposal_ids: list[str] | None = None
    blob_refs: list[BlobRef] | None = None
    blob_contents: list[StageEffectBlobContent] | None = None

    def to_json(self) -> JsonObject:
        value: JsonObject = {"method": self.method, "receipt": self.receipt_id}
        if self.call_kind is not None:
            value["call_kind"] = self.call_kind
        if self.write_kind is not None:
            value["write_kind"] = self.write_kind
        if self.cost is not None:
            value["cost"] = _cost_json(self.cost)
        if self.proposal_ids is not None:
            value["proposal_ids"] = json_value(self.proposal_ids)
        if self.blob_refs is not None:
            value["blob_refs"] = json_value([_blob_ref_json(ref) for ref in self.blob_refs])
        if self.blob_contents is not None:
            value["blob_contents"] = json_value(
                [_blob_content_json(content) for content in self.blob_contents]
            )
        return value


class CallbackReceiptLog:
    """Append-only receipt log for one stage invocation."""

    def __init__(self) -> None:
        self._records: list[CallbackReceipt] = []

    def record_result(self, result: CallbackResult) -> None:
        """Capture callback receipts carried by one public-seam callback result."""
        self._records.extend(_receipts_from_result(result))

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


def _receipts_from_result(result: CallbackResult) -> list[CallbackReceipt]:
    records = []
    for value in result.receipts:
        receipt = _receipt_id(value.receipt)
        if not receipt:
            continue
        records.append(
            CallbackReceipt(
                method=result.method,
                receipt_id=receipt,
                call_kind=value.call_kind if value.call_kind is not UNSET else None,
                write_kind=value.write_kind if value.write_kind is not UNSET else None,
                cost=_matching_primary_cost(result, receipt),
                proposal_ids=_matching_proposal_ids(result, receipt),
                blob_refs=_matching_blob_refs(result, receipt),
                blob_contents=_matching_blob_contents(result, receipt),
            )
        )
    return records


def _matching_primary_cost(result: CallbackResult, receipt: str) -> Cost | None:
    if isinstance(result, ProposalSubmitResult):
        return None
    primary = result.primary
    if primary.receipt != receipt or primary.cost is UNSET:
        return None
    return primary.cost


def _matching_proposal_ids(result: CallbackResult, receipt: str) -> list[str] | None:
    if not isinstance(result, ProposalSubmitResult):
        return None
    primary = result.primary
    if primary.receipt != receipt:
        return None
    return list(primary.proposal_ids)


def _matching_blob_refs(result: CallbackResult, receipt: str) -> list[BlobRef] | None:
    if not isinstance(result, AgentRunResult):
        return None
    primary = result.primary
    if primary.receipt != receipt:
        return None
    if primary.transcript_ref is UNSET:
        return None
    return [primary.transcript_ref]


def _matching_blob_contents(
    result: CallbackResult,
    receipt: str,
) -> list[StageEffectBlobContent] | None:
    if not isinstance(result, AgentRunResult):
        return None
    primary = result.primary
    if primary.receipt != receipt:
        return None
    if primary.transcript_ref is UNSET or primary.transcript_content_base64 is UNSET:
        return None
    return [
        StageEffectBlobContent(
            blob_ref=primary.transcript_ref,
            content_base64=primary.transcript_content_base64,
        )
    ]


def _receipt_id(value: ReceiptRef) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, ReceiptRefRecord):
        return value.id
    raise TypeError(f"unsupported receipt ref: {value!r}")


def _cost_json(cost: Cost) -> JsonObject:
    value: JsonObject = {}
    if cost.usd_micro is not UNSET:
        value["usd_micro"] = cost.usd_micro
    if cost.input_tokens is not UNSET:
        value["input_tokens"] = cost.input_tokens
    if cost.output_tokens is not UNSET:
        value["output_tokens"] = cost.output_tokens
    if cost.lm_calls is not UNSET:
        value["lm_calls"] = cost.lm_calls
    if cost.agent_calls is not UNSET:
        value["agent_calls"] = cost.agent_calls
    if cost.sandbox_calls is not UNSET:
        value["sandbox_calls"] = cost.sandbox_calls
    if cost.metric_calls is not UNSET:
        value["metric_calls"] = cost.metric_calls
    if cost.wall_ms is not UNSET:
        value["wall_ms"] = cost.wall_ms
    return value


def _blob_ref_json(blob: BlobRef) -> JsonObject:
    value: JsonObject = {
        "kind": "blob_ref",
        "id": blob.id,
        "sha256": blob.sha256,
        "bytes": blob.bytes,
        "data_classes": list(blob.data_classes),
    }
    if blob.media_type is not UNSET:
        value["media_type"] = blob.media_type
    if blob.uri is not UNSET:
        value["uri"] = blob.uri
    return value


def _blob_content_json(content: StageEffectBlobContent) -> JsonObject:
    return {
        "blob_ref": _blob_ref_json(content.blob_ref),
        "content_base64": content.content_base64,
    }


def _is_effect_receipt(record: CallbackReceipt) -> bool:
    if record.method == "leaven/lm.complete":
        return record.receipt_id.startswith("lmrec_")
    if record.method == "leaven/agent.run":
        return record.receipt_id.startswith("agentrec_")
    return False


__all__ = ["CallbackReceiptLog"]
