"""Concrete private contexts supplied to Python stage functions."""

from __future__ import annotations

from ..contexts import RolloutContext
from .lm import CallbackLmBuilder
from .protocols import LmCompleteCallback


class CallbackRolloutContext(RolloutContext):
    """A live `RolloutContext` bound to a stage driver's effect callbacks.

    `cx.lm.complete(...)` routes through the active stage callback. The other
    effect builders (`agent`, `sandbox`, `workspace`, `batch`) stay scaffold for
    this slice; the prompt/LM/exact-match path uses only `lm`.
    """

    def __init__(
        self,
        callback: LmCompleteCallback,
        *,
        candidate_id: str,
        stage_call_id: str,
    ) -> None:
        self.lm = CallbackLmBuilder(callback, stage_call_id)
        self._candidate_id = candidate_id
        self._stage_call_id = stage_call_id

    @property
    def candidate_id(self) -> str:
        return self._candidate_id

    @property
    def stage_id(self) -> str:
        return self._stage_call_id


__all__ = ["CallbackRolloutContext"]
