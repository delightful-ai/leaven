"""`lv.optimize(...).run()` — the entry point for an optimization run.

Composes a seed artifact, an environment (task + rollout + rubric), an
optimizer config, and a runtime into a runnable optimization. The builder is
typed by the artifact type so `result.best.artifact` is fully typed. Train /
validation / test splits come from `Case.split` tags on the environment's task.
"""

from __future__ import annotations

from .environment import Environment
from .optimizers.config import OptimizerConfig
from .result import Optimized
from .runtime import Runtime


class OptimizeBuilder[A]:
    """Builder returned by `lv.optimize(...)`. Call `.run()` to execute."""

    seed: A
    environment: Environment
    optimizer: OptimizerConfig
    runtime: Runtime

    async def run(self) -> Optimized[A]:
        """Execute the optimization. Returns when the optimizer terminates.

        Termination conditions (whichever comes first): optimizer-internal
        stopping criteria, budget exhausted, max iterations reached, user
        cancellation.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    def dry_run(self) -> OptimizeBuilder[A]:
        """Mark the run as dry-run: validates configuration without executing.

        Returns self for chaining. Calling `.run()` afterward returns an
        empty `Optimized[A]` and writes a validation report.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


def optimize[A](
    *,
    seed: A,
    environment: Environment,
    optimizer: OptimizerConfig,
    runtime: Runtime,
) -> OptimizeBuilder[A]:
    """Compose an optimization run. Call `.run()` to execute.

    `environment` bundles the task (cases with split tags, sandbox needs), the
    rollout (how the current artifact runs on a case), and the rubric (how the
    result scores). The optimizer owns the outer loop (reflect / propose /
    judge). Train / validation / test splits are read from `Case.split` on the
    environment's task; the runtime supplies execution, budget, and trust.
    """
    b = OptimizeBuilder[A]()
    b.seed = seed
    b.environment = environment
    b.optimizer = optimizer
    b.runtime = runtime
    return b


__all__ = ["OptimizeBuilder", "optimize"]
