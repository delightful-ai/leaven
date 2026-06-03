"""Concrete private contexts supplied to Python stage functions."""

from __future__ import annotations

from .._handles import WorkspaceHandle
from .._receipts import CallReceipt
from ..builders.agent import AgentBuilder
from ..contexts import RolloutContext
from .lm import CallbackLmBuilder
from .protocols import AgentRunCallback, LmCompleteCallback


class CallbackRolloutContext(RolloutContext):
    """A live `RolloutContext` bound to a stage driver's effect callbacks.

    `cx.lm.complete(...)` and `cx.agent.run(...)` route through the active stage
    callback. The other effect builders (`sandbox`, `workspace`, `batch`) stay
    scaffold for this slice.
    """

    def __init__(
        self,
        callback: LmCompleteCallback,
        *,
        candidate_id: str,
        stage_call_id: str,
        agent_callback: AgentRunCallback | None = None,
    ) -> None:
        self.lm = CallbackLmBuilder(callback, stage_call_id)
        self.agent = (
            AgentBuilder._for_seam(
                agent_callback,
                candidate_id=candidate_id,
                idempotency_prefix=f"{stage_call_id}-agent",
                plan_id=f"plan_{_id_fragment(stage_call_id)}_agent",
            )
            if agent_callback is not None
            else AgentBuilder()
        )
        self._candidate_id = candidate_id
        self._stage_call_id = stage_call_id

    @property
    def candidate_id(self) -> str:
        return self._candidate_id

    @property
    def stage_id(self) -> str:
        return self._stage_call_id

    @property
    def rollout_workspace(self) -> WorkspaceHandle:
        return WorkspaceHandle(
            workspace_id=_materialized_workspace_id(self._candidate_id),
            candidate_id=self._candidate_id,
            lifetime="stage_call",
            receipt=CallReceipt(receipt_id=f"wrec_{_id_fragment(self._stage_call_id)}"),
        )


__all__ = ["CallbackRolloutContext"]


def _materialized_workspace_id(candidate_id: str) -> str:
    stem = candidate_id.removeprefix("cand_")
    sanitized = "".join(ch if ch.isalnum() or ch == "_" else "_" for ch in stem)
    return f"ws_{sanitized}_materialized"


def _id_fragment(value: str) -> str:
    return "".join(ch if ch.isalnum() else "_" for ch in value)
