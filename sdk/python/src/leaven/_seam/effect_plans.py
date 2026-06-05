"""Graph-effect Plan IR requests for private public-seam clients."""

from collections.abc import Sequence
from dataclasses import dataclass
from typing import Literal

from msgspec import UNSET

from leaven._seam._wire.expressions import EvaluationSetExpr
from leaven._seam._wire.payloads import (
    CommitPolicyGraphWritesAtomic,
    PlanDocument,
    PlanOp,
    VisibilityClass,
)
from leaven._seam._wire.refs import CandidateRef, ExternalEventPayload, WireJsonObject
from leaven._seam._wire.writes import (
    EmitRunEventWrite,
    EvaluationRequestWriteRecord,
    RequestEvaluationWrite,
)

from .plans import SeamRequestMethod, _plan_document


@dataclass(frozen=True)
class EvaluationRequestRequest:
    """A single public-seam `leaven/evaluation.request` Plan request."""

    request_id: str
    plan_id: str
    idempotency_key: str
    candidates: Sequence[CandidateRef]
    set: EvaluationSetExpr
    granularity: Literal["aggregate", "per_case"]
    purpose: Literal["train", "validation", "test", "diagnostic", "custom"]
    shape: Literal["independent", "pairwise", "listwise"] = "independent"
    evaluator: str | None = None
    metadata: WireJsonObject | None = None

    @property
    def method(self) -> SeamRequestMethod:
        """Locked evaluation-request method."""
        return "leaven/evaluation.request"

    def to_params(self) -> PlanDocument:
        """Return the locked evaluation-request Plan params."""
        return _plan_document(
            plan_id=self.plan_id,
            ops=[self._evaluation_request_write()],
            return_names=["evaluation_request"],
            commit=CommitPolicyGraphWritesAtomic(on_stale="reject"),
        )

    def _evaluation_request_write(self) -> PlanOp:
        return PlanOp(
            kind="write",
            name="evaluation_request",
            idempotency_key=self.idempotency_key,
            write=RequestEvaluationWrite(
                request=EvaluationRequestWriteRecord(
                    shape=self.shape,
                    candidates=list(self.candidates),
                    set=self.set,
                    granularity=self.granularity,
                    purpose=self.purpose,
                    evaluator=self.evaluator if self.evaluator is not None else UNSET,
                    metadata=self.metadata if self.metadata is not None else UNSET,
                )
            ),
        )


@dataclass(frozen=True)
class EventEmitRequest:
    """A single public-seam `leaven/event.emit` Plan request."""

    request_id: str
    plan_id: str
    idempotency_key: str
    event_kind: str
    payload_schema: str
    payload: ExternalEventPayload
    visibility: VisibilityClass

    @property
    def method(self) -> SeamRequestMethod:
        """Locked event-emit method."""
        return "leaven/event.emit"

    def to_params(self) -> PlanDocument:
        """Return the locked event-emit Plan params."""
        return _plan_document(
            plan_id=self.plan_id,
            ops=[self._event_write()],
            return_names=["run_event"],
            commit=CommitPolicyGraphWritesAtomic(on_stale="reject"),
        )

    def _event_write(self) -> PlanOp:
        return PlanOp(
            kind="write",
            name="run_event",
            idempotency_key=self.idempotency_key,
            write=EmitRunEventWrite(
                event_kind=self.event_kind,
                payload_schema=self.payload_schema,
                payload=self.payload,
                visibility=self.visibility,
            ),
        )


__all__ = ["EvaluationRequestRequest", "EventEmitRequest"]
