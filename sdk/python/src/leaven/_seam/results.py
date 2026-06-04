"""Typed private result records for public-seam extension method calls."""

from typing import Literal

from msgspec import UNSET, Struct, UnsetType

from ._wire import JsonObject
from ._wire.payloads import (
    BlobRef,
    CallReceiptKind,
    Cost,
    OperationReceiptKind,
    PlanResultStatus,
    ReceiptRef,
    Replayability,
    WriteReceiptKind,
)


class ResultReceipt(Struct, frozen=True, omit_defaults=True):
    """Typed receipt projection carried by extension method results."""

    kind: OperationReceiptKind
    receipt: ReceiptRef
    status: PlanResultStatus
    result_hash: str
    call_kind: CallReceiptKind | UnsetType = UNSET
    write_kind: WriteReceiptKind | UnsetType = UNSET
    cost: Cost | UnsetType = UNSET
    proposal_ids: list[str] | UnsetType = UNSET


class ResultRedaction(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    """Public redaction fact attached to an extension method result."""

    path: str
    reason: str
    policy_fingerprint: str | UnsetType = UNSET
    public_reason: str | UnsetType = UNSET
    audit_reason: str | UnsetType = UNSET


class AgentCommandRecord(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    """One command audited inside an `agent_session` primary."""

    argv: list[str]
    status: str
    receipt: str | UnsetType = UNSET


class AgentSessionPrimary(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    """Primary value for `leaven/agent.run`."""

    kind: Literal["agent_session"]
    status: str
    receipt: str
    commands: list[AgentCommandRecord]
    transcript_ref: BlobRef | UnsetType = UNSET
    cost: Cost | UnsetType = UNSET


class AgentRunResult(Struct, frozen=True, forbid_unknown_fields=True):
    """Typed ACP extension result for `leaven/agent.run`."""

    method: Literal["leaven/agent.run"]
    primary: AgentSessionPrimary
    receipts: list[ResultReceipt]
    redactions: list[ResultRedaction]
    capability_fingerprint: str
    policy_fingerprint: str
    data_classes: list[str]


class LmContentPart(Struct, frozen=True, forbid_unknown_fields=True):
    """One public LM message content part."""

    kind: Literal["text"]
    text: str


class LmMessageRecord(Struct, frozen=True, forbid_unknown_fields=True):
    """Assistant message returned by `leaven/lm.complete`."""

    role: str
    content: list[LmContentPart]


class LmResponsePrimary(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    """Primary value for `leaven/lm.complete`."""

    kind: Literal["lm_response"]
    message: LmMessageRecord
    receipt: str
    cost: Cost | UnsetType = UNSET


class LmCompleteResult(Struct, frozen=True, forbid_unknown_fields=True):
    """Typed ACP extension result for `leaven/lm.complete`."""

    method: Literal["leaven/lm.complete"]
    primary: LmResponsePrimary
    receipts: list[ResultReceipt]
    redactions: list[ResultRedaction]
    capability_fingerprint: str
    policy_fingerprint: str
    data_classes: list[str]


class ProposalBatchPrimary(Struct, frozen=True, forbid_unknown_fields=True):
    """Primary value for `leaven/proposal.submit_batch`."""

    kind: Literal["proposal_batch_receipt"]
    batch_id: str
    proposal_ids: list[str]
    status: str
    receipt: str


class ProposalSubmitResult(Struct, frozen=True, forbid_unknown_fields=True):
    """Typed ACP extension result for `leaven/proposal.submit_batch`."""

    method: Literal["leaven/proposal.submit_batch"]
    primary: ProposalBatchPrimary
    receipts: list[ResultReceipt]
    redactions: list[ResultRedaction]
    capability_fingerprint: str
    policy_fingerprint: str
    data_classes: list[str]


class CaseRecordPrimary(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    """Primary value for case read methods."""

    kind: Literal["case_record"]
    case: ReceiptRef
    receipt: str
    data_classes: list[str]
    replayability: Replayability
    input: JsonObject | UnsetType = UNSET
    target: JsonObject | UnsetType = UNSET
    metadata: JsonObject | UnsetType = UNSET


class CaseLoadResult(Struct, frozen=True, forbid_unknown_fields=True):
    """Typed ACP extension result for case read methods."""

    method: Literal[
        "leaven/case.load", "leaven/case.input", "leaven/case.target", "leaven/case.metadata"
    ]
    primary: CaseRecordPrimary
    receipts: list[ResultReceipt]
    redactions: list[ResultRedaction]
    capability_fingerprint: str
    policy_fingerprint: str
    data_classes: list[str]


__all__ = [
    "AgentCommandRecord",
    "AgentRunResult",
    "AgentSessionPrimary",
    "CaseLoadResult",
    "CaseRecordPrimary",
    "LmCompleteResult",
    "LmContentPart",
    "LmMessageRecord",
    "LmResponsePrimary",
    "ProposalBatchPrimary",
    "ProposalSubmitResult",
    "ResultReceipt",
    "ResultRedaction",
]
