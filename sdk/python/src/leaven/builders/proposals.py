"""`cx.proposals.*` — submit + apply proposal batches from proposer stages."""

import asyncio
from typing import Protocol

from pydantic import BaseModel, ConfigDict

from .._receipts import WriteReceipt
from .._seam import ProposalSubmitRequest
from .._seam._wire import JsonObject
from .._seam._wire.json_value import json_object
from .._seam._wire.results import ProposalSubmitResult
from ..json_value import JsonValue
from ..proposal import ProposalBatch, ProposalEffect


class ProposalSubmission(BaseModel):
    """Result of a proposal submission. Apply is a separate call."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    receipt: WriteReceipt
    proposal_ids: list[str]


class _SeamRequester(Protocol):
    """Small private protocol ProposalsBuilder needs from the seam client."""

    def proposal_submit(self, request: ProposalSubmitRequest) -> ProposalSubmitResult: ...


class ProposalsBuilder:
    """Proposal submission bound to a context."""

    def __init__(
        self,
        *,
        _client: "_SeamRequester | None" = None,
        _idempotency_prefix: str = "proposal-builder",
        _plan_id: str = "planpythonproposalbuilder001",
    ) -> None:
        self._client = _client
        self._idempotency_prefix = _idempotency_prefix
        self._plan_id = _plan_id

    @classmethod
    def _for_seam(
        cls,
        client: "_SeamRequester",
        *,
        idempotency_prefix: str = "proposal-builder",
        plan_id: str = "planpythonproposalbuilder001",
    ) -> "ProposalsBuilder":
        """Bind this builder to the private public-seam process client."""
        return cls(
            _client=client,
            _idempotency_prefix=idempotency_prefix,
            _plan_id=plan_id,
        )

    async def submit(self, batch: ProposalBatch) -> ProposalSubmission:
        """Submit a proposal batch. Engine validates against capability + surface."""
        if self._client is None:
            raise NotImplementedError(
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
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def submit_and_apply(self, batch: ProposalBatch) -> WriteReceipt:
        """Convenience: submit + apply in one round-trip."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


def _proposal_submission_from_result(result: ProposalSubmitResult) -> ProposalSubmission:
    primary = result.primary
    return ProposalSubmission(
        receipt=WriteReceipt(receipt_id=primary.receipt),
        proposal_ids=list(primary.proposal_ids),
    )


def _effect_to_wire(effect: ProposalEffect, batch: ProposalBatch) -> JsonObject:
    if effect.kind == "create":
        return _create_effect_to_wire(effect, batch)
    if effect.kind in {"change", "change_from_workspace_diff", "change_from_agent_session"}:
        return _change_effect_to_wire(effect, batch)
    raise TypeError(f"unsupported proposal effect: {effect.kind}")


def _create_effect_to_wire(effect: ProposalEffect, batch: ProposalBatch) -> JsonObject:
    return json_object(
        {
            "effect": {
                "kind": "create",
                "artifact_type": _required_payload_string(effect, "artifact_type"),
                "artifact_schema": _required_payload_string(effect, "artifact_schema"),
                "artifact": _literal_expr(_required_payload(effect, "artifact")),
            },
            "causal": {"inputs": []},
            "informed_by": _literal_expr(_receipt_ids(batch)),
            "read_receipts": _receipt_ids(batch),
        }
    )


def _change_effect_to_wire(effect: ProposalEffect, batch: ProposalBatch) -> JsonObject:
    if effect.parent_candidate_id is None:
        raise ValueError(f"{effect.kind} proposal effects require parent_candidate_id")
    wire_effect: JsonObject = {
        "kind": effect.kind,
        "target": effect.parent_candidate_id,
        "surface_fingerprint": effect.surface,
        "change_schema": _required_payload_string(effect, "change_schema"),
    }
    if effect.kind == "change":
        wire_effect["change"] = _literal_expr(_required_payload(effect, "change"))
    read_receipts = _receipt_ids(batch)
    if effect.kind == "change_from_agent_session":
        if effect.agent_session_receipt is None:
            raise ValueError("change_from_agent_session requires agent_session_receipt")
        agent_receipt = effect.agent_session_receipt.receipt_id
        wire_effect["agent_receipt"] = agent_receipt
        wire_effect["parser"] = _required_payload_string(effect, "parser")
        read_receipts = [*read_receipts, agent_receipt]
    return json_object(
        {
            "effect": wire_effect,
            "causal": {"inputs": [effect.parent_candidate_id]},
            "informed_by": _literal_expr(read_receipts),
            "read_receipts": read_receipts,
        }
    )


def _receipt_ids(batch: ProposalBatch) -> list[str]:
    return [
        *(receipt.receipt_id for receipt in batch.read_receipts),
        *(receipt.receipt_id for receipt in batch.effect_receipts),
    ]


def _literal_expr(value: JsonValue) -> JsonObject:
    return json_object({"kind": "literal", "value": value})


def _required_payload_string(effect: ProposalEffect, key: str) -> str:
    value = _required_payload(effect, key)
    if not isinstance(value, str):
        raise TypeError(f"{effect.kind} proposal payload requires string `{key}`")
    return value


def _required_payload(effect: ProposalEffect, key: str) -> JsonValue:
    if key not in effect.payload:
        raise KeyError(f"{effect.kind} proposal payload requires `{key}`")
    return effect.payload[key]


__all__ = ["ProposalSubmission", "ProposalsBuilder"]
