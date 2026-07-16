"""`lv.optimize(...).run()` — the entry point for an optimization run.

Composes a seed artifact, an environment (task + rollout + rubric), an
optimizer config, and a runtime into a runnable optimization. The builder is
typed by the artifact type so `result.best.artifact` is fully typed. Train /
validation / test splits come from `Case.split` tags on the environment's task.

`.run()` lowers the whole composition into one locked `leaven/optimize.run`
request against the spawned `leaven seam serve --stdio` host. The host drives
the real GEPA loop (reflect / propose / screen / admit), dispatching per-case
runner and scorer stages back to the configured Python worker, and returns the
optimized projection (best candidate, frontier, cost totals, durable run
reference) plus a durable run checkpoint the result reads back.
"""

import secrets
import time
from typing import cast

from ._errors import UnsupportedConfigurationError
from ._runs import optimized_from_optimize_run
from ._seam_optimize import PlannedOptimizeCase, default_runs_root, run_optimization
from ._seam_optimize.artifact_projection import OptimizeSeed
from .artifacts.agent_kit import AgentKitArtifact
from .artifacts.prompt import PromptArtifact
from .decorators import RegisteredStage
from .environment import Environment
from .optimizers.config import OptimizerConfig
from .optimizers.gepa import Gepa
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

        Termination conditions (whichever comes first): the optimizer's
        candidate-pool cap, the metric-call budget, or the optional usd ceiling.
        """
        seed = self._optimize_seed()
        runner = self._runner_stage()
        optimizer = self._gepa_config()
        cases = self._plan_cases()
        run_id = self._run_id()
        runs_root = default_runs_root()

        outcome = await run_optimization(
            seed=seed,
            cases=cases,
            runner=runner,
            optimizer=optimizer,
            rubric=self.environment.rubric,
            run_id=run_id,
            runtime=self.runtime,
            runs_root=runs_root,
        )
        optimized = optimized_from_optimize_run(outcome)
        return cast("Optimized[A]", optimized)

    def _optimize_seed(self) -> OptimizeSeed:
        if isinstance(self.seed, PromptArtifact | AgentKitArtifact):
            return self.seed
        raise TypeError(
            "lv.optimize optimizes a PromptArtifact or AgentKitArtifact seed; "
            f"got {type(self.seed).__name__}"
        )

    def _runner_stage(self) -> RegisteredStage[OptimizeSeed, str]:
        rollout = self.environment.rollout
        if rollout.kind != "function" or rollout.stage is None:
            raise UnsupportedConfigurationError(
                "this slice supports a function rollout (`Rollout.fn(runner)`); "
                f"got rollout kind {rollout.kind!r}"
            )
        if rollout.stage.role != "runner":
            raise TypeError(
                f"the rollout stage must be a @lv.runner; got role {rollout.stage.role!r}"
            )
        return rollout.stage

    def _gepa_config(self) -> Gepa:
        if not isinstance(self.optimizer, Gepa):
            raise UnsupportedConfigurationError(
                "this slice supports the GEPA optimizer (`lv.optimizers.gepa(...)`); "
                f"got optimizer {self.optimizer.name!r}"
            )
        return self.optimizer

    def _plan_cases(self) -> list[PlannedOptimizeCase]:
        cases = self.environment.task.cases
        if not cases:
            raise ValueError("the task has no cases to optimize over")
        planned = [
            PlannedOptimizeCase(
                case_id=_wire_case_id(case.id),
                input=dict(case.input),
                target=dict(case.target) if case.target is not None else None,
                metadata=dict(case.metadata),
                split=case.split,
            )
            for case in cases
        ]
        seen_ids: set[str] = set()
        duplicate_ids: set[str] = set()
        for case in planned:
            if case.case_id in seen_ids:
                duplicate_ids.add(case.case_id)
            seen_ids.add(case.case_id)
        if duplicate_ids:
            joined = ", ".join(repr(case_id) for case_id in sorted(duplicate_ids))
            raise ValueError(
                f"task case ids must be unique after wire projection; duplicate: {joined}"
            )
        return planned

    def _run_id(self) -> str:
        """Build a fresh run id for this invocation.

        The host writes a durable run dir at `<runs_root>/run_<run_id>` and, on a
        colliding dir, RESUMES the prior optimizer checkpoint (latest-at-start
        consistency). A fixed run id would therefore make `.run()` non-idempotent:
        repeated runs would silently resume stale state and could print a prior
        run's improvement even when the current configuration would not improve.
        So each invocation gets a unique run id (a human-readable task prefix plus
        a monotonic timestamp and random suffix), keeping `.run()` deterministic
        and safe to rerun. The result's `run_dir` points at this fresh dir.
        """
        name = self.environment.task.name
        prefix = _slug(name) if name else "leaven_optimize"
        return f"{prefix}_{time.time_ns():x}_{secrets.token_hex(4)}"


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


def _wire_case_id(case_id: str) -> str:
    """Project a user case id into the wire CaseId pattern `^case_[A-Za-z0-9_.:-]+$`.

    The wire pattern permits hyphens, so preserving them keeps source case
    identities distinct from otherwise-similar underscore ids.
    """
    return case_id if case_id.startswith("case_") else f"case_{case_id}"


def _slug(name: str) -> str:
    """Lower a free-form name into a run-id-safe slug."""
    cleaned = "".join(ch if ch.isalnum() else "_" for ch in name).strip("_")
    return cleaned or "leaven_optimize"


__all__ = ["OptimizeBuilder", "optimize"]
