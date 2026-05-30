"""`RegisteredStage` — a role-tagged served stage handle.

Governing spec: `docs/specs/leaven_python.md` — How Python code reaches the
engine (served workers).
"""

from __future__ import annotations

from collections.abc import Awaitable, Callable

from pydantic import BaseModel, ConfigDict

from ..wire.stage_payloads import StageRole

__all__ = ["RegisteredStage"]


class RegisteredStage(BaseModel):
    """A role-tagged stage the worker serves back to the engine."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    role: StageRole
    name: str
    fn: Callable[..., Awaitable[object]]
