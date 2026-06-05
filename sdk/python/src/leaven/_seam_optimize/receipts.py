"""Private receipt projection for durable-seam optimize reports."""

from dataclasses import dataclass

from msgspec import UNSET, UnsetType

from .._receipts import CallReceipt, WriteReceipt
from .._seam._wire.payloads import (
    BlobRef as WireBlobRef,
)
from .._seam._wire.payloads import (
    ReceiptRefRecord,
    StageEffectReceipt,
    StageRunResult,
)
from ..blob_ref import BlobRef


@dataclass(frozen=True)
class EffectCostTotals:
    """Metered cost and LM-token totals from stage effect receipts."""

    cost_usd: float
    lm_tokens: int


@dataclass(frozen=True)
class EffectBlobContent:
    """Private byte content attached to an effect receipt blob ref."""

    receipt_id: str
    blob_ref: BlobRef
    content_base64: str


def effect_receipts_from_stage_result(result: StageRunResult) -> list[CallReceipt]:
    """Extract opaque effect receipts from private worker stage metadata."""
    if result.effect_receipts is UNSET:
        return []
    return [
        CallReceipt(
            receipt_id=value.receipt,
            blob_refs=_blob_refs_from_receipt(value),
        )
        for value in result.effect_receipts
        if value.receipt
    ]


def effect_blob_contents_from_stage_result(result: StageRunResult) -> list[EffectBlobContent]:
    """Extract callback blob contents that must be materialized by Rust."""
    if result.effect_receipts is UNSET:
        return []
    contents: list[EffectBlobContent] = []
    for receipt in result.effect_receipts:
        if receipt.blob_contents is UNSET:
            continue
        contents.extend(
            EffectBlobContent(
                receipt_id=receipt.receipt,
                blob_ref=_blob_ref(content.blob_ref),
                content_base64=content.content_base64,
            )
            for content in receipt.blob_contents
        )
    return contents


def effect_cost_totals_from_stage_result(result: StageRunResult) -> EffectCostTotals:
    """Extract metered usage totals from private worker stage metadata."""
    usd_micro = 0
    lm_tokens = 0
    if result.effect_receipts is UNSET:
        return EffectCostTotals(cost_usd=0.0, lm_tokens=0)
    for value in result.effect_receipts:
        cost = value.cost
        if cost is UNSET:
            continue
        usd_micro += _nonnegative_int(cost.usd_micro)
        lm_tokens += _nonnegative_int(cost.input_tokens)
        lm_tokens += _nonnegative_int(cost.output_tokens)
    return EffectCostTotals(cost_usd=usd_micro / 1_000_000, lm_tokens=lm_tokens)


def proposal_receipts_from_stage_result(result: StageRunResult) -> list[WriteReceipt]:
    """Extract proposal write receipts from private worker stage metadata."""
    receipts = []
    if result.proposal_receipts is UNSET:
        return receipts
    for value in result.proposal_receipts:
        receipt = _receipt_id(value.receipt)
        if not receipt:
            continue
        receipts.append(
            WriteReceipt(
                receipt_id=receipt,
                proposal_ids=list(value.proposal_ids) if value.proposal_ids is not UNSET else [],
            )
        )
    return receipts


def sum_effect_cost_totals(values: list[EffectCostTotals]) -> EffectCostTotals:
    """Sum per-stage cost totals."""
    return EffectCostTotals(
        cost_usd=sum(value.cost_usd for value in values),
        lm_tokens=sum(value.lm_tokens for value in values),
    )


def _nonnegative_int(value: int | UnsetType) -> int:
    if value is UNSET:
        return 0
    if value < 0:
        raise ValueError("stage effect cost values must be nonnegative")
    return value


def _blob_refs_from_receipt(value: StageEffectReceipt) -> list[BlobRef]:
    if value.blob_refs is UNSET:
        return []
    return [_blob_ref(ref) for ref in value.blob_refs]


def _blob_ref(ref: WireBlobRef) -> BlobRef:
    return BlobRef(
        blob_id=ref.id,
        sha256=ref.sha256,
        bytes=ref.bytes,
        data_classes=list(ref.data_classes),
    )


def _receipt_id(value: str | ReceiptRefRecord) -> str:
    if isinstance(value, str):
        return value
    return value.id


__all__ = [
    "EffectBlobContent",
    "EffectCostTotals",
    "effect_blob_contents_from_stage_result",
    "effect_cost_totals_from_stage_result",
    "effect_receipts_from_stage_result",
    "proposal_receipts_from_stage_result",
    "sum_effect_cost_totals",
]
