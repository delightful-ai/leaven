"""Environment — the inner-loop bundle: task + rollout + rubric.

The verifiers `Env` analog: a named, shareable unit that says WHAT the task is
(`task`), HOW the current artifact runs on it (`rollout`), and HOW the result
scores (`rubric`). The optimizer (outer loop) and runtime (execution substrate)
stay separate, composed at `optimize(seed=..., environment=env, optimizer=...,
runtime=...)`.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

from .rubric import Rubric
from .stages import Rollout
from .task import Task


class Environment(BaseModel):
    """Inner-loop bundle: task + rollout + rubric."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    task: Task
    rollout: Rollout
    rubric: Rubric


__all__ = ["Environment"]
