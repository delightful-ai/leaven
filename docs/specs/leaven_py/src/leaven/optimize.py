"""`lv.optimize(...).run()` — the entry point for an optimization run.

Composes a seed artifact, case sets, an optimizer config, a runtime, and stage
handlers into a runnable optimization. The builder is typed by the artifact
type so `result.best.artifact` is fully typed.
"""

from __future__ import annotations

from typing import Any

from .case import CaseSet
from .decorators import RegisteredStage
from .optimizers.config import OptimizerConfig
from .result import Optimized
from .runtime import Runtime


class OptimizeBuilder[A]:
    """Builder returned by `lv.optimize(...)`. Call `.run()` to execute."""

    seed: A
    train: CaseSet
    val: CaseSet | None
    test: CaseSet | None
    optimizer: OptimizerConfig
    runtime: Runtime
    runner: RegisteredStage[A, Any] | None
    scorer: RegisteredStage[A, Any] | None
    evaluator: RegisteredStage[A, Any] | None
    proposer: RegisteredStage[A, Any] | None
    reflector: RegisteredStage[A, Any] | None
    judge: RegisteredStage[A, Any] | None

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
    train: CaseSet,
    optimizer: OptimizerConfig,
    runtime: Runtime,
    val: CaseSet | None = None,
    test: CaseSet | None = None,
    runner: RegisteredStage[A, Any] | None = None,
    scorer: RegisteredStage[A, Any] | None = None,
    evaluator: RegisteredStage[A, Any] | None = None,
    proposer: RegisteredStage[A, Any] | None = None,
    reflector: RegisteredStage[A, Any] | None = None,
    judge: RegisteredStage[A, Any] | None = None,
) -> OptimizeBuilder[A]:
    """Compose an optimization run. Call `.run()` to execute.

    Required: `seed`, `train`, `optimizer`, `runtime`.
    Almost-always-required: `runner` + `scorer`, OR `evaluator`.
    Optional: `val`, `test`, `proposer` (overrides optimizer default),
    `reflector` (overrides optimizer default), `judge` (for
    pairwise/listwise optimizers).
    """
    b = OptimizeBuilder[A]()
    b.seed = seed
    b.train = train
    b.val = val
    b.test = test
    b.optimizer = optimizer
    b.runtime = runtime
    b.runner = runner
    b.scorer = scorer
    b.evaluator = evaluator
    b.proposer = proposer
    b.reflector = reflector
    b.judge = judge
    return b


__all__ = ["OptimizeBuilder", "optimize"]
