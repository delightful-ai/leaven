"""`lv.evolve(...)` — stage-composition entry point.

This is the FlashEvolve-shaped product surface: artifact, task, swappable
stages, optimizer, runtime. The scaffold records the composition but still
raises at execution boundaries until the engine wire is implemented.
"""

from __future__ import annotations

from typing import Any

from .optimizers.config import OptimizerConfig
from .runtime import Runtime
from .stages import Stages
from .task import Task


class EvolutionBuilder[A]:
    """Builder returned by `lv.evolve(...)`. Call `.run()` to execute."""

    artifact: A
    task: Task
    stages: Stages
    optimizer: OptimizerConfig
    runtime: Runtime

    async def run(self) -> Any:
        """Execute the evolution run."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


def evolve[A](
    *,
    artifact: A,
    task: Task,
    stages: Stages,
    optimizer: OptimizerConfig,
    runtime: Runtime,
) -> EvolutionBuilder[A]:
    """Compose an evolution run from artifact, task, stages, optimizer, runtime."""
    b = EvolutionBuilder[A]()
    b.artifact = artifact
    b.task = task
    b.stages = stages
    b.optimizer = optimizer
    b.runtime = runtime
    return b


__all__ = ["EvolutionBuilder", "evolve"]
