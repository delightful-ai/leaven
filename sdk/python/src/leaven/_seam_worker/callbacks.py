"""Private callback receipt capture for Python command workers."""

from dataclasses import dataclass
from typing import Literal

from msgspec import UNSET, UnsetType

from .._seam._wire.payloads import (
    BlobRef,
    CallReceiptKind,
    Cost,
    ReceiptRef,
    ReceiptRefRecord,
    StageCost,
    StageEffectBlobContent,
    StageEffectReceipt,
    StageProposalReceipt,
    WriteReceiptKind,
)
from .._seam._wire.results import AgentRunResult, LmCompleteResult, ProposalSubmitResult

EFFECT_CALLBACK_METHODS = frozenset({"leaven/lm.complete", "leaven/agent.run"})
PROPOSAL_CALLBACK_METHOD = "leaven/proposal.submit_batch"

type CallbackResult = LmCompleteResult | AgentRunResult | ProposalSubmitResult
type EffectCallbackMethod = Literal["leaven/lm.complete", "leaven/agent.run"]
type EffectCallKind = Literal["lm_complete", "agent_run"]


@dataclass(frozen=True)
class CallbackReceipt:
    """One receipt reported by a nested callback result."""

    method: str
    receipt_id: str
    call_kind: CallReceiptKind | None = None
    write_kind: WriteReceiptKind | None = None
    cost: Cost | None = None
    proposal_ids: list[str] | None = None
    blob_refs: list[BlobRef] | None = None
    blob_contents: list[StageEffectBlobContent] | None = None


class CallbackReceiptLog:
    """Append-only receipt log for one stage invocation."""

    def __init__(self) -> None:
        self._records: list[CallbackReceipt] = []

    def record_result(self, result: CallbackResult) -> None:
        """Capture callback receipts carried by one public-seam callback result."""
        self._records.extend(_receipts_from_result(result))

    def effect_receipts(self) -> list[StageEffectReceipt]:
        """Return effect-call receipts safe to attach to a private stage result."""
        receipts: list[StageEffectReceipt] = []
        for record in self._records:
            receipt = _effect_receipt(record)
            if receipt is not None:
                receipts.append(receipt)
        return receipts

    def proposal_receipts(self) -> list[StageProposalReceipt]:
        """Return proposal-write receipts observed during a proposer stage."""
        receipts: list[StageProposalReceipt] = []
        for record in self._records:
            receipt = _proposal_receipt(record)
            if receipt is not None:
                receipts.append(receipt)
        return receipts


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


def _effect_receipt(record: CallbackReceipt) -> StageEffectReceipt | None:
    if not _is_effect_receipt(record):
        return None
    return StageEffectReceipt(
        method=_effect_method(record.method),
        receipt=record.receipt_id,
        call_kind=_effect_call_kind(record.call_kind),
        cost=_stage_cost(record.cost),
        blob_refs=record.blob_refs if record.blob_refs is not None else UNSET,
        blob_contents=record.blob_contents if record.blob_contents is not None else UNSET,
    )


def _proposal_receipt(record: CallbackReceipt) -> StageProposalReceipt | None:
    if record.method != PROPOSAL_CALLBACK_METHOD:
        return None
    return StageProposalReceipt(
        method="leaven/proposal.submit_batch",
        receipt=record.receipt_id,
        write_kind=_proposal_write_kind(record.write_kind),
        proposal_ids=record.proposal_ids if record.proposal_ids is not None else UNSET,
    )


def _effect_method(method: str) -> EffectCallbackMethod:
    if method == "leaven/lm.complete":
        return "leaven/lm.complete"
    if method == "leaven/agent.run":
        return "leaven/agent.run"
    raise TypeError(f"unsupported effect callback method: {method!r}")


def _effect_call_kind(kind: CallReceiptKind | None) -> EffectCallKind | UnsetType:
    if kind is None:
        return UNSET
    if kind == "lm_complete":
        return "lm_complete"
    if kind == "agent_run":
        return "agent_run"
    raise TypeError(f"unsupported effect callback call kind: {kind!r}")


def _proposal_write_kind(kind: WriteReceiptKind | None) -> Literal["submit_proposal_batch"] | UnsetType:
    if kind is None:
        return UNSET
    if kind == "submit_proposal_batch":
        return "submit_proposal_batch"
    raise TypeError(f"unsupported proposal callback write kind: {kind!r}")


def _stage_cost(cost: Cost | None) -> StageCost | UnsetType:
    if cost is None:
        return UNSET
    if cost.agent_calls is not UNSET:
        raise TypeError("stage effect receipt cost cannot carry agent_calls")
    if cost.sandbox_calls is not UNSET:
        raise TypeError("stage effect receipt cost cannot carry sandbox_calls")
    if cost.metric_calls is not UNSET:
        raise TypeError("stage effect receipt cost cannot carry metric_calls")
    if cost.wall_ms is not UNSET:
        raise TypeError("stage effect receipt cost cannot carry wall_ms")
    return StageCost(
        usd_micro=cost.usd_micro,
        input_tokens=cost.input_tokens,
        output_tokens=cost.output_tokens,
        lm_calls=cost.lm_calls,
    )


def _is_effect_receipt(record: CallbackReceipt) -> bool:
    if record.method == "leaven/lm.complete":
        return record.receipt_id.startswith("lmrec_")
    if record.method == "leaven/agent.run":
        return record.receipt_id.startswith("agentrec_")
    return False


__all__ = ["CallbackReceiptLog"]
