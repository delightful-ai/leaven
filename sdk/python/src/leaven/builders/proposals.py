"""`cx.proposals.*` — submit + apply proposal batches from proposer stages."""

import asyncio
import json
from typing import Protocol

from pydantic import BaseModel, ConfigDict

from .._errors import UnboundBuilderError
from .._json_parse import parse_json_value
from .._receipts import WriteReceipt
from .._seam import ProposalApplyRequest, ProposalSubmitRequest
from .._seam._wire.expressions import PlanExpressionLiteral, ValueExprLiteral
from .._seam._wire.refs import ReceiptRef
from .._seam._wire.results import ProposalApplyResult, ProposalSubmitResult
from .._seam._wire.writes import (
    ProposalCausalInputs,
    ProposalEffectAgentSession,
    ProposalEffectChange,
    ProposalEffectCreate,
    ProposalWriteRecord,
)
from ..artifacts.directory import DirectoryArtifact
from ..artifacts.prompt import PromptArtifact, PromptTemplateChange
from ..artifacts.skill_bank import SkillBank, SkillBankChangeRecord
from ..json_value import JsonValue
from ..proposal import ProposalArtifactValue, ProposalBatch, ProposalChangeValue, ProposalEffect


class ProposalSubmission(BaseModel):
    """Result of a proposal submission. Apply is a separate call."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    receipt: WriteReceipt
    batch_id: str
    proposal_ids: list[str]


class _ProposalSubmitter(Protocol):
    """Small private protocol ProposalsBuilder needs for proposal submission."""

    def proposal_submit(self, request: ProposalSubmitRequest) -> ProposalSubmitResult: ...


class _ProposalApplier(Protocol):
    """Small private protocol ProposalsBuilder needs for proposal application."""

    def proposal_apply(self, request: ProposalApplyRequest) -> ProposalApplyResult: ...


class ProposalsBuilder:
    """Proposal submission bound to a context."""

    def __init__(
        self,
        *,
        _client: "_ProposalSubmitter | None" = None,
        _apply_client: "_ProposalApplier | None" = None,
        _idempotency_prefix: str = "proposal-builder",
        _plan_id: str = "planpythonproposalbuilder001",
    ) -> None:
        self._client = _client
        self._apply_client = _apply_client
        self._idempotency_prefix = _idempotency_prefix
        self._plan_id = _plan_id

    @classmethod
    def _for_seam(
        cls,
        client: "_ProposalSubmitter",
        *,
        apply_client: "_ProposalApplier | None" = None,
        idempotency_prefix: str = "proposal-builder",
        plan_id: str = "planpythonproposalbuilder001",
    ) -> "ProposalsBuilder":
        """Bind this builder to the private public-seam process client."""
        return cls(
            _client=client,
            _apply_client=apply_client,
            _idempotency_prefix=idempotency_prefix,
            _plan_id=plan_id,
        )

    async def submit(self, batch: ProposalBatch) -> ProposalSubmission:
        """Submit a proposal batch. Engine validates against capability + surface."""
        if self._client is None:
            raise UnboundBuilderError(
                "ProposalsBuilder.submit needs an engine-bound public-seam client; "
                "use the cx.proposals instance supplied to a proposer stage"
            )
        request = ProposalSubmitRequest(
            request_id=f"{self._idempotency_prefix}-submit",
            plan_id=self._plan_id,
            idempotency_key=f"{self._idempotency_prefix}-submit",
            proposals=[_effect_to_wire(effect, batch) for effect in batch.effects],
        )
        result = await asyncio.to_thread(self._client.proposal_submit, request)
        return _proposal_submission_from_result(result)

    async def apply(self, submission: ProposalSubmission) -> WriteReceipt:
        """Ask the engine to apply a previously-submitted proposal batch."""
        if self._apply_client is None:
            raise UnboundBuilderError(
                "ProposalsBuilder.apply needs an engine-bound public-seam client; "
                "use the cx.proposals instance supplied to a proposer stage"
            )
        request = ProposalApplyRequest(
            request_id=f"{self._idempotency_prefix}-apply",
            plan_id=f"{self._plan_id}-apply",
            idempotency_key=f"{self._idempotency_prefix}-apply",
            proposal_batch=submission.batch_id,
        )
        result = await asyncio.to_thread(self._apply_client.proposal_apply, request)
        return _proposal_apply_receipt_from_result(result)

    async def submit_and_apply(self, batch: ProposalBatch) -> WriteReceipt:
        """Convenience: submit + apply in one round-trip."""
        submission = await self.submit(batch)
        return await self.apply(submission)


def _proposal_submission_from_result(result: ProposalSubmitResult) -> ProposalSubmission:
    primary = result.primary
    return ProposalSubmission(
        receipt=WriteReceipt(receipt_id=primary.receipt),
        batch_id=primary.batch_id,
        proposal_ids=list(primary.proposal_ids),
    )


def _proposal_apply_receipt_from_result(result: ProposalApplyResult) -> WriteReceipt:
    primary = result.primary
    return WriteReceipt(receipt_id=primary.receipt)


def _effect_to_wire(effect: ProposalEffect, batch: ProposalBatch) -> ProposalWriteRecord:
    if effect.kind == "create":
        return _create_effect_to_wire(effect, batch)
    if effect.kind in {"change", "change_from_workspace_diff", "change_from_agent_session"}:
        return _change_effect_to_wire(effect, batch)
    raise TypeError(f"unsupported proposal effect: {effect.kind}")


def _create_effect_to_wire(effect: ProposalEffect, batch: ProposalBatch) -> ProposalWriteRecord:
    read_receipts = _receipt_ids(batch)
    return ProposalWriteRecord(
        effect=ProposalEffectCreate(
            artifact_type=_required_string(effect.artifact_type, "create artifact_type"),
            artifact_schema=_required_string(effect.artifact_schema, "create artifact_schema"),
            artifact=_artifact_literal_expr(_required_artifact(effect.artifact, "create artifact")),
        ),
        causal=ProposalCausalInputs(inputs=[]),
        informed_by=_plan_literal_expr(read_receipts),
        read_receipts=read_receipts,
    )


def _change_effect_to_wire(effect: ProposalEffect, batch: ProposalBatch) -> ProposalWriteRecord:
    if effect.parent_candidate_id is None:
        raise ValueError(f"{effect.kind} proposal effects require parent_candidate_id")
    wire_effect: ProposalEffectChange | ProposalEffectAgentSession
    if effect.kind == "change":
        wire_effect = ProposalEffectChange(
            target=_required_string(effect.parent_candidate_id, "change parent_candidate_id"),
            surface_fingerprint=_required_string(effect.surface, "change surface"),
            change_schema=_required_string(effect.change_schema, "change change_schema"),
            change=_literal_expr(_required_value(effect.change_value, "change change")),
        )
    elif effect.kind == "change_from_agent_session":
        if effect.agent_session_receipt is None:
            raise ValueError("change_from_agent_session requires agent_session_receipt")
        agent_receipt = effect.agent_session_receipt.receipt_id
        wire_effect = ProposalEffectAgentSession(
            target=_required_string(
                effect.parent_candidate_id,
                "change_from_agent_session parent_candidate_id",
            ),
            agent_receipt=agent_receipt,
            parser=_required_string(effect.parser, "change_from_agent_session parser"),
            surface_fingerprint=_required_string(effect.surface, "change_from_agent_session surface"),
            change_schema=_required_string(
                effect.change_schema,
                "change_from_agent_session change_schema",
            ),
        )
    else:
        raise TypeError(f"unsupported proposal effect: {effect.kind}")
    read_receipts = _effect_read_receipts(effect, batch)
    return ProposalWriteRecord(
        effect=wire_effect,
        causal=ProposalCausalInputs(inputs=[effect.parent_candidate_id]),
        informed_by=_plan_literal_expr(read_receipts),
        read_receipts=read_receipts,
    )


def _receipt_ids(batch: ProposalBatch) -> list[ReceiptRef]:
    return [
        *(receipt.receipt_id for receipt in batch.read_receipts),
        *(receipt.receipt_id for receipt in batch.effect_receipts),
    ]


def _effect_read_receipts(effect: ProposalEffect, batch: ProposalBatch) -> list[ReceiptRef]:
    receipt_ids = _receipt_ids(batch)
    if effect.kind == "change_from_agent_session":
        if effect.agent_session_receipt is None:
            raise ValueError("change_from_agent_session requires agent_session_receipt")
        return [*receipt_ids, effect.agent_session_receipt.receipt_id]
    return receipt_ids


def _plan_literal_expr(value: JsonValue) -> PlanExpressionLiteral:
    return PlanExpressionLiteral(value=value)


def _literal_expr(value: ProposalChangeValue) -> ValueExprLiteral:
    return ValueExprLiteral(value=_proposal_value(value))


def _artifact_literal_expr(value: ProposalArtifactValue) -> ValueExprLiteral:
    return ValueExprLiteral(value=_artifact_value(value))


def _required_string(value: str | None, field: str) -> str:
    if value is None:
        raise ValueError(f"proposal effect missing {field}")
    return value


def _required_value(value: ProposalChangeValue | None, field: str) -> ProposalChangeValue:
    if value is None:
        raise ValueError(f"proposal effect missing {field}")
    return value


def _required_artifact(value: ProposalArtifactValue | None, field: str) -> ProposalArtifactValue:
    if value is None:
        raise ValueError(f"proposal effect missing {field}")
    return value


def _artifact_value(value: ProposalArtifactValue) -> JsonValue:
    if isinstance(value, DirectoryArtifact | PromptArtifact | SkillBank):
        return parse_json_value(
            json.loads(value.model_dump_json(exclude_none=True)),
            context="proposal artifact",
        )
    raise TypeError(f"unsupported proposal artifact: {type(value).__name__}")


def _proposal_value(value: ProposalChangeValue) -> JsonValue:
    if isinstance(value, PromptTemplateChange):
        return value.to_json_value()
    if isinstance(value, SkillBankChangeRecord):
        return value.to_json_value()
    raise TypeError(f"unsupported proposal change: {type(value).__name__}")


__all__ = ["ProposalSubmission", "ProposalsBuilder"]
